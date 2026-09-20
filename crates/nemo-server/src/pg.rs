//! Write-through adapter for the frozen phase-8 schema (ADR-0033).
//!
//! `query()` (not `query!`) so tests compile without a live database. When
//! `DATABASE_URL` is unset the process stays in-memory.

use std::collections::{BTreeMap, HashMap, VecDeque};

use nemo_wire::cbor;
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope};
use nemo_wire::hpke::HpkeKeypair;
use nemo_wire::ids::{self, IdentityId, KEY_LEN};
use nemo_wire::{HomeServerBinding, RevocationStatement, ServerBundle, SigningKey};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use thiserror::Error;

use crate::federation::{OutboundRow, PeerState};
use crate::group::{
    CredRecord, FanoutTarget, FileSlot, GroupHost, GroupId, GroupState, MemberCred, PendingRecord,
    StoredInvite, StreamRow,
};
use crate::home::{DirectoryRow, HomeServer, Mailbox, StoredEnvelope, TokenKind};

#[derive(Debug, Error)]
pub enum PersistError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("corrupt postgres row: {0}")]
    Corrupt(&'static str),
}

pub type Result<T> = std::result::Result<T, PersistError>;

pub async fn connect(url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(url)
        .await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(sqlx::Error::from)?;
    Ok(pool)
}

/// Load durable state, or mint a new identity and persist it.
pub async fn load_or_init(
    pool: &PgPool,
    host: &str,
    s2s_port: u16,
) -> Result<(HomeServer, GroupHost)> {
    match load(pool).await? {
        Some((mut home, groups)) => {
            if home.bundle().host != host || home.bundle().s2s_port != s2s_port as u64 {
                home.rebind_host(host.to_string(), s2s_port);
                flush(pool, &home, &groups).await?;
            }
            Ok((home, groups))
        }
        None => {
            let home = HomeServer::advertise(host.to_string(), s2s_port);
            let groups = GroupHost::new();
            flush(pool, &home, &groups).await?;
            Ok((home, groups))
        }
    }
}

pub async fn flush(pool: &PgPool, home: &HomeServer, groups: &GroupHost) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "TRUNCATE TABLE mailbox_rows, tokens, prekeys, memberships, \
         group_files, group_stream, group_pending, group_invites, group_creds, groups, \
         outbound, hpke_seen, peers, identities, server_identity \
         RESTART IDENTITY CASCADE",
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO server_identity \
         (hpke_secret, hpke_public, sign_secret, sign_public, host, s2s_port) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(home.hpke.secret.as_slice())
    .bind(home.hpke.public.as_slice())
    .bind(home.sign.to_bytes().as_slice())
    .bind(home.sign.verifying_key().to_bytes().as_slice())
    .bind(home.bundle.host.as_str())
    .bind(home.bundle.s2s_port as i32)
    .execute(&mut *tx)
    .await?;

    let mut ids: Vec<IdentityId> = home.mailboxes.keys().copied().collect();
    for id in home.directory.keys() {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    ids.sort();
    for id in ids {
        insert_identity(&mut tx, home, id).await?;
    }

    for (id, box_) in &home.mailboxes {
        for row in box_.rows.values() {
            let padded = row
                .inner
                .encode_padded()
                .map_err(|_| PersistError::Corrupt("inner encode"))?;
            sqlx::query(
                "INSERT INTO mailbox_rows \
                 (identity_id, seq, received_at, expires_at, inner_padded, idempotency_token) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id.as_slice())
            .bind(row.seq as i64)
            .bind(row.received_at as i64)
            .bind(row.expires_at as i64)
            .bind(padded.as_slice())
            .bind(row.inner.idempotency_token.as_slice())
            .execute(&mut *tx)
            .await?;
        }
    }

    for (token, kind) in &home.tokens {
        match kind {
            TokenKind::Share {
                mailbox,
                expires_at,
                burned,
                reserved_prekey,
            } => {
                sqlx::query(
                    "INSERT INTO tokens \
                     (token, kind, mailbox, expires_at, burned, reserved_prekey, \
                      grace_until, window_start, window_count) \
                     VALUES ($1, 1, $2, $3, $4, $5, NULL, NULL, 0)",
                )
                .bind(token.as_slice())
                .bind(mailbox.as_slice())
                .bind(*expires_at as i64)
                .bind(*burned)
                .bind(reserved_prekey.as_deref())
                .execute(&mut *tx)
                .await?;
            }
            TokenKind::Contact {
                mailbox,
                grace_until,
                window_start,
                window_count,
            } => {
                sqlx::query(
                    "INSERT INTO tokens \
                     (token, kind, mailbox, expires_at, burned, reserved_prekey, \
                      grace_until, window_start, window_count) \
                     VALUES ($1, 2, $2, NULL, FALSE, NULL, $3, $4, $5)",
                )
                .bind(token.as_slice())
                .bind(mailbox.as_slice())
                .bind(grace_until.map(|g| g as i64))
                .bind(*window_start as i64)
                .bind(*window_count as i32)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    for (id, queue) in &home.prekeys {
        for blob in queue {
            sqlx::query("INSERT INTO prekeys (identity_id, blob) VALUES ($1, $2)")
                .bind(id.as_slice())
                .bind(blob.as_slice())
                .execute(&mut *tx)
                .await?;
        }
    }

    for (id, list) in &home.memberships {
        for gid in list {
            sqlx::query("INSERT INTO memberships (identity_id, group_id) VALUES ($1, $2)")
                .bind(id.as_slice())
                .bind(gid.0.as_slice())
                .execute(&mut *tx)
                .await?;
        }
    }

    for (gid, g) in &groups.groups {
        sqlx::query("INSERT INTO groups (group_id, next_seq, bytes) VALUES ($1, $2, $3)")
            .bind(gid.0.as_slice())
            .bind(g.next_seq as i64)
            .bind(g.bytes as i64)
            .execute(&mut *tx)
            .await?;
        for (cid, rec) in &g.creds {
            sqlx::query(
                "INSERT INTO group_creds \
                 (group_id, credential_id, credential_secret, live, signing_pk, \
                  fanout_capability, fanout_hpke) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(gid.0.as_slice())
            .bind(cid.as_slice())
            .bind(rec.secret.as_slice())
            .bind(rec.live)
            .bind(rec.signing_pk.as_ref().map(|p| p.as_slice()))
            .bind(
                rec.fanout
                    .as_ref()
                    .map(|f| f.delivery_capability.as_slice()),
            )
            .bind(rec.fanout.as_ref().map(|f| f.home_hpke_public.as_slice()))
            .execute(&mut *tx)
            .await?;
        }
        for (nonce, inv) in &g.invites {
            sqlx::query(
                "INSERT INTO group_invites (group_id, nonce, expires_at, invitee_binding) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(gid.0.as_slice())
            .bind(nonce.as_slice())
            .bind(inv.expires_at as i64)
            .bind(inv.invitee_binding.as_ref().map(|b| b.as_slice()))
            .execute(&mut *tx)
            .await?;
        }
        for (pid, pending) in &g.pending {
            sqlx::query(
                "INSERT INTO group_pending \
                 (group_id, pending_id, credential_id, credential_secret, expires_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(gid.0.as_slice())
            .bind(pid.as_slice())
            .bind(pending.cred.credential_id.as_slice())
            .bind(pending.cred.credential_secret.as_slice())
            .bind(pending.expires_at as i64)
            .execute(&mut *tx)
            .await?;
        }
        for row in &g.stream {
            sqlx::query(
                "INSERT INTO group_stream (group_id, seq, \"type\", body, received_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(gid.0.as_slice())
            .bind(row.seq as i64)
            .bind(row.type_ as i16)
            .bind(row.body.as_slice())
            .bind(row.received_at as i64)
            .execute(&mut *tx)
            .await?;
        }
    }

    for (token, slot) in &groups.files {
        sqlx::query(
            "INSERT INTO group_files \
             (fetch_token, group_id, owner_cred, size_bytes, expires_at, bytes) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(token.as_slice())
        .bind(slot.group_id.0.as_slice())
        .bind(slot.owner.as_slice())
        .bind(slot.size as i32)
        .bind(slot.expires_at as i64)
        .bind(slot.bytes.as_deref())
        .execute(&mut *tx)
        .await?;
    }

    for (sid, peer) in &home.peers {
        sqlx::query(
            "INSERT INTO peers \
             (server_id, bundle_cbor, refused, last_recv_counter, last_send_counter) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(sid.as_slice())
        .bind(peer.bundle.encode())
        .bind(peer.refused)
        .bind(peer.last_rx as i64)
        .bind(peer.next_tx as i64)
        .execute(&mut *tx)
        .await?;
    }

    for row in &home.outbound {
        sqlx::query(
            "INSERT INTO outbound (dest, outer_cbor, enqueued_at, next_attempt, backoff_secs) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(row.dest.as_slice())
        .bind(row.outer.encode())
        .bind(row.enqueued_at as i64)
        .bind(row.next_attempt as i64)
        .bind(row.backoff_secs as i64)
        .execute(&mut *tx)
        .await?;
    }

    for (enc, seq) in &home.seen_enc {
        sqlx::query("INSERT INTO hpke_seen (enc, seq, seen_at) VALUES ($1, $2, $3)")
            .bind(enc.as_slice())
            .bind(*seq as i64)
            .bind(0_i64)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn load(pool: &PgPool) -> Result<Option<(HomeServer, GroupHost)>> {
    let ident = sqlx::query(
        "SELECT hpke_secret, hpke_public, sign_secret, sign_public, host, s2s_port \
         FROM server_identity",
    )
    .fetch_optional(pool)
    .await?;
    let Some(ident) = ident else {
        return Ok(None);
    };

    let hpke_secret: Vec<u8> = ident.try_get("hpke_secret")?;
    let hpke_public: Vec<u8> = ident.try_get("hpke_public")?;
    let sign_secret: Vec<u8> = ident.try_get("sign_secret")?;
    let host: String = ident.try_get("host")?;
    let s2s_port: i32 = ident.try_get("s2s_port")?;
    let hpke = HpkeKeypair {
        public: key32(&hpke_public)?,
        secret: key32(&hpke_secret)?,
    };
    let sign_bytes = key32(&sign_secret)?;
    let sign = SigningKey::from_bytes(&sign_bytes);
    let mut home = HomeServer::from_identity(hpke, sign, host, s2s_port as u16);
    let mut groups = GroupHost::new();

    let id_rows = sqlx::query(
        "SELECT identity_id, identity_public_key, revocation_public_key, binding_cbor, \
         binding_seq, revocation_cbor, mailbox_disabled, mailbox_next_seq, mailbox_bytes \
         FROM identities",
    )
    .fetch_all(pool)
    .await?;
    for row in id_rows {
        let id = key32(row.try_get::<Vec<u8>, _>("identity_id")?.as_slice())?;
        let pk = key32(row.try_get::<Vec<u8>, _>("identity_public_key")?.as_slice())?;
        let rev_pk = key32(
            row.try_get::<Vec<u8>, _>("revocation_public_key")?
                .as_slice(),
        )?;
        let binding_cbor: Vec<u8> = row.try_get("binding_cbor")?;
        let binding_seq: i64 = row.try_get("binding_seq")?;
        let revocation_cbor: Option<Vec<u8>> = row.try_get("revocation_cbor")?;
        let disabled: bool = row.try_get("mailbox_disabled")?;
        let next_seq: i64 = row.try_get("mailbox_next_seq")?;
        let bytes: i64 = row.try_get("mailbox_bytes")?;

        if next_seq > 0 {
            home.mailboxes.insert(
                id,
                Mailbox {
                    owner_pk: pk,
                    next_seq: next_seq as u64,
                    rows: BTreeMap::new(),
                    idempotency: HashMap::new(),
                    bytes: bytes as u64,
                    disabled,
                },
            );
        }
        if !binding_cbor.is_empty() {
            let binding = decode_binding(&binding_cbor)?;
            let revocation = match revocation_cbor {
                Some(b) if !b.is_empty() => Some(
                    RevocationStatement::decode(&b)
                        .map_err(|_| PersistError::Corrupt("revocation"))?,
                ),
                _ => None,
            };
            home.directory.insert(
                id,
                DirectoryRow {
                    identity_public_key: pk,
                    revocation_public_key: rev_pk,
                    binding,
                    binding_seq: binding_seq as u64,
                    revocation,
                },
            );
        }
    }

    let mail_rows = sqlx::query(
        "SELECT identity_id, seq, received_at, expires_at, inner_padded, idempotency_token \
         FROM mailbox_rows ORDER BY identity_id, seq",
    )
    .fetch_all(pool)
    .await?;
    for row in mail_rows {
        let id = key32(row.try_get::<Vec<u8>, _>("identity_id")?.as_slice())?;
        let seq: i64 = row.try_get("seq")?;
        let inner_padded: Vec<u8> = row.try_get("inner_padded")?;
        let inner = InnerEnvelope::decode_padded(&inner_padded)
            .map_err(|_| PersistError::Corrupt("inner"))?;
        let stored = StoredEnvelope {
            seq: seq as u64,
            received_at: row.try_get::<i64, _>("received_at")? as u64,
            expires_at: row.try_get::<i64, _>("expires_at")? as u64,
            inner,
        };
        if let Some(box_) = home.mailboxes.get_mut(&id) {
            box_.idempotency
                .insert(stored.inner.idempotency_token, stored.seq);
            box_.rows.insert(stored.seq, stored);
        }
    }

    let tok_rows = sqlx::query(
        "SELECT token, kind, mailbox, expires_at, burned, reserved_prekey, \
         grace_until, window_start, window_count FROM tokens",
    )
    .fetch_all(pool)
    .await?;
    for row in tok_rows {
        let token = key32(row.try_get::<Vec<u8>, _>("token")?.as_slice())?;
        let kind: i16 = row.try_get("kind")?;
        let mailbox = key32(row.try_get::<Vec<u8>, _>("mailbox")?.as_slice())?;
        let tkind = match kind {
            1 => TokenKind::Share {
                mailbox,
                expires_at: row.try_get::<Option<i64>, _>("expires_at")?.unwrap_or(0) as u64,
                burned: row.try_get("burned")?,
                reserved_prekey: row.try_get("reserved_prekey")?,
            },
            2 => TokenKind::Contact {
                mailbox,
                grace_until: row
                    .try_get::<Option<i64>, _>("grace_until")?
                    .map(|g| g as u64),
                window_start: row.try_get::<Option<i64>, _>("window_start")?.unwrap_or(0) as u64,
                window_count: row.try_get::<i32, _>("window_count")? as usize,
            },
            _ => return Err(PersistError::Corrupt("token kind")),
        };
        home.tokens.push((token, tkind));
    }

    let pre_rows = sqlx::query("SELECT identity_id, blob FROM prekeys ORDER BY identity_id, pos")
        .fetch_all(pool)
        .await?;
    for row in pre_rows {
        let id = key32(row.try_get::<Vec<u8>, _>("identity_id")?.as_slice())?;
        let blob: Vec<u8> = row.try_get("blob")?;
        home.prekeys.entry(id).or_insert_with(VecDeque::new).push_back(blob);
    }

    let mem_rows = sqlx::query("SELECT identity_id, group_id FROM memberships")
        .fetch_all(pool)
        .await?;
    for row in mem_rows {
        let id = key32(row.try_get::<Vec<u8>, _>("identity_id")?.as_slice())?;
        let gid = GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?);
        home.memberships.entry(id).or_default().push(gid);
    }

    let g_rows = sqlx::query("SELECT group_id, next_seq, bytes FROM groups")
        .fetch_all(pool)
        .await?;
    for row in g_rows {
        let gid = GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?);
        groups.groups.insert(
            gid,
            GroupState {
                next_seq: row.try_get::<i64, _>("next_seq")? as u64,
                creds: HashMap::new(),
                invites: HashMap::new(),
                pending: HashMap::new(),
                bytes: row.try_get::<i64, _>("bytes")? as u64,
                stream: Vec::new(),
            },
        );
    }

    let cred_rows = sqlx::query(
        "SELECT group_id, credential_id, credential_secret, live, signing_pk, \
         fanout_capability, fanout_hpke FROM group_creds",
    )
    .fetch_all(pool)
    .await?;
    for row in cred_rows {
        let gid = GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?);
        let cid = key32(row.try_get::<Vec<u8>, _>("credential_id")?.as_slice())?;
        let secret = key32(row.try_get::<Vec<u8>, _>("credential_secret")?.as_slice())?;
        let signing_pk = opt_key(row.try_get("signing_pk")?)?;
        let cap = opt_key(row.try_get("fanout_capability")?)?;
        let hpke = opt_key(row.try_get("fanout_hpke")?)?;
        let fanout = match (cap, hpke) {
            (Some(delivery_capability), Some(home_hpke_public)) => Some(FanoutTarget {
                delivery_capability,
                home_hpke_public,
            }),
            _ => None,
        };
        if let Some(g) = groups.groups.get_mut(&gid) {
            g.creds.insert(
                cid,
                CredRecord {
                    secret,
                    live: row.try_get("live")?,
                    signing_pk,
                    fanout,
                },
            );
        }
    }

    let inv_rows =
        sqlx::query("SELECT group_id, nonce, expires_at, invitee_binding FROM group_invites")
            .fetch_all(pool)
            .await?;
    for row in inv_rows {
        let gid = GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?);
        let nonce = key32(row.try_get::<Vec<u8>, _>("nonce")?.as_slice())?;
        if let Some(g) = groups.groups.get_mut(&gid) {
            g.invites.insert(
                nonce,
                StoredInvite {
                    expires_at: row.try_get::<i64, _>("expires_at")? as u64,
                    invitee_binding: opt_key(row.try_get("invitee_binding")?)?,
                },
            );
        }
    }

    let pend_rows = sqlx::query(
        "SELECT group_id, pending_id, credential_id, credential_secret, expires_at \
         FROM group_pending",
    )
    .fetch_all(pool)
    .await?;
    for row in pend_rows {
        let gid = GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?);
        let pid = key32(row.try_get::<Vec<u8>, _>("pending_id")?.as_slice())?;
        if let Some(g) = groups.groups.get_mut(&gid) {
            g.pending.insert(
                pid,
                PendingRecord {
                    cred: MemberCred {
                        credential_id: key32(
                            row.try_get::<Vec<u8>, _>("credential_id")?.as_slice(),
                        )?,
                        credential_secret: key32(
                            row.try_get::<Vec<u8>, _>("credential_secret")?.as_slice(),
                        )?,
                    },
                    expires_at: row.try_get::<i64, _>("expires_at")? as u64,
                },
            );
        }
    }

    let stream_rows = sqlx::query(
        "SELECT group_id, seq, \"type\", body, received_at FROM group_stream ORDER BY group_id, seq",
    )
    .fetch_all(pool)
    .await?;
    for row in stream_rows {
        let gid = GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?);
        let type_u: i16 = row.try_get("type")?;
        let type_ = MessageType::from_u8(type_u as u8)
            .map_err(|_| PersistError::Corrupt("stream type"))?;
        if let Some(g) = groups.groups.get_mut(&gid) {
            g.stream.push(StreamRow {
                seq: row.try_get::<i64, _>("seq")? as u64,
                type_,
                body: row.try_get("body")?,
                received_at: row.try_get::<i64, _>("received_at")? as u64,
            });
        }
    }

    let file_rows = sqlx::query(
        "SELECT fetch_token, group_id, owner_cred, size_bytes, expires_at, bytes FROM group_files",
    )
    .fetch_all(pool)
    .await?;
    for row in file_rows {
        let token = key32(row.try_get::<Vec<u8>, _>("fetch_token")?.as_slice())?;
        groups.files.insert(
            token,
            FileSlot {
                group_id: GroupId(key32(row.try_get::<Vec<u8>, _>("group_id")?.as_slice())?),
                owner: key32(row.try_get::<Vec<u8>, _>("owner_cred")?.as_slice())?,
                size: row.try_get::<i32, _>("size_bytes")? as usize,
                expires_at: row.try_get::<i64, _>("expires_at")? as u64,
                bytes: row.try_get("bytes")?,
            },
        );
    }

    let peer_rows = sqlx::query(
        "SELECT server_id, bundle_cbor, refused, last_recv_counter, last_send_counter FROM peers",
    )
    .fetch_all(pool)
    .await?;
    for row in peer_rows {
        let sid = key32(row.try_get::<Vec<u8>, _>("server_id")?.as_slice())?;
        let bundle = ServerBundle::decode(&row.try_get::<Vec<u8>, _>("bundle_cbor")?)
            .map_err(|_| PersistError::Corrupt("peer bundle"))?;
        home.peers.insert(
            sid,
            PeerState::restore(
                bundle,
                row.try_get("refused")?,
                row.try_get::<i64, _>("last_recv_counter")? as u64,
                row.try_get::<i64, _>("last_send_counter")? as u64,
            ),
        );
    }

    let out_rows = sqlx::query(
        "SELECT dest, outer_cbor, enqueued_at, next_attempt, backoff_secs FROM outbound ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    for row in out_rows {
        home.outbound.push(OutboundRow {
            dest: key32(row.try_get::<Vec<u8>, _>("dest")?.as_slice())?,
            outer: OuterEnvelope::decode(&row.try_get::<Vec<u8>, _>("outer_cbor")?)
                .map_err(|_| PersistError::Corrupt("outer"))?,
            enqueued_at: row.try_get::<i64, _>("enqueued_at")? as u64,
            next_attempt: row.try_get::<i64, _>("next_attempt")? as u64,
            backoff_secs: row.try_get::<i64, _>("backoff_secs")? as u64,
        });
    }

    let seen_rows = sqlx::query("SELECT enc, seq FROM hpke_seen")
        .fetch_all(pool)
        .await?;
    for row in seen_rows {
        home.seen_enc.insert(
            key32(row.try_get::<Vec<u8>, _>("enc")?.as_slice())?,
            row.try_get::<i64, _>("seq")? as u64,
        );
    }

    Ok(Some((home, groups)))
}

async fn insert_identity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    id: IdentityId,
) -> Result<()> {
    let dir = home.directory.get(&id);
    let box_ = home.mailboxes.get(&id);
    let pk = dir
        .map(|d| d.identity_public_key)
        .or_else(|| box_.map(|b| b.owner_pk))
        .unwrap_or([0u8; KEY_LEN]);
    let rev_pk = dir.map(|d| d.revocation_public_key).unwrap_or([0u8; KEY_LEN]);
    let binding_cbor = match dir {
        Some(d) => encode_binding(&d.binding),
        None => Vec::new(),
    };
    let binding_seq = dir.map(|d| d.binding_seq).unwrap_or(0);
    let revocation_cbor = dir.and_then(|d| d.revocation.as_ref().map(|r| r.encode()));
    let (disabled, next_seq, bytes) = match box_ {
        Some(b) => (b.disabled, b.next_seq as i64, b.bytes as i64),
        None => (false, 0, 0),
    };
    sqlx::query(
        "INSERT INTO identities \
         (identity_id, identity_public_key, revocation_public_key, binding_cbor, \
          binding_seq, revocation_cbor, mailbox_disabled, mailbox_next_seq, mailbox_bytes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id.as_slice())
    .bind(pk.as_slice())
    .bind(rev_pk.as_slice())
    .bind(binding_cbor.as_slice())
    .bind(binding_seq as i64)
    .bind(revocation_cbor.as_deref())
    .bind(disabled)
    .bind(next_seq)
    .bind(bytes)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn key32(bytes: &[u8]) -> Result<[u8; KEY_LEN]> {
    ids::copy_fixed(bytes).map_err(|_| PersistError::Corrupt("key length"))
}

fn opt_key(bytes: Option<Vec<u8>>) -> Result<Option<[u8; KEY_LEN]>> {
    match bytes {
        Some(b) if !b.is_empty() => Ok(Some(key32(&b)?)),
        _ => Ok(None),
    }
}

fn encode_binding(binding: &HomeServerBinding) -> Vec<u8> {
    cbor::encode(&binding.to_cbor_value())
}

fn decode_binding(bytes: &[u8]) -> Result<HomeServerBinding> {
    let value = cbor::decode(bytes).map_err(|_| PersistError::Corrupt("binding cbor"))?;
    HomeServerBinding::from_cbor_value(&value).map_err(|_| PersistError::Corrupt("binding"))
}
