//! Home-server mailbox log. No HTTP. HPKE open is this process; clients never hold the secret.

use std::collections::{BTreeMap, HashMap, VecDeque};

use nemo_wire::envelope::{InnerEnvelope, OuterEnvelope, TtlBucket};
use nemo_wire::hpke::{self, open_outer, HpkeKeypair};
use nemo_wire::ids::{self, IdentityId, ServerId, KEY_LEN};
use nemo_wire::{
    allowed_intro_ttl, ContactCard, HomeServerBinding, MailboxOwnerAuth, RevocationStatement,
    ServerBundle, SigningKey, VerifyingKey, INTRO_TTL_30_MIN,
};

use crate::group::GroupId;
use rand::RngCore;
use subtle::ConstantTimeEq;

use crate::error::{Result, ServerError};
use crate::federation::{Enqueue, OutboundRow, PeerState};

pub const FETCH_LIMIT_MAX: u64 = 256;
pub const SHARE_TTL_DEFAULT: u64 = INTRO_TTL_30_MIN;
pub const HPKE_ENC_WINDOW: usize = 1024;
pub const CONTACT_GRACE_SECS: u64 = 72 * 3600;
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

struct Mailbox {
    owner_pk: [u8; KEY_LEN],
    next_seq: u64,
    rows: BTreeMap<u64, StoredEnvelope>,
    idempotency: HashMap<[u8; KEY_LEN], u64>,
    bytes: u64,
    disabled: bool,
}

struct DirectoryRow {
    identity_public_key: [u8; KEY_LEN],
    revocation_public_key: [u8; KEY_LEN],
    binding: HomeServerBinding,
    binding_seq: u64,
    revocation: Option<RevocationStatement>,
}

#[derive(Clone, Debug)]
pub struct DiscoveryRow {
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub binding: HomeServerBinding,
    pub revocation: Option<RevocationStatement>,
}

enum TokenKind {
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
    hpke: HpkeKeypair,
    sign: SigningKey,
    bundle: ServerBundle,
    pub now: u64,
    limits: Limits,
    mailboxes: HashMap<IdentityId, Mailbox>,
    directory: HashMap<IdentityId, DirectoryRow>,
    memberships: HashMap<IdentityId, Vec<GroupId>>,
    tokens: Vec<([u8; KEY_LEN], TokenKind)>,
    prekeys: HashMap<IdentityId, VecDeque<Vec<u8>>>,
    seen_enc: HashMap<[u8; KEY_LEN], u64>,
    pub(crate) peers: HashMap<ServerId, PeerState>,
    pub(crate) outbound: Vec<OutboundRow>,
}

impl HomeServer {
    pub fn new() -> Self {
        let hpke = HpkeKeypair::generate();
        let sign = SigningKey::generate(&mut rand::rngs::OsRng);
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
        }
    }

    pub fn with_limits(limits: Limits) -> Self {
        let mut s = Self::new();
        s.limits = limits;
        s
    }

    pub fn hpke_public(&self) -> [u8; KEY_LEN] {
        self.hpke.public
    }

    pub fn server_id(&self) -> ServerId {
        self.hpke.server_id()
    }

    pub fn bundle(&self) -> &ServerBundle {
        &self.bundle
    }

    pub fn sign_public(&self) -> [u8; KEY_LEN] {
        self.sign.verifying_key().to_bytes()
    }

    pub fn peer_refused(&self, id: ServerId) -> bool {
        self.peers.get(&id).is_some_and(|p| p.refused)
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
        if binding.server_id != self.server_id() {
            self.disable_mailbox(identity_id);
        }
        Ok(())
    }

    pub fn note_group_membership(&mut self, identity_id: IdentityId, group_id: GroupId) {
        let list = self.memberships.entry(identity_id).or_default();
        if !list.contains(&group_id) {
            list.push(group_id);
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
        self.disable_mailbox(stmt.identity_id);
        Ok(self
            .memberships
            .remove(&stmt.identity_id)
            .unwrap_or_default())
    }

    pub fn mailbox_disabled(&self, owner: IdentityId) -> bool {
        self.mailboxes.get(&owner).is_some_and(|m| m.disabled)
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
        Ok(())
    }

    pub fn mint_contact_capability(&mut self, owner: IdentityId) -> Result<[u8; KEY_LEN]> {
        self.require_mailbox(owner)?;
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
        if confirm {
            if let Some(TokenKind::Contact {
                mailbox,
                grace_until,
                ..
            }) = self.token_mut(&old)
            {
                if *mailbox == owner {
                    *grace_until = Some(grace);
                }
            }
        }
        Ok(new)
    }

    pub fn publish_prekey(&mut self, owner: IdentityId, blob: Vec<u8>) -> Result<()> {
        self.require_mailbox(owner)?;
        self.prekeys.entry(owner).or_default().push_back(blob);
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
        }
        self.seen_enc.insert(enc, seq);
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
    }

    fn authorize_append(&mut self, token: &[u8; KEY_LEN]) -> Result<IdentityId> {
        let now = self.now;
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
                *mailbox
            }
            _ => {
                self.dummy_work();
                return Err(ServerError::Denied);
            }
        };
        if self.mailboxes.get(&mailbox).is_some_and(|m| m.disabled) {
            return Err(ServerError::Denied);
        }
        Ok(mailbox)
    }

    fn burn_if_share(&mut self, token: &[u8; KEY_LEN]) {
        if let Some(TokenKind::Share { burned, .. }) = self.token_mut(token) {
            *burned = true;
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
        for seq in expired {
            remove_row(box_, seq);
        }
        while box_.bytes > max_bytes {
            let Some((&oldest, _)) = box_.rows.iter().next() else {
                break;
            };
            remove_row(box_, oldest);
        }
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

pub(crate) fn random_token() -> [u8; KEY_LEN] {
    let mut t = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut t);
    t
}
