//! Write-through adapter for the frozen phase-8 schema (ADR-0033).
//!
//! `query()` (not `query!`) so tests compile without a live database. When
//! `DATABASE_URL` is unset the process stays in-memory.

use std::collections::{BTreeMap, HashMap, VecDeque};

use nemo_wire::cbor;
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope};
use nemo_wire::hpke::HpkeKeypair;
use nemo_wire::ids::{self, IdentityId, ServerId, KEY_LEN};
use nemo_wire::{HomeServerBinding, RevocationStatement, ServerBundle, SigningKey};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use thiserror::Error;

use crate::federation::{OutboundRow, PeerState};
use crate::group::{
    CredRecord, FanoutTarget, FileSlot, GroupDirty, GroupHost, GroupId, GroupState, MemberCred,
    PendingRecord, StoredInvite, StreamRow,
};
use crate::home::{DirectoryRow, HomeDirty, HomeServer, Mailbox, StoredEnvelope, TokenKind};

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
        Some((mut home, mut groups)) => {
            if home.bundle().host != host || home.bundle().s2s_port != s2s_port as u64 {
                home.rebind_host(host.to_string(), s2s_port);
                persist(pool, &mut home, &mut groups).await?;
            }
            Ok((home, groups))
        }
        None => {
            let mut home = HomeServer::advertise(host.to_string(), s2s_port);
            let mut groups = GroupHost::new();
            persist(pool, &mut home, &mut groups).await?;
            Ok((home, groups))
        }
    }
}

/// Incremental write-through persist. Drains the [`HomeDirty`]/[`GroupDirty`]
/// journals and writes only what changed since the last successful call, in a
/// single transaction ordered for foreign keys (identities before children).
///
/// Clean state short-circuits with zero statements, so change-free polls cost
/// nothing. On error the journals merge back, so no change is ever lost.
pub async fn persist(pool: &PgPool, home: &mut HomeServer, groups: &mut GroupHost) -> Result<()> {
    if home.dirty.is_clean() && groups.dirty.is_clean() {
        return Ok(());
    }
    let hd = std::mem::take(&mut home.dirty);
    let gd = std::mem::take(&mut groups.dirty);
    let res = persist_inner(pool, home, groups, &hd, &gd).await;
    if res.is_err() {
        home.dirty.absorb(hd);
        groups.dirty.absorb(gd);
    }
    res
}

async fn persist_inner(
    pool: &PgPool,
    home: &HomeServer,
    groups: &GroupHost,
    hd: &HomeDirty,
    gd: &GroupDirty,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    if hd.server_identity {
        upsert_server_identity(&mut tx, home).await?;
    }
    for id in &hd.identities {
        upsert_identity(&mut tx, home, id).await?;
    }
    for owner in &hd.mailbox_cleared {
        delete_mailbox_all(&mut tx, owner).await?;
    }
    for (owner, seq) in &hd.mailbox_dropped {
        delete_mailbox_exact(&mut tx, owner, *seq).await?;
    }
    for (owner, upto) in &hd.mailbox_acked {
        delete_mailbox_acked(&mut tx, owner, *upto).await?;
    }
    insert_mailbox_rows(&mut tx, home, &hd.mailbox_new).await?;
    for owner in &hd.tokens_dropped {
        delete_tokens_owner(&mut tx, owner).await?;
    }
    for token in &hd.tokens {
        upsert_token(&mut tx, home, token).await?;
    }
    for owner in &hd.prekeys {
        rewrite_prekeys(&mut tx, home, owner).await?;
    }
    for owner in &hd.memberships {
        rewrite_memberships(&mut tx, home, owner).await?;
    }
    for peer in &hd.peers {
        upsert_peer(&mut tx, home, peer).await?;
    }
    if hd.outbound_dirty {
        rewrite_outbound(&mut tx, home).await?;
    }
    if hd.seen_enc_cleared {
        rewrite_seen(&mut tx, home).await?;
    } else {
        insert_seen_new(&mut tx, home, &hd.seen_enc_new).await?;
    }
    for gid in &gd.groups {
        upsert_group(&mut tx, groups, gid).await?;
    }
    for (gid, cid) in &gd.creds {
        upsert_cred(&mut tx, groups, gid, cid).await?;
    }
    for gid in &gd.invites {
        rewrite_invites(&mut tx, groups, gid).await?;
    }
    for gid in &gd.pending {
        rewrite_pending(&mut tx, groups, gid).await?;
    }
    insert_stream_new(&mut tx, groups, &gd.stream_new).await?;
    for token in &gd.files {
        upsert_file(&mut tx, groups, token).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn upsert_server_identity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO server_identity \
         (singleton, hpke_secret, hpke_public, sign_secret, sign_public, host, s2s_port) \
         VALUES (TRUE, $1, $2, $3, $4, $5, $6) \
         ON CONFLICT (singleton) DO UPDATE SET \
         hpke_secret = excluded.hpke_secret, hpke_public = excluded.hpke_public, \
         sign_secret = excluded.sign_secret, sign_public = excluded.sign_public, \
         host = excluded.host, s2s_port = excluded.s2s_port",
    )
    .bind(home.hpke.secret.as_slice())
    .bind(home.hpke.public.as_slice())
    .bind(home.sign.to_bytes().as_slice())
    .bind(home.sign.verifying_key().to_bytes().as_slice())
    .bind(home.bundle.host.as_str())
    .bind(home.bundle.s2s_port as i32)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

struct IdentityRow {
    pk: [u8; KEY_LEN],
    rev_pk: [u8; KEY_LEN],
    binding_cbor: Vec<u8>,
    binding_seq: i64,
    revocation_cbor: Option<Vec<u8>>,
    disabled: bool,
    next_seq: i64,
    bytes: i64,
}

fn identity_row(home: &HomeServer, id: &IdentityId) -> IdentityRow {
    let dir = home.directory.get(id);
    let box_ = home.mailboxes.get(id);
    IdentityRow {
        pk: dir
            .map(|d| d.identity_public_key)
            .or_else(|| box_.map(|b| b.owner_pk))
            .unwrap_or([0u8; KEY_LEN]),
        rev_pk: dir.map(|d| d.revocation_public_key).unwrap_or([0u8; KEY_LEN]),
        binding_cbor: dir.map(|d| encode_binding(&d.binding)).unwrap_or_default(),
        binding_seq: dir.map(|d| d.binding_seq as i64).unwrap_or(0),
        revocation_cbor: dir.and_then(|d| d.revocation.as_ref().map(|r| r.encode())),
        disabled: box_.map(|b| b.disabled).unwrap_or(false),
        next_seq: box_.map(|b| b.next_seq as i64).unwrap_or(0),
        bytes: box_.map(|b| b.bytes as i64).unwrap_or(0),
    }
}

async fn upsert_identity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    id: &IdentityId,
) -> Result<()> {
    let r = identity_row(home, id);
    sqlx::query(
        "INSERT INTO identities \
         (identity_id, identity_public_key, revocation_public_key, binding_cbor, \
          binding_seq, revocation_cbor, mailbox_disabled, mailbox_next_seq, mailbox_bytes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (identity_id) DO UPDATE SET \
         identity_public_key = excluded.identity_public_key, \
         revocation_public_key = excluded.revocation_public_key, \
         binding_cbor = excluded.binding_cbor, binding_seq = excluded.binding_seq, \
         revocation_cbor = excluded.revocation_cbor, \
         mailbox_disabled = excluded.mailbox_disabled, \
         mailbox_next_seq = excluded.mailbox_next_seq, \
         mailbox_bytes = excluded.mailbox_bytes",
    )
    .bind(id.as_slice())
    .bind(r.pk.as_slice())
    .bind(r.rev_pk.as_slice())
    .bind(r.binding_cbor.as_slice())
    .bind(r.binding_seq)
    .bind(r.revocation_cbor.as_deref())
    .bind(r.disabled)
    .bind(r.next_seq)
    .bind(r.bytes)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn delete_mailbox_all(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &IdentityId,
) -> Result<()> {
    sqlx::query("DELETE FROM mailbox_rows WHERE identity_id = $1")
        .bind(owner.as_slice())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn delete_mailbox_exact(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &IdentityId,
    seq: u64,
) -> Result<()> {
    sqlx::query("DELETE FROM mailbox_rows WHERE identity_id = $1 AND seq = $2")
        .bind(owner.as_slice())
        .bind(seq as i64)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn delete_mailbox_acked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &IdentityId,
    upto: u64,
) -> Result<()> {
    sqlx::query("DELETE FROM mailbox_rows WHERE identity_id = $1 AND seq <= $2")
        .bind(owner.as_slice())
        .bind(upto as i64)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn insert_mailbox_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    new_rows: &[(IdentityId, u64)],
) -> Result<()> {
    // Resolve against live memory; rows appended then wiped pre-persist
    // (e.g. disabled mailbox) are already gone and must not be resurrected.
    let mut vals: Vec<(Vec<u8>, i64, i64, i64, Vec<u8>, Vec<u8>)> = Vec::new();
    for (owner, seq) in new_rows {
        if let Some(row) = home.mailboxes.get(owner).and_then(|b| b.rows.get(seq)) {
            let padded = row
                .inner
                .encode_padded()
                .map_err(|_| PersistError::Corrupt("inner encode"))?;
            vals.push((
                owner.to_vec(),
                row.seq as i64,
                row.received_at as i64,
                row.expires_at as i64,
                padded,
                row.inner.idempotency_token.to_vec(),
            ));
        }
    }
    if vals.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO mailbox_rows \
         (identity_id, seq, received_at, expires_at, inner_padded, idempotency_token) ",
    );
    qb.push_values(vals.iter(), |mut b, v| {
        b.push_bind(v.0.as_slice())
            .push_bind(v.1)
            .push_bind(v.2)
            .push_bind(v.3)
            .push_bind(v.4.as_slice())
            .push_bind(v.5.as_slice());
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn delete_tokens_owner(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &IdentityId,
) -> Result<()> {
    sqlx::query("DELETE FROM tokens WHERE mailbox = $1")
        .bind(owner.as_slice())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn upsert_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    token: &[u8; KEY_LEN],
) -> Result<()> {
    // Tokens removed pre-persist (mailbox disabled) are already covered by the
    // owner delete and must not be resurrected.
    let Some((_, kind)) = home.tokens.iter().find(|(k, _)| k == token) else {
        return Ok(());
    };
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
                 VALUES ($1, 1, $2, $3, $4, $5, NULL, NULL, 0) \
                 ON CONFLICT (token) DO UPDATE SET \
                 mailbox = excluded.mailbox, expires_at = excluded.expires_at, \
                 burned = excluded.burned, reserved_prekey = excluded.reserved_prekey",
            )
            .bind(token.as_slice())
            .bind(mailbox.as_slice())
            .bind(*expires_at as i64)
            .bind(*burned)
            .bind(reserved_prekey.as_deref())
            .execute(&mut **tx)
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
                 VALUES ($1, 2, $2, NULL, FALSE, NULL, $3, $4, $5) \
                 ON CONFLICT (token) DO UPDATE SET \
                 mailbox = excluded.mailbox, grace_until = excluded.grace_until, \
                 window_start = excluded.window_start, window_count = excluded.window_count",
            )
            .bind(token.as_slice())
            .bind(mailbox.as_slice())
            .bind(grace_until.map(|g| g as i64))
            .bind(*window_start as i64)
            .bind(*window_count as i32)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn rewrite_prekeys(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    owner: &IdentityId,
) -> Result<()> {
    sqlx::query("DELETE FROM prekeys WHERE identity_id = $1")
        .bind(owner.as_slice())
        .execute(&mut **tx)
        .await?;
    let Some(queue) = home.prekeys.get(owner) else {
        return Ok(());
    };
    if queue.is_empty() {
        return Ok(());
    }
    // One statement for the whole queue: restock bursts stay at 2 round trips.
    let mut qb = sqlx::QueryBuilder::new("INSERT INTO prekeys (identity_id, blob) ");
    qb.push_values(queue.iter(), |mut b, blob| {
        b.push_bind(owner.as_slice()).push_bind(blob.as_slice());
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn rewrite_memberships(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    owner: &IdentityId,
) -> Result<()> {
    sqlx::query("DELETE FROM memberships WHERE identity_id = $1")
        .bind(owner.as_slice())
        .execute(&mut **tx)
        .await?;
    let Some(list) = home.memberships.get(owner) else {
        return Ok(());
    };
    if list.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new("INSERT INTO memberships (identity_id, group_id) ");
    qb.push_values(list.iter(), |mut b, gid| {
        b.push_bind(owner.as_slice()).push_bind(gid.0.as_slice());
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn upsert_peer(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    peer: &ServerId,
) -> Result<()> {
    let Some(p) = home.peers.get(peer) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO peers \
         (server_id, bundle_cbor, refused, last_recv_counter, last_send_counter) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (server_id) DO UPDATE SET \
         bundle_cbor = excluded.bundle_cbor, refused = excluded.refused, \
         last_recv_counter = excluded.last_recv_counter, \
         last_send_counter = excluded.last_send_counter",
    )
    .bind(peer.as_slice())
    .bind(p.bundle.encode())
    .bind(p.refused)
    .bind(p.last_rx as i64)
    .bind(p.next_tx as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn rewrite_outbound(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
) -> Result<()> {
    sqlx::query("DELETE FROM outbound").execute(&mut **tx).await?;
    if home.outbound.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO outbound (dest, outer_cbor, enqueued_at, next_attempt, backoff_secs) ",
    );
    qb.push_values(home.outbound.iter(), |mut b, row| {
        b.push_bind(row.dest.as_slice())
            .push_bind(row.outer.encode())
            .push_bind(row.enqueued_at as i64)
            .push_bind(row.next_attempt as i64)
            .push_bind(row.backoff_secs as i64);
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn insert_seen_new(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    home: &HomeServer,
    new_encs: &[[u8; KEY_LEN]],
) -> Result<()> {
    let mut vals: Vec<(Vec<u8>, i64)> = Vec::new();
    for enc in new_encs {
        if let Some(seq) = home.seen_enc.get(enc) {
            vals.push((enc.to_vec(), *seq as i64));
        }
    }
    if vals.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new("INSERT INTO hpke_seen (enc, seq, seen_at) ");
    qb.push_values(vals.iter(), |mut b, v| {
        b.push_bind(v.0.as_slice()).push_bind(v.1).push_bind(0_i64);
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn rewrite_seen(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, home: &HomeServer) -> Result<()> {
    sqlx::query("DELETE FROM hpke_seen").execute(&mut **tx).await?;
    if home.seen_enc.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new("INSERT INTO hpke_seen (enc, seq, seen_at) ");
    qb.push_values(home.seen_enc.iter(), |mut b, (enc, seq)| {
        b.push_bind(enc.as_slice()).push_bind(*seq as i64).push_bind(0_i64);
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn upsert_group(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    groups: &GroupHost,
    gid: &GroupId,
) -> Result<()> {
    let Some(g) = groups.groups.get(gid) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO groups (group_id, next_seq, bytes) VALUES ($1, $2, $3) \
         ON CONFLICT (group_id) DO UPDATE SET \
         next_seq = excluded.next_seq, bytes = excluded.bytes",
    )
    .bind(gid.0.as_slice())
    .bind(g.next_seq as i64)
    .bind(g.bytes as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_cred(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    groups: &GroupHost,
    gid: &GroupId,
    cid: &[u8; KEY_LEN],
) -> Result<()> {
    let Some(rec) = groups.groups.get(gid).and_then(|g| g.creds.get(cid)) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO group_creds \
         (group_id, credential_id, credential_secret, live, signing_pk, \
          fanout_capability, fanout_hpke) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (group_id, credential_id) DO UPDATE SET \
         credential_secret = excluded.credential_secret, live = excluded.live, \
         signing_pk = excluded.signing_pk, \
         fanout_capability = excluded.fanout_capability, \
         fanout_hpke = excluded.fanout_hpke",
    )
    .bind(gid.0.as_slice())
    .bind(cid.as_slice())
    .bind(rec.secret.as_slice())
    .bind(rec.live)
    .bind(rec.signing_pk.as_ref().map(|p| p.as_slice()))
    .bind(rec.fanout.as_ref().map(|f| f.delivery_capability.as_slice()))
    .bind(rec.fanout.as_ref().map(|f| f.home_hpke_public.as_slice()))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn rewrite_invites(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    groups: &GroupHost,
    gid: &GroupId,
) -> Result<()> {
    sqlx::query("DELETE FROM group_invites WHERE group_id = $1")
        .bind(gid.0.as_slice())
        .execute(&mut **tx)
        .await?;
    let Some(g) = groups.groups.get(gid) else {
        return Ok(());
    };
    if g.invites.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO group_invites (group_id, nonce, expires_at, invitee_binding) ",
    );
    qb.push_values(g.invites.iter(), |mut b, (nonce, inv)| {
        b.push_bind(gid.0.as_slice())
            .push_bind(nonce.as_slice())
            .push_bind(inv.expires_at as i64)
            .push_bind(inv.invitee_binding.as_ref().map(|x| x.as_slice()));
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn rewrite_pending(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    groups: &GroupHost,
    gid: &GroupId,
) -> Result<()> {
    sqlx::query("DELETE FROM group_pending WHERE group_id = $1")
        .bind(gid.0.as_slice())
        .execute(&mut **tx)
        .await?;
    let Some(g) = groups.groups.get(gid) else {
        return Ok(());
    };
    if g.pending.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO group_pending \
         (group_id, pending_id, credential_id, credential_secret, expires_at) ",
    );
    qb.push_values(g.pending.iter(), |mut b, (pid, p)| {
        b.push_bind(gid.0.as_slice())
            .push_bind(pid.as_slice())
            .push_bind(p.cred.credential_id.as_slice())
            .push_bind(p.cred.credential_secret.as_slice())
            .push_bind(p.expires_at as i64);
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn insert_stream_new(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    groups: &GroupHost,
    new_rows: &[(GroupId, u64)],
) -> Result<()> {
    let mut vals: Vec<(Vec<u8>, i64, i16, Vec<u8>, i64)> = Vec::new();
    for (gid, seq) in new_rows {
        if let Some(row) = groups
            .groups
            .get(gid)
            .and_then(|g| g.stream.iter().find(|r| r.seq == *seq))
        {
            vals.push((
                gid.0.to_vec(),
                row.seq as i64,
                row.type_ as i16,
                row.body.clone(),
                row.received_at as i64,
            ));
        }
    }
    if vals.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO group_stream (group_id, seq, \"type\", body, received_at) ",
    );
    qb.push_values(vals.iter(), |mut b, v| {
        b.push_bind(v.0.as_slice())
            .push_bind(v.1)
            .push_bind(v.2)
            .push_bind(v.3.as_slice())
            .push_bind(v.4);
    });
    qb.build().execute(&mut **tx).await?;
    Ok(())
}

async fn upsert_file(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    groups: &GroupHost,
    token: &[u8; KEY_LEN],
) -> Result<()> {
    let Some(slot) = groups.files.get(token) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO group_files \
         (fetch_token, group_id, owner_cred, size_bytes, expires_at, bytes) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (fetch_token) DO UPDATE SET \
         group_id = excluded.group_id, owner_cred = excluded.owner_cred, \
         size_bytes = excluded.size_bytes, expires_at = excluded.expires_at, \
         bytes = excluded.bytes",
    )
    .bind(token.as_slice())
    .bind(slot.group_id.0.as_slice())
    .bind(slot.owner.as_slice())
    .bind(slot.size as i32)
    .bind(slot.expires_at as i64)
    .bind(slot.bytes.as_deref())
    .execute(&mut **tx)
    .await?;
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
