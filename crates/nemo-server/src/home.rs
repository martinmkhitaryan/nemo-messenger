//! Home-server mailbox log. No HTTP. HPKE open is this process; clients never hold the secret.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use nemo_wire::envelope::{InnerEnvelope, OuterEnvelope, TtlBucket};
use nemo_wire::hpke::{self, open_outer, HpkeKeypair};
use nemo_wire::ids::{self, IdentityId, ServerId, KEY_LEN};
use nemo_wire::{
    allowed_intro_ttl, ContactCard, HomeServerBinding, MailboxOwnerAuth, RevocationStatement,
    ServerBundle, SigningKey, VerifyingKey, INTRO_TTL_30_MIN,
};

use crate::group::GroupId;
use rand::Rng;
use subtle::ConstantTimeEq;

use crate::error::{Result, ServerError};
use crate::federation::{Enqueue, OutboundRow, PeerState};

pub const FETCH_LIMIT_MAX: u64 = 256;
pub const SHARE_TTL_DEFAULT: u64 = INTRO_TTL_30_MIN;
pub const HPKE_ENC_WINDOW: usize = 1024;
pub const CONTACT_GRACE_SECS: u64 = 72 * 3600;
/// Contact capability idle GC fuse (ADR-0008): a non-newest
/// cap whose last use (`window_start`) is older than this is dead.
pub const CONTACT_CAP_IDLE_SECS: u64 = 30 * 24 * 3600;
/// Share-token GC buffers (ADR-0008), anchored on `expires_at`
/// so no schema change is needed: TTLs are at most 1h, so `expires_at + buffer`
/// is ~buffer after creation.
pub const SHARE_EXPIRED_BUFFER_SECS: u64 = 24 * 3600;
pub const SHARE_RESERVED_BUFFER_SECS: u64 = 7 * 24 * 3600;
/// Periodic token sweeper interval for `pump_loop`.
pub const TOKEN_SWEEP_INTERVAL_SECS: u64 = 24 * 3600;
pub const OWNER_MAX_AGE: u64 = MailboxOwnerAuth::MAX_AGE_SECS;
pub const RATE_PER_MIN: usize = 30;
pub const MAILBOX_MAX_BYTES: u64 = 500 * 1024 * 1024;
pub const MAILBOX_MAX_AGE_SECS: u64 = 14 * 24 * 3600;

#[derive(Clone, Debug)]
pub struct Limits {
    pub mailbox_max_bytes: u64,
    pub mailbox_max_age_secs: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            mailbox_max_bytes: MAILBOX_MAX_BYTES,
            mailbox_max_age_secs: MAILBOX_MAX_AGE_SECS,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StoredEnvelope {
    pub seq: u64,
    pub received_at: u64,
    pub expires_at: u64,
    pub inner: InnerEnvelope,
}

pub(crate) struct Mailbox {
    pub owner_pk: [u8; KEY_LEN],
    pub next_seq: u64,
    pub rows: BTreeMap<u64, StoredEnvelope>,
    pub idempotency: HashMap<[u8; KEY_LEN], u64>,
    pub bytes: u64,
    pub disabled: bool,
}

pub(crate) struct DirectoryRow {
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub binding: HomeServerBinding,
    pub binding_seq: u64,
    pub revocation: Option<RevocationStatement>,
}

#[derive(Clone, Debug)]
pub struct DiscoveryRow {
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub binding: HomeServerBinding,
    pub revocation: Option<RevocationStatement>,
}

pub(crate) enum TokenKind {
    Share {
        mailbox: IdentityId,
        expires_at: u64,
        burned: bool,
        reserved_prekey: Option<Vec<u8>>,
    },
    Contact {
        mailbox: IdentityId,
        grace_until: Option<u64>,
        window_start: u64,
        window_count: usize,
    },
}

pub struct HomeServer {
    pub(crate) hpke: HpkeKeypair,
    pub(crate) sign: SigningKey,
    pub(crate) bundle: ServerBundle,
    pub now: u64,
    pub(crate) limits: Limits,
    pub(crate) mailboxes: HashMap<IdentityId, Mailbox>,
    pub(crate) directory: HashMap<IdentityId, DirectoryRow>,
    pub(crate) memberships: HashMap<IdentityId, Vec<GroupId>>,
    pub(crate) tokens: Vec<([u8; KEY_LEN], TokenKind)>,
    pub(crate) prekeys: HashMap<IdentityId, VecDeque<Vec<u8>>>,
    pub(crate) seen_enc: HashMap<[u8; KEY_LEN], u64>,
    pub(crate) peers: HashMap<ServerId, PeerState>,
    pub(crate) outbound: Vec<OutboundRow>,
    /// Last wall-clock (`now`) token-GC sweep. In-memory only, so a restart
    /// sweeps on first tick and cleans legacy rows immediately.
    pub(crate) last_token_sweep: u64,
    /// Incremental-persistence journal. Mutation methods record what changed;
    /// `pg::persist` drains it and writes only those rows. Empty means the
    /// database already matches memory, so persist is a no-op.
    pub(crate) dirty: HomeDirty,
}

/// What changed in [`HomeServer`] since the last successful [`crate::pg::persist`].
///
/// Granularity is chosen so every entry maps to a small exact statement:
/// single-row upserts/deletes, or a per-owner rewrite for the small
/// queue/set tables. Drain-time lookups run against live memory, so entries
/// whose rows are already gone (e.g. appended then disabled before persist)
/// are skipped instead of resurrected.
#[derive(Default)]
pub(crate) struct HomeDirty {
    /// `server_identity` row changed (`rebind_host`).
    pub server_identity: bool,
    /// Upsert `identities` row (mailbox counters, binding, revocation, disabled flag).
    pub identities: HashSet<IdentityId>,
    /// `INSERT` these mailbox rows `(owner, seq)`.
    pub mailbox_new: Vec<(IdentityId, u64)>,
    /// `DELETE` rows with `seq <= upto`, merged to the max per owner.
    pub mailbox_acked: HashMap<IdentityId, u64>,
    /// `DELETE` these exact `(owner, seq)` rows (expiry GC).
    pub mailbox_dropped: Vec<(IdentityId, u64)>,
    /// `DELETE` all rows for these owners (mailbox disabled).
    pub mailbox_cleared: HashSet<IdentityId>,
    /// Upsert these tokens by primary key (minted or field-changed).
    pub tokens: HashSet<[u8; KEY_LEN]>,
    /// `DELETE` all tokens of these owners (mailbox disabled).
    pub tokens_dropped: HashSet<IdentityId>,
    /// `DELETE` these exact tokens (capability GC). Owner deletes stay in
    /// `tokens_dropped`; this is per-token expiry pruning.
    pub tokens_dropped_exact: HashSet<[u8; KEY_LEN]>,
    /// Rewrite these owners' prekey queues (`DELETE` + batched `INSERT`).
    pub prekeys: HashSet<IdentityId>,
    /// Rewrite these owners' membership lists.
    pub memberships: HashSet<IdentityId>,
    /// Upsert these peer rows (bundle, refused flag, S2S counters).
    pub peers: HashSet<ServerId>,
    /// Rewrite the `outbound` table (federation-only, normally tiny).
    pub outbound_dirty: bool,
    /// `INSERT` these HPKE-dedup entries (sequence looked up in memory).
    pub seen_enc_new: Vec<[u8; KEY_LEN]>,
    /// Dedup window wrapped: `DELETE` all + rewrite current content.
    pub seen_enc_cleared: bool,
}

impl HomeDirty {
    pub fn is_clean(&self) -> bool {
        !self.server_identity
            && self.identities.is_empty()
            && self.mailbox_new.is_empty()
            && self.mailbox_acked.is_empty()
            && self.mailbox_dropped.is_empty()
            && self.mailbox_cleared.is_empty()
            && self.tokens.is_empty()
            && self.tokens_dropped.is_empty()
            && self.tokens_dropped_exact.is_empty()
            && self.prekeys.is_empty()
            && self.memberships.is_empty()
            && self.peers.is_empty()
            && !self.outbound_dirty
            && self.seen_enc_new.is_empty()
            && !self.seen_enc_cleared
    }

    /// Merge a previously drained journal back after a failed persist, so no
    /// change is lost. Ack high-water marks merge to the max.
    pub fn absorb(&mut self, other: Self) {
        self.server_identity |= other.server_identity;
        self.identities.extend(other.identities);
        self.mailbox_new.extend(other.mailbox_new);
        for (owner, upto) in other.mailbox_acked {
            self.mailbox_acked
                .entry(owner)
                .and_modify(|u| *u = (*u).max(upto))
                .or_insert(upto);
        }
        self.mailbox_dropped.extend(other.mailbox_dropped);
        self.mailbox_cleared.extend(other.mailbox_cleared);
        self.tokens.extend(other.tokens);
        self.tokens_dropped.extend(other.tokens_dropped);
        self.tokens_dropped_exact.extend(other.tokens_dropped_exact);
        self.prekeys.extend(other.prekeys);
        self.memberships.extend(other.memberships);
        self.peers.extend(other.peers);
        self.outbound_dirty |= other.outbound_dirty;
        self.seen_enc_new.extend(other.seen_enc_new);
        self.seen_enc_cleared |= other.seen_enc_cleared;
    }

    pub fn ack_upto(&mut self, owner: IdentityId, upto: u64) {
        self.mailbox_acked
            .entry(owner)
            .and_modify(|u| *u = (*u).max(upto))
            .or_insert(upto);
    }
}

impl HomeServer {
    pub fn new() -> Self {
        let hpke = HpkeKeypair::generate();
        let sign = SigningKey::generate(&mut rand::rng());
        let bundle = ServerBundle::sign(
            &sign,
            ServerBundle {
                server_id: hpke.server_id(),
                server_hpke_public_key: hpke.public,
                server_sign_public_key: sign.verifying_key().to_bytes(),
                host: "local".into(),
                s2s_port: 8443,
                signature: [0; 64],
            },
        )
        .expect("generated bundle");
        Self {
            hpke,
            sign,
            bundle,
            now: 1_700_000_000,
            limits: Limits::default(),
            mailboxes: HashMap::new(),
            directory: HashMap::new(),
            memberships: HashMap::new(),
            tokens: Vec::new(),
            prekeys: HashMap::new(),
            seen_enc: HashMap::new(),
            peers: HashMap::new(),
            outbound: Vec::new(),
            last_token_sweep: 0,
            dirty: HomeDirty::default(),
        }
    }

    pub fn with_limits(limits: Limits) -> Self {
        let mut s = Self::new();
        s.limits = limits;
        s
    }

    /// Rebuild the advertised bundle (operator hostname / S2S port).
    pub fn rebind_host(&mut self, host: String, s2s_port: u16) {
        self.bundle = ServerBundle::sign(
            &self.sign,
            ServerBundle {
                server_id: self.hpke.server_id(),
                server_hpke_public_key: self.hpke.public,
                server_sign_public_key: self.sign.verifying_key().to_bytes(),
                host,
                s2s_port: s2s_port as u64,
                signature: [0; 64],
            },
        )
        .expect("rebind bundle");
        self.dirty.server_identity = true;
    }

    pub fn advertise(host: impl Into<String>, s2s_port: u16) -> Self {
        let mut s = Self::new();
        s.rebind_host(host.into(), s2s_port);
        s
    }

    pub(crate) fn from_identity(
        hpke: HpkeKeypair,
        sign: SigningKey,
        host: String,
        s2s_port: u16,
    ) -> Self {
        let bundle = ServerBundle::sign(
            &sign,
            ServerBundle {
                server_id: hpke.server_id(),
                server_hpke_public_key: hpke.public,
                server_sign_public_key: sign.verifying_key().to_bytes(),
                host,
                s2s_port: s2s_port as u64,
                signature: [0; 64],
            },
        )
        .expect("stored bundle");
        Self {
            hpke,
            sign,
            bundle,
            now: 1_700_000_000,
            limits: Limits::default(),
            mailboxes: HashMap::new(),
            directory: HashMap::new(),
            memberships: HashMap::new(),
            tokens: Vec::new(),
            prekeys: HashMap::new(),
            seen_enc: HashMap::new(),
            peers: HashMap::new(),
            outbound: Vec::new(),
            last_token_sweep: 0,
            dirty: HomeDirty::default(),
        }
    }

    pub fn hpke_public(&self) -> [u8; KEY_LEN] {
        self.hpke.public
    }

    pub fn server_id(&self) -> ServerId {
        self.hpke.server_id()
    }

    /// Number of capability tokens held in memory (operator deploy check).
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn bundle(&self) -> &ServerBundle {
        &self.bundle
    }

    pub fn sign_public(&self) -> [u8; KEY_LEN] {
        self.sign.verifying_key().to_bytes()
    }

    pub(crate) fn signing_key(&self) -> &SigningKey {
        &self.sign
    }

    pub fn peer_id_for_sign_key(&self, pk: &[u8; KEY_LEN]) -> Option<ServerId> {
        self.peers.iter().find_map(|(id, p)| {
            (p.bundle.server_sign_public_key == *pk).then_some(*id)
        })
    }

    pub fn peer_refused(&self, id: ServerId) -> bool {
        self.peers.get(&id).is_some_and(|p| p.refused)
    }

    /// Record a federation peer change (bundle, refused flag, S2S counters).
    pub(crate) fn mark_peer_dirty(&mut self, peer: ServerId) {
        self.dirty.peers.insert(peer);
    }

    /// Record an outbound-queue change (push, remove, backoff).
    pub(crate) fn mark_outbound_dirty(&mut self) {
        self.dirty.outbound_dirty = true;
    }

    /// Alice → her home: same-server ingest, else outbound queue.
    pub fn enqueue(&mut self, outer: OuterEnvelope) -> Result<Enqueue> {
        if outer.destination_server_id == self.server_id() {
            return Ok(Enqueue::Local(self.ingest(&outer)?));
        }
        self.outbound.push(OutboundRow {
            dest: outer.destination_server_id,
            outer,
            enqueued_at: self.now,
            next_attempt: self.now,
            backoff_secs: 1,
        });
        self.dirty.outbound_dirty = true;
        Ok(Enqueue::Queued)
    }

    /// Phase 4: A verifies the registering identity before accepting an outbound.
    pub fn enqueue_from_owner(
        &mut self,
        owner: IdentityId,
        auth: &MailboxOwnerAuth,
        now_unix: u64,
        outer: OuterEnvelope,
    ) -> Result<Enqueue> {
        self.check_owner(owner, auth, now_unix)?;
        self.require_mailbox(owner)?;
        self.enqueue(outer)
    }

    pub fn outbound_len(&self) -> usize {
        self.outbound.len()
    }

    pub fn defer_outbound(&mut self, until: u64) {
        for row in &mut self.outbound {
            row.next_attempt = until;
        }
        if !self.outbound.is_empty() {
            self.dirty.outbound_dirty = true;
        }
    }

    pub fn sign_rotate(
        &self,
        new_public_key: [u8; KEY_LEN],
        seq: u64,
    ) -> Result<nemo_wire::ServerSignRotate> {
        Ok(nemo_wire::ServerSignRotate::sign(
            &self.sign,
            new_public_key,
            seq,
        )?)
    }

    pub fn bundle_pin(&self, peer: ServerId) -> Option<&ServerBundle> {
        self.peers.get(&peer).map(|p| &p.bundle)
    }

    pub fn register(&mut self, identity_id: IdentityId, owner_pk: [u8; KEY_LEN]) -> Result<()> {
        if self.mailboxes.contains_key(&identity_id) {
            return Err(ServerError::AlreadyRegistered);
        }
        self.mailboxes.insert(
            identity_id,
            Mailbox {
                owner_pk,
                next_seq: 1,
                rows: BTreeMap::new(),
                idempotency: HashMap::new(),
                bytes: 0,
                disabled: false,
            },
        );
        self.dirty.identities.insert(identity_id);
        Ok(())
    }

    /// Register as spec §5.1: keys, binding, and the card's share token.
    pub fn register_from_card(&mut self, card: &ContactCard) -> Result<IdentityId> {
        card.verify(self.now)?;
        if card.binding.server_id != self.server_id() {
            return Err(ServerError::Denied);
        }
        let id = card.identity_id();
        self.register(id, card.identity_public_key)?;
        self.directory.insert(
            id,
            DirectoryRow {
                identity_public_key: card.identity_public_key,
                revocation_public_key: card.revocation_public_key,
                binding: card.binding.clone(),
                binding_seq: card.binding.seq,
                revocation: None,
            },
        );
        self.register_share_token(id, card.share_token, None)?;
        Ok(id)
    }

    pub fn discovery(&self, identity_id: IdentityId) -> Result<DiscoveryRow> {
        match self.directory.get(&identity_id) {
            Some(row) => Ok(DiscoveryRow {
                identity_public_key: row.identity_public_key,
                revocation_public_key: row.revocation_public_key,
                binding: row.binding.clone(),
                revocation: row.revocation.clone(),
            }),
            None => {
                self.dummy_work();
                Err(ServerError::Denied)
            }
        }
    }

    /// Higher `seq` on another server disables this mailbox (phase 1 §5.3).
    pub fn observe_binding(
        &mut self,
        identity_id: IdentityId,
        binding: &HomeServerBinding,
    ) -> Result<()> {
        let pk = match self.directory.get(&identity_id) {
            Some(row) => row.identity_public_key,
            None => {
                let box_ = self
                    .mailboxes
                    .get(&identity_id)
                    .ok_or(ServerError::Denied)?;
                box_.owner_pk
            }
        };
        let vk = VerifyingKey::from_bytes(&pk).map_err(|_| ServerError::Denied)?;
        if ids::identity_id(&pk) != identity_id {
            return Err(ServerError::Denied);
        }
        binding
            .verify(&vk, self.now)
            .map_err(|_| ServerError::Denied)?;
        let stored_seq = self
            .directory
            .get(&identity_id)
            .map(|r| r.binding_seq)
            .unwrap_or(0);
        if binding.seq <= stored_seq {
            return Ok(());
        }
        if let Some(row) = self.directory.get_mut(&identity_id) {
            row.binding = binding.clone();
            row.binding_seq = binding.seq;
        } else {
            self.directory.insert(
                identity_id,
                DirectoryRow {
                    identity_public_key: pk,
                    revocation_public_key: [0u8; KEY_LEN],
                    binding: binding.clone(),
                    binding_seq: binding.seq,
                    revocation: None,
                },
            );
        }
        self.dirty.identities.insert(identity_id);
        if binding.server_id != self.server_id() {
            self.disable_mailbox(identity_id);
        }
        Ok(())
    }

    pub fn note_group_membership(&mut self, identity_id: IdentityId, group_id: GroupId) {
        let list = self.memberships.entry(identity_id).or_default();
        if !list.contains(&group_id) {
            list.push(group_id);
            self.dirty.memberships.insert(identity_id);
        }
    }

    /// Wipe mailbox and keys; return hosted group ids so the host can append `0x13`.
    pub fn ingest_revocation(&mut self, stmt: &RevocationStatement) -> Result<Vec<GroupId>> {
        let row = self
            .directory
            .get(&stmt.identity_id)
            .ok_or(ServerError::Denied)?;
        let pk = RevocationStatement::verifying_key(&row.revocation_public_key)
            .map_err(|_| ServerError::Denied)?;
        stmt.verify(&pk).map_err(|_| ServerError::Denied)?;
        if stmt.identity_id != ids::identity_id(&row.identity_public_key) {
            return Err(ServerError::Denied);
        }
        if let Some(row) = self.directory.get_mut(&stmt.identity_id) {
            row.revocation = Some(stmt.clone());
        }
        self.dirty.identities.insert(stmt.identity_id);
        self.disable_mailbox(stmt.identity_id);
        let removed = self.memberships.remove(&stmt.identity_id);
        if removed.is_some() {
            self.dirty.memberships.insert(stmt.identity_id);
        }
        Ok(removed.unwrap_or_default())
    }

    pub fn mailbox_disabled(&self, owner: IdentityId) -> bool {
        self.mailboxes.get(&owner).is_some_and(|m| m.disabled)
    }

    /// Highest assigned seq, or 0 if the mailbox is empty / missing.
    pub fn mailbox_head(&self, owner: IdentityId) -> u64 {
        self.mailboxes
            .get(&owner)
            .map(|m| m.next_seq.saturating_sub(1))
            .unwrap_or(0)
    }

    pub fn mint_share_token(
        &mut self,
        owner: IdentityId,
        ttl_secs: Option<u64>,
    ) -> Result<[u8; KEY_LEN]> {
        let token = random_token();
        self.register_share_token(owner, token, ttl_secs)?;
        Ok(token)
    }

    /// Register the share token already printed on a contact card.
    pub fn register_share_token(
        &mut self,
        owner: IdentityId,
        token: [u8; KEY_LEN],
        ttl_secs: Option<u64>,
    ) -> Result<()> {
        self.require_mailbox(owner)?;
        let ttl = ttl_secs.unwrap_or(SHARE_TTL_DEFAULT);
        if !allowed_intro_ttl(ttl) {
            return Err(ServerError::Denied);
        }
        self.tokens.push((
            token,
            TokenKind::Share {
                mailbox: owner,
                expires_at: self.now + ttl,
                burned: false,
                reserved_prekey: None,
            },
        ));
        self.dirty.tokens.insert(token);
        Ok(())
    }

    pub fn mint_contact_capability(&mut self, owner: IdentityId) -> Result<[u8; KEY_LEN]> {
        self.require_mailbox(owner)?;
        // Opportunistic prune (amortized, no new jobs): sweep this mailbox's
        // dead tokens before minting, so steady-state stays bounded.
        self.sweep_dead_tokens_for(owner);
        let token = random_token();
        self.tokens.push((
            token,
            TokenKind::Contact {
                mailbox: owner,
                grace_until: None,
                window_start: self.now,
                window_count: 0,
            },
        ));
        self.dirty.tokens.insert(token);
        Ok(token)
    }

    /// Old capability stays valid for 72 hours after the owner confirms rotation.
    pub fn rotate_contact(
        &mut self,
        owner: IdentityId,
        old: [u8; KEY_LEN],
        confirm: bool,
    ) -> Result<[u8; KEY_LEN]> {
        let new = self.mint_contact_capability(owner)?;
        let grace = self.now + CONTACT_GRACE_SECS;
        let mut touched_old = false;
        if confirm {
            if let Some(TokenKind::Contact {
                mailbox,
                grace_until,
                ..
            }) = self.token_mut(&old)
            {
                if *mailbox == owner {
                    *grace_until = Some(grace);
                    touched_old = true;
                }
            }
        }
        if touched_old {
            self.dirty.tokens.insert(old);
        }
        Ok(new)
    }

    pub fn publish_prekey(&mut self, owner: IdentityId, blob: Vec<u8>) -> Result<()> {
        self.require_mailbox(owner)?;
        self.prekeys.entry(owner).or_default().push_back(blob);
        self.dirty.prekeys.insert(owner);
        Ok(())
    }

    /// Does not burn the share token. Reserves at most one prekey for that token.
    pub fn fetch_prekey(&mut self, token: [u8; KEY_LEN]) -> Result<Vec<u8>> {
        let now = self.now;
        let mailbox = match self.token(&token) {
            Some(TokenKind::Share {
                mailbox,
                expires_at,
                burned,
                reserved_prekey,
            }) if !*burned && now < *expires_at => {
                if let Some(blob) = reserved_prekey {
                    return Ok(blob.clone());
                }
                *mailbox
            }
            _ => {
                self.dummy_work();
                return Err(ServerError::Denied);
            }
        };
        let blob = self
            .prekeys
            .get_mut(&mailbox)
            .and_then(|q| q.pop_front())
            .ok_or(ServerError::Denied)?;
        if let Some(TokenKind::Share {
            reserved_prekey, ..
        }) = self.token_mut(&token)
        {
            *reserved_prekey = Some(blob.clone());
        }
        self.dirty.prekeys.insert(mailbox);
        self.dirty.tokens.insert(token);
        Ok(blob)
    }

    /// Server B: open HPKE and append. Returns mailbox `seq`.
    pub fn ingest(&mut self, outer: &OuterEnvelope) -> Result<u64> {
        let enc = hpke::encap_key(&outer.hpke_ciphertext)?;
        if let Some(&seq) = self.seen_enc.get(&enc) {
            return Ok(seq);
        }
        let inner = open_outer(&self.hpke, outer)?;
        let seq = self.append_inner(inner)?;
        if self.seen_enc.len() >= HPKE_ENC_WINDOW {
            self.seen_enc.clear();
            self.dirty.seen_enc_cleared = true;
        }
        self.seen_enc.insert(enc, seq);
        if !self.dirty.seen_enc_cleared {
            self.dirty.seen_enc_new.push(enc);
        }
        Ok(seq)
    }

    pub fn append_inner(&mut self, inner: InnerEnvelope) -> Result<u64> {
        let token = inner.delivery_capability;
        let mailbox = self.authorize_append(&token)?;
        let box_ = self
            .mailboxes
            .get_mut(&mailbox)
            .ok_or(ServerError::Denied)?;
        if let Some(&seq) = box_.idempotency.get(&inner.idempotency_token) {
            return Ok(seq);
        }
        let size = inner.encode_padded()?.len() as u64;
        let expires_at =
            envelope_expiry(self.now, inner.ttl_bucket, self.limits.mailbox_max_age_secs);
        let seq = box_.next_seq;
        box_.next_seq += 1;
        box_.idempotency.insert(inner.idempotency_token, seq);
        box_.bytes += size;
        box_.rows.insert(
            seq,
            StoredEnvelope {
                seq,
                received_at: self.now,
                expires_at,
                inner,
            },
        );
        self.dirty.mailbox_new.push((mailbox, seq));
        self.dirty.identities.insert(mailbox);
        self.burn_if_share(&token);
        self.gc_mailbox(mailbox);
        Ok(seq)
    }

    pub fn fetch(
        &mut self,
        owner: IdentityId,
        auth: &MailboxOwnerAuth,
        now_unix: u64,
    ) -> Result<Vec<StoredEnvelope>> {
        self.check_owner(owner, auth, now_unix)?;
        if auth.limit == 0 || auth.limit > FETCH_LIMIT_MAX {
            return Err(ServerError::FetchLimit);
        }
        self.gc_mailbox(owner);
        let box_ = self.mailboxes.get(&owner).ok_or(ServerError::OwnerAuth)?;
        Ok(box_
            .rows
            .values()
            .filter(|r| r.seq > auth.cursor && r.expires_at > self.now)
            .take(auth.limit as usize)
            .cloned()
            .collect())
    }

    pub fn ack(&mut self, owner: IdentityId, auth: &MailboxOwnerAuth, now_unix: u64) -> Result<()> {
        self.check_owner(owner, auth, now_unix)?;
        let box_ = self
            .mailboxes
            .get_mut(&owner)
            .ok_or(ServerError::OwnerAuth)?;
        let up_to = auth.cursor;
        let drop: Vec<u64> = box_.rows.keys().copied().filter(|&s| s <= up_to).collect();
        if drop.is_empty() {
            return Ok(());
        }
        for seq in drop {
            if let Some(row) = box_.rows.remove(&seq) {
                box_.bytes = box_.bytes.saturating_sub(
                    row.inner
                        .encode_padded()
                        .map(|b| b.len() as u64)
                        .unwrap_or(0),
                );
            }
        }
        self.dirty.ack_upto(owner, up_to);
        self.dirty.identities.insert(owner);
        Ok(())
    }

    pub(crate) fn check_owner(
        &self,
        owner: IdentityId,
        auth: &MailboxOwnerAuth,
        now_unix: u64,
    ) -> Result<()> {
        let box_ = self.mailboxes.get(&owner).ok_or(ServerError::OwnerAuth)?;
        if box_.disabled {
            return Err(ServerError::OwnerAuth);
        }
        if auth.mailbox_hint.as_slice() != owner.as_slice() {
            return Err(ServerError::OwnerAuth);
        }
        let pk = VerifyingKey::from_bytes(&box_.owner_pk).map_err(|_| ServerError::OwnerAuth)?;
        auth.verify(&pk, now_unix)
            .map_err(|_| ServerError::OwnerAuth)
    }

    fn disable_mailbox(&mut self, owner: IdentityId) {
        if let Some(box_) = self.mailboxes.get_mut(&owner) {
            let seqs: Vec<u64> = box_.rows.keys().copied().collect();
            for seq in seqs {
                remove_row(box_, seq);
            }
            box_.disabled = true;
        }
        self.prekeys.remove(&owner);
        self.tokens.retain(|(_, kind)| match kind {
            TokenKind::Share { mailbox, .. } | TokenKind::Contact { mailbox, .. } => {
                *mailbox != owner
            }
        });
        self.dirty.mailbox_cleared.insert(owner);
        self.dirty.tokens_dropped.insert(owner);
        self.dirty.prekeys.insert(owner);
        self.dirty.identities.insert(owner);
    }

    fn authorize_append(&mut self, token: &[u8; KEY_LEN]) -> Result<IdentityId> {
        let now = self.now;
        let key = *token;
        let mut touched_window = false;
        let mailbox = match self.token_mut(token) {
            Some(TokenKind::Share {
                mailbox,
                expires_at,
                burned,
                ..
            }) if !*burned && now < *expires_at => *mailbox,
            Some(TokenKind::Contact {
                mailbox,
                grace_until,
                window_start,
                window_count,
            }) => {
                if grace_until.is_some_and(|g| now >= g) {
                    self.dummy_work();
                    return Err(ServerError::Denied);
                }
                if now.saturating_sub(*window_start) >= 60 {
                    *window_start = now;
                    *window_count = 0;
                }
                if *window_count >= RATE_PER_MIN {
                    return Err(ServerError::Denied);
                }
                *window_count += 1;
                touched_window = true;
                *mailbox
            }
            _ => {
                self.dummy_work();
                return Err(ServerError::Denied);
            }
        };
        if touched_window {
            self.dirty.tokens.insert(key);
        }
        if self.mailboxes.get(&mailbox).is_some_and(|m| m.disabled) {
            return Err(ServerError::Denied);
        }
        Ok(mailbox)
    }

    fn burn_if_share(&mut self, token: &[u8; KEY_LEN]) {
        if let Some(TokenKind::Share { burned, .. }) = self.token_mut(token) {
            if !*burned {
                *burned = true;
                self.dirty.tokens.insert(*token);
            }
        }
    }

    fn gc_mailbox(&mut self, owner: IdentityId) {
        let now = self.now;
        let max_age = self.limits.mailbox_max_age_secs;
        let max_bytes = self.limits.mailbox_max_bytes;
        let Some(box_) = self.mailboxes.get_mut(&owner) else {
            return;
        };
        let expired: Vec<u64> = box_
            .rows
            .iter()
            .filter(|(_, r)| r.expires_at <= now || now.saturating_sub(r.received_at) > max_age)
            .map(|(s, _)| *s)
            .collect();
        let mut dropped: Vec<(IdentityId, u64)> =
            expired.into_iter().map(|s| (owner, s)).collect();
        for (_, seq) in &dropped {
            remove_row(box_, *seq);
        }
        while box_.bytes > max_bytes {
            let Some((&oldest, _)) = box_.rows.iter().next() else {
                break;
            };
            remove_row(box_, oldest);
            dropped.push((owner, oldest));
        }
        if !dropped.is_empty() {
            self.dirty.mailbox_dropped.extend(dropped);
            self.dirty.identities.insert(owner);
        }
    }

    /// Periodic sweeper gate: true when `TOKEN_SWEEP_INTERVAL_SECS` elapsed
    /// since the last sweep. `last_token_sweep == 0` (fresh boot) is always due,
    /// so deploying cleans legacy rows on first tick.
    pub fn token_sweep_due(&self) -> bool {
        self.now.saturating_sub(self.last_token_sweep) >= TOKEN_SWEEP_INTERVAL_SECS
    }

    pub fn record_token_sweep(&mut self) {
        self.last_token_sweep = self.now;
    }

    /// Sweep dead capability tokens for one mailbox (opportunistic prune).
    /// Returns the number of rows removed from memory (durable via `dirty`).
    pub fn sweep_dead_tokens_for(&mut self, owner: IdentityId) -> usize {
        let survivor = self.newest_contact_idx_for(owner);
        let now = self.now;
        let dead: Vec<[u8; KEY_LEN]> = self
            .tokens
            .iter()
            .enumerate()
            .filter_map(|(i, (tok, kind))| {
                if !token_belongs_to(kind, owner) {
                    return None;
                }
                // Orphaned mailbox rows are always reclaimable.
                if self.mailboxes.get(&owner).is_none_or(|m| m.disabled) {
                    return Some(*tok);
                }
                match kind {
                    TokenKind::Share {
                        expires_at,
                        reserved_prekey,
                        ..
                    } => share_token_dead(*expires_at, reserved_prekey, now).then_some(*tok),
                    TokenKind::Contact {
                        grace_until,
                        window_start,
                        ..
                    } => {
                        if Some(i) == survivor {
                            return None;
                        }
                        // In-grace replacements stay valid: never reap early.
                        if grace_until.is_some_and(|g| now < g) {
                            return None;
                        }
                        contact_token_dead(*window_start, now).then_some(*tok)
                    }
                }
            })
            .collect();
        self.drop_tokens_exact(dead)
    }

    /// Sweep dead capability tokens for all mailboxes (periodic sweeper).
    /// Keeps the newest contact cap per mailbox, reaps idle/expired shares.
    pub fn sweep_dead_tokens(&mut self) -> usize {
        // Survivors per mailbox: newest minted (max `window_start`, tie last).
        let mut survivors: HashMap<IdentityId, usize> = HashMap::new();
        let mut best_start: HashMap<IdentityId, u64> = HashMap::new();
        for (i, (_, kind)) in self.tokens.iter().enumerate() {
            if let TokenKind::Contact {
                mailbox,
                window_start,
                ..
            } = kind
            {
                let keep = match best_start.get(mailbox) {
                    None => true,
                    Some(&s) => *window_start > s || (*window_start == s),
                };
                // Later index wins ties: Vec order is mint order in memory.
                if keep {
                    best_start.insert(*mailbox, *window_start);
                    survivors.insert(*mailbox, i);
                }
            }
        }
        let now = self.now;
        // Snapshot disabled set to avoid borrowing `mailboxes` inside retain.
        let disabled: HashSet<IdentityId> = self
            .mailboxes
            .iter()
            .filter_map(|(id, m)| m.disabled.then_some(*id))
            .collect();
        let dead: Vec<[u8; KEY_LEN]> = self
            .tokens
            .iter()
            .enumerate()
            .filter_map(|(i, (tok, kind))| {
                let owner = token_mailbox(kind);
                // Orphan or disabled owner: reclaim everything.
                if !self.mailboxes.contains_key(&owner) || disabled.contains(&owner) {
                    return Some(*tok);
                }
                match kind {
                    TokenKind::Share {
                        expires_at,
                        reserved_prekey,
                        ..
                    } => share_token_dead(*expires_at, reserved_prekey, now).then_some(*tok),
                    TokenKind::Contact {
                        grace_until,
                        window_start,
                        ..
                    } => {
                        if survivors.get(&owner).is_some_and(|&s| s == i) {
                            return None;
                        }
                        if grace_until.is_some_and(|g| now < g) {
                            return None;
                        }
                        contact_token_dead(*window_start, now).then_some(*tok)
                    }
                }
            })
            .collect();
        self.drop_tokens_exact(dead)
    }

    /// Newest contact-capability index for `owner` (mint order = Vec order,
    /// tie-broken by greatest `window_start` so reloads stay deterministic).
    fn newest_contact_idx_for(&self, owner: IdentityId) -> Option<usize> {
        let mut best: Option<(usize, u64)> = None;
        for (i, (_, kind)) in self.tokens.iter().enumerate() {
            if let TokenKind::Contact {
                mailbox,
                window_start,
                ..
            } = kind
            {
                if *mailbox != owner {
                    continue;
                }
                match best {
                    None => best = Some((i, *window_start)),
                    Some((bi, bs)) => {
                        if *window_start > bs || (*window_start == bs && i > bi) {
                            best = Some((i, *window_start));
                        }
                    }
                }
            }
        }
        best.map(|(i, _)| i)
    }

    /// Remove tokens from memory and journal per-token deletes.
    /// Upsert entries for the same tokens are dropped: `pg::persist` skips
    /// upserts for rows already gone, so they must not resurrect.
    fn drop_tokens_exact(&mut self, dead: Vec<[u8; KEY_LEN]>) -> usize {
        if dead.is_empty() {
            return 0;
        }
        let gone: HashSet<[u8; KEY_LEN]> = dead.iter().copied().collect();
        self.tokens.retain(|(tok, _)| !gone.contains(tok));
        for tok in &dead {
            self.dirty.tokens.remove(tok);
        }
        self.dirty.tokens_dropped_exact.extend(dead.iter().copied());
        dead.len()
    }

    fn require_mailbox(&self, owner: IdentityId) -> Result<()> {
        match self.mailboxes.get(&owner) {
            Some(m) if !m.disabled => Ok(()),
            _ => Err(ServerError::Denied),
        }
    }

    fn token(&self, want: &[u8; KEY_LEN]) -> Option<&TokenKind> {
        let mut found = None;
        for (k, v) in &self.tokens {
            if bool::from(k.ct_eq(want)) {
                found = Some(v);
            }
        }
        found
    }

    fn token_mut(&mut self, want: &[u8; KEY_LEN]) -> Option<&mut TokenKind> {
        let mut idx = None;
        for (i, (k, _)) in self.tokens.iter().enumerate() {
            if bool::from(k.ct_eq(want)) {
                idx = Some(i);
            }
        }
        idx.map(|i| &mut self.tokens[i].1)
    }

    fn dummy_work(&self) {
        let mut acc = 0u8;
        for (k, _) in &self.tokens {
            acc ^= k[0];
        }
        let _ = acc;
    }
}

impl Default for HomeServer {
    fn default() -> Self {
        Self::new()
    }
}

fn remove_row(box_: &mut Mailbox, seq: u64) {
    if let Some(row) = box_.rows.remove(&seq) {
        box_.bytes = box_.bytes.saturating_sub(
            row.inner
                .encode_padded()
                .map(|b| b.len() as u64)
                .unwrap_or(0),
        );
        box_.idempotency.retain(|_, s| *s != seq);
    }
}

fn envelope_expiry(now: u64, ttl: TtlBucket, max_age: u64) -> u64 {
    let cap = now.saturating_add(max_age);
    match ttl.ttl_secs() {
        Some(s) => now.saturating_add(s).min(cap),
        None => cap,
    }
}

/// Share-token death (ADR-0008): `expires_at + buffer`.
/// Reserved tokens get the 7-day retry window, all others the 1-day buffer
/// (expired unburned and burned-without-reservation collapse to the same fuse).
fn share_token_dead(expires_at: u64, reserved_prekey: &Option<Vec<u8>>, now: u64) -> bool {
    let buf = if reserved_prekey.is_some() {
        SHARE_RESERVED_BUFFER_SECS
    } else {
        SHARE_EXPIRED_BUFFER_SECS
    };
    now > expires_at.saturating_add(buf)
}

/// Contact-cap death: last use (`window_start`) older than the idle fuse.
fn contact_token_dead(window_start: u64, now: u64) -> bool {
    now.saturating_sub(window_start) > CONTACT_CAP_IDLE_SECS
}

fn token_mailbox(kind: &TokenKind) -> IdentityId {
    match kind {
        TokenKind::Share { mailbox, .. } | TokenKind::Contact { mailbox, .. } => *mailbox,
    }
}

fn token_belongs_to(kind: &TokenKind, owner: IdentityId) -> bool {
    token_mailbox(kind) == owner
}

pub(crate) fn random_token() -> [u8; KEY_LEN] {
    let mut t = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut t);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebind_keeps_keys() {
        let mut home = HomeServer::new();
        let id = home.server_id();
        let pk = home.hpke_public();
        home.rebind_host("example.invalid".into(), 9443);
        assert_eq!(home.server_id(), id);
        assert_eq!(home.hpke_public(), pk);
        assert_eq!(home.bundle().host, "example.invalid");
        assert_eq!(home.bundle().s2s_port, 9443);
        home.bundle().verify().unwrap();
    }

    #[test]
    fn from_identity_restores_server_id() {
        let src = HomeServer::new();
        let restored = HomeServer::from_identity(
            src.hpke.clone(),
            src.sign.clone(),
            "restored".into(),
            1,
        );
        assert_eq!(restored.server_id(), src.server_id());
        assert_eq!(restored.hpke_public(), src.hpke_public());
        assert_eq!(restored.bundle().host, "restored");
    }

    fn test_mailbox(home: &mut HomeServer) -> IdentityId {
        let sk = SigningKey::generate(&mut rand::rng());
        let pk = sk.verifying_key().to_bytes();
        let id = ids::identity_id(&pk);
        home.register(id, pk).unwrap();
        id
    }

    fn push_share(
        home: &mut HomeServer,
        owner: IdentityId,
        expires_at: u64,
        burned: bool,
        reserved: bool,
    ) -> [u8; KEY_LEN] {
        let tok = random_token();
        home.tokens.push((
            tok,
            TokenKind::Share {
                mailbox: owner,
                expires_at,
                burned,
                reserved_prekey: reserved.then(|| vec![1, 2, 3]),
            },
        ));
        tok
    }

    fn push_contact(
        home: &mut HomeServer,
        owner: IdentityId,
        window_start: u64,
        grace_until: Option<u64>,
    ) -> [u8; KEY_LEN] {
        let tok = random_token();
        home.tokens.push((
            tok,
            TokenKind::Contact {
                mailbox: owner,
                grace_until,
                window_start,
                window_count: 0,
            },
        ));
        tok
    }

    #[test]
    fn share_gc_expired_and_burned_buffers() {
        let mut home = HomeServer::new();
        let id = test_mailbox(&mut home);
        let base: u64 = 1_700_000_000;
        home.now = base;
        let exp = base + 1_800; // 30min TTL

        let live = push_share(&mut home, id, exp, false, false);
        let within_buf = push_share(&mut home, id, exp, false, false);
        let past_buf = push_share(&mut home, id, exp, false, false);
        let burned_fresh = push_share(&mut home, id, exp, true, false);
        let reserved_keep = push_share(&mut home, id, exp, true, true);
        let reserved_old = push_share(&mut home, id, exp, true, true);

        // live + within 1d buffer kept; past 1d reaped.
        home.now = exp + SHARE_EXPIRED_BUFFER_SECS - 10;
        assert_eq!(home.sweep_dead_tokens_for(id), 0);
        // Move past the 1d fuse but inside the 7d reserved window.
        home.now = exp + SHARE_EXPIRED_BUFFER_SECS + 10;
        let n = home.sweep_dead_tokens_for(id);
        // live/within/past/burned collapse to expired+1d; reserved survives.
        assert!(n >= 3, "expected share sweep, got {n}");
        assert!(
            home.tokens.iter().all(|(t, _)| *t != past_buf),
            "expired past buffer must go"
        );
        assert!(
            home.tokens.iter().all(|(t, _)| *t != burned_fresh),
            "burned without reservation must go after 1d"
        );
        assert!(
            home.tokens.iter().any(|(t, _)| *t == reserved_keep),
            "burned with reservation must survive 1d (7d window)"
        );
        assert!(home.dirty.tokens_dropped_exact.contains(&past_buf));

        // Reserved survives until expires + 7d.
        home.now = exp + SHARE_RESERVED_BUFFER_SECS - 10;
        assert!(home.tokens.iter().any(|(t, _)| *t == reserved_keep));
        home.now = exp + SHARE_RESERVED_BUFFER_SECS + 10;
        home.sweep_dead_tokens_for(id);
        assert!(
            home.tokens.iter().all(|(t, _)| *t != reserved_keep),
            "reserved must go after 7d"
        );
        assert!(
            home.tokens.iter().all(|(t, _)| *t != reserved_old),
            "reserved must go after 7d"
        );
        let _ = (live, within_buf);
    }

    #[test]
    fn contact_gc_newest_always_kept_and_idle() {
        let mut home = HomeServer::new();
        let id = test_mailbox(&mut home);
        let base: u64 = 1_700_000_000;
        home.now = base;
        let old = push_contact(&mut home, id, base, None);
        let mid = push_contact(&mut home, id, base + 5 * 24 * 3600, None);
        let newest = push_contact(&mut home, id, base + 10 * 24 * 3600, None);

        // All idle >30d: only newest survives.
        home.now = base + 45 * 24 * 3600;
        let n = home.sweep_dead_tokens_for(id);
        assert_eq!(n, 2);
        assert!(home.tokens.iter().any(|(t, _)| *t == newest));
        assert!(home.tokens.iter().all(|(t, _)| *t != old && *t != mid));

        // Recently used non-newest is kept.
        let mut home = HomeServer::new();
        let id = test_mailbox(&mut home);
        home.now = base;
        let _old = push_contact(&mut home, id, base, None);
        let recent = push_contact(&mut home, id, base + 40 * 24 * 3600, None);
        let _newest = push_contact(&mut home, id, base + 40 * 24 * 3600, None);
        home.now = base + 45 * 24 * 3600;
        home.sweep_dead_tokens_for(id);
        assert!(
            home.tokens.iter().any(|(t, _)| *t == recent),
            "idle <30d must be kept"
        );
    }

    #[test]
    fn contact_gc_grace_keeps_idle_but_expired_grace_goes() {
        let mut home = HomeServer::new();
        let id = test_mailbox(&mut home);
        let base: u64 = 1_700_000_000;
        home.now = base;
        let grace_kept = push_contact(&mut home, id, base, Some(base + 50 * 24 * 3600));
        let grace_dead = push_contact(&mut home, id, base, Some(base + 40 * 24 * 3600));
        let _newest = push_contact(&mut home, id, base + 44 * 24 * 3600, None);
        home.now = base + 45 * 24 * 3600;
        home.sweep_dead_tokens_for(id);
        assert!(
            home.tokens.iter().any(|(t, _)| *t == grace_kept),
            "in-grace cap must survive even when idle"
        );
        assert!(
            home.tokens.iter().all(|(t, _)| *t != grace_dead),
            "grace-expired idle cap must go"
        );
    }

    #[test]
    fn mint_prunes_opportunistically_and_sweep_gate() {
        let mut home = HomeServer::new();
        let id = test_mailbox(&mut home);
        let base: u64 = 1_700_000_000;
        home.now = base;
        let oldest = push_contact(&mut home, id, base, None);
        let _middle = push_contact(&mut home, id, base + 24 * 3600, None);
        home.now = base + 40 * 24 * 3600;
        // Mint sweeps the same mailbox first: non-survivor idle goes,
        // survivor (previous newest) stays until the next sweep.
        let newest = home.mint_contact_capability(id).unwrap();
        assert!(
            home.tokens.iter().all(|(t, _)| *t != oldest),
            "opportunistic prune must drop idle non-newest"
        );
        assert!(home.tokens.iter().any(|(t, _)| *t == newest));
        assert!(home.dirty.tokens_dropped_exact.contains(&oldest));

        // Gate: fresh boot is due, recent sweep is not.
        home.last_token_sweep = 0;
        home.now = base + 40 * 24 * 3600;
        assert!(home.token_sweep_due());
        home.record_token_sweep();
        assert!(!home.token_sweep_due());
        home.now += TOKEN_SWEEP_INTERVAL_SECS;
        assert!(home.token_sweep_due());
    }
}

