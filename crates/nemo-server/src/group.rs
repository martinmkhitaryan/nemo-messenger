//! Group stream host. Does not parse MLS commits. No HTTP.

use std::collections::HashMap;

use nemo_wire::envelope::{MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::KEY_LEN;
use nemo_wire::{
    AttachmentReserve, GroupAdmit, GroupInvite, RemoveBundle, RevocationStatement,
    SigningKeyReplace, VerifyingKey,
};
use subtle::ConstantTimeEq;

use crate::error::{Result, ServerError};
use crate::home::random_token;

pub const STREAM_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const STREAM_MAX_AGE_SECS: u64 = 30 * 24 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(pub [u8; KEY_LEN]);

#[derive(Clone, Debug)]
pub struct FanoutTarget {
    pub delivery_capability: [u8; KEY_LEN],
    pub home_hpke_public: [u8; KEY_LEN],
}

#[derive(Clone, Debug)]
pub struct MemberCred {
    pub credential_id: [u8; KEY_LEN],
    pub credential_secret: [u8; KEY_LEN],
}

#[derive(Clone, Debug)]
pub struct CreatedGroup {
    pub group_id: GroupId,
    pub cred: MemberCred,
}

#[derive(Clone, Debug)]
pub struct PendingJoin {
    pub pending_id: [u8; KEY_LEN],
    pub cred: MemberCred,
    pub invitee_binding: Option<[u8; KEY_LEN]>,
}

#[derive(Clone, Debug)]
pub struct StreamAppend {
    pub seq: u64,
    pub outers: Vec<OuterEnvelope>,
}

struct CredRecord {
    secret: [u8; KEY_LEN],
    live: bool,
    signing_pk: Option<[u8; KEY_LEN]>,
    fanout: Option<FanoutTarget>,
}

struct StoredInvite {
    expires_at: u64,
    invitee_binding: Option<[u8; KEY_LEN]>,
}

struct PendingRecord {
    cred: MemberCred,
    expires_at: u64,
}

struct FileSlot {
    group_id: GroupId,
    owner: [u8; KEY_LEN],
    size: usize,
    expires_at: u64,
    bytes: Option<Vec<u8>>,
}

struct GroupState {
    next_seq: u64,
    creds: HashMap<[u8; KEY_LEN], CredRecord>,
    invites: HashMap<[u8; KEY_LEN], StoredInvite>,
    pending: HashMap<[u8; KEY_LEN], PendingRecord>,
    bytes: u64,
}

pub struct GroupHost {
    pub now: u64,
    groups: HashMap<GroupId, GroupState>,
    files: HashMap<[u8; KEY_LEN], FileSlot>,
}

impl GroupHost {
    pub fn new() -> Self {
        Self {
            now: 1_700_000_000,
            groups: HashMap::new(),
            files: HashMap::new(),
        }
    }

    pub fn create_group(
        &mut self,
        signing_pk: [u8; KEY_LEN],
        fanout: FanoutTarget,
    ) -> Result<CreatedGroup> {
        let group_id = GroupId(random_token());
        let cred = MemberCred {
            credential_id: random_token(),
            credential_secret: random_token(),
        };
        let mut creds = HashMap::new();
        creds.insert(
            cred.credential_id,
            CredRecord {
                secret: cred.credential_secret,
                live: true,
                signing_pk: Some(signing_pk),
                fanout: Some(fanout),
            },
        );
        self.groups.insert(
            group_id,
            GroupState {
                next_seq: 1,
                creds,
                invites: HashMap::new(),
                pending: HashMap::new(),
                bytes: 0,
            },
        );
        Ok(CreatedGroup { group_id, cred })
    }

    /// Live member stores a signed invite. The host does not mint it.
    pub fn store_invite(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        invite: &GroupInvite,
    ) -> Result<()> {
        self.authorize_live(group_id, cred)?;
        if invite.group_id != group_id.0 {
            return Err(ServerError::Denied);
        }
        let pk = self
            .signing_pk(group_id, cred.credential_id)
            .ok_or(ServerError::Denied)?;
        let vk = VerifyingKey::from_bytes(&pk).map_err(|_| ServerError::Denied)?;
        invite.verify(&vk).map_err(|_| ServerError::Denied)?;
        let g = self.groups.get_mut(&group_id).ok_or(ServerError::Denied)?;
        g.invites.insert(
            invite.nonce,
            StoredInvite {
                expires_at: self.now.saturating_add(invite.ttl_secs),
                invitee_binding: invite.invitee_binding,
            },
        );
        Ok(())
    }

    pub fn accept(&mut self, group_id: GroupId, nonce: [u8; KEY_LEN]) -> Result<PendingJoin> {
        let g = self.groups.get_mut(&group_id).ok_or(ServerError::Denied)?;
        let stored = g.invites.remove(&nonce).ok_or(ServerError::Denied)?;
        if self.now >= stored.expires_at {
            return Err(ServerError::Denied);
        }
        let cred = MemberCred {
            credential_id: random_token(),
            credential_secret: random_token(),
        };
        g.creds.insert(
            cred.credential_id,
            CredRecord {
                secret: cred.credential_secret,
                live: false,
                signing_pk: None,
                fanout: None,
            },
        );
        let pending_id = random_token();
        g.pending.insert(
            pending_id,
            PendingRecord {
                cred: cred.clone(),
                expires_at: stored.expires_at,
            },
        );
        Ok(PendingJoin {
            pending_id,
            cred,
            invitee_binding: stored.invitee_binding,
        })
    }

    pub fn admit(
        &mut self,
        group_id: GroupId,
        admitter: &MemberCred,
        admit: &GroupAdmit,
        joiner_signing_pk: [u8; KEY_LEN],
        fanout: FanoutTarget,
    ) -> Result<[u8; KEY_LEN]> {
        self.authorize_live(group_id, admitter)?;
        if admit.group_id != group_id.0 {
            return Err(ServerError::Denied);
        }
        let pk = self
            .signing_pk(group_id, admitter.credential_id)
            .ok_or(ServerError::Denied)?;
        let vk = VerifyingKey::from_bytes(&pk).map_err(|_| ServerError::Denied)?;
        admit.verify(&vk).map_err(|_| ServerError::Denied)?;
        self.gc_pending(group_id);
        let g = self.groups.get_mut(&group_id).ok_or(ServerError::Denied)?;
        let reserved = g
            .pending
            .remove(&admit.pending_id)
            .ok_or(ServerError::Denied)?;
        if self.now >= reserved.expires_at {
            return Err(ServerError::Denied);
        }
        let rec = g
            .creds
            .get_mut(&reserved.cred.credential_id)
            .ok_or(ServerError::Denied)?;
        rec.live = true;
        rec.signing_pk = Some(joiner_signing_pk);
        rec.fanout = Some(fanout);
        Ok(reserved.cred.credential_id)
    }

    /// Host-only stream object. Caller has already verified the statement.
    pub fn append_host_revocation(
        &mut self,
        group_id: GroupId,
        stmt: &RevocationStatement,
    ) -> Result<StreamAppend> {
        if !self.groups.contains_key(&group_id) {
            return Err(ServerError::Denied);
        }
        self.fanout_copy(group_id, MessageType::RevocationStatement, stmt.encode())
    }

    pub fn refresh_fanout(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        fanout: FanoutTarget,
    ) -> Result<()> {
        self.authorize_live(group_id, cred)?;
        let g = self.groups.get_mut(&group_id).ok_or(ServerError::Denied)?;
        if let Some(rec) = g.creds.get_mut(&cred.credential_id) {
            rec.fanout = Some(fanout);
        }
        Ok(())
    }

    pub fn append(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        type_: MessageType,
        body: Vec<u8>,
    ) -> Result<StreamAppend> {
        self.authorize_live(group_id, cred)?;
        match type_ {
            MessageType::RemoveBundle => self.append_remove(group_id, cred, body),
            MessageType::SigningKeyReplace => {
                self.apply_signing_replace(group_id, cred, &body)?;
                self.fanout_copy(group_id, type_, body)
            }
            MessageType::MlsHandshake | MessageType::MlsApp | MessageType::RevocationStatement => {
                self.fanout_copy(group_id, type_, body)
            }
            MessageType::AttachmentReserve => self.append_reserve(group_id, cred, body),
            _ => Err(ServerError::Denied),
        }
    }

    /// After reserve: body length MUST equal the reserved A* bucket. First upload wins.
    pub fn upload_file(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        fetch_token: [u8; KEY_LEN],
        body: Vec<u8>,
    ) -> Result<()> {
        self.authorize_live(group_id, cred)?;
        let slot = self
            .files
            .get_mut(&fetch_token)
            .ok_or(ServerError::Denied)?;
        if slot.group_id != group_id
            || slot.owner != cred.credential_id
            || self.now >= slot.expires_at
        {
            return Err(ServerError::Denied);
        }
        if slot.bytes.is_some() || body.len() != slot.size {
            return Err(ServerError::Denied);
        }
        slot.bytes = Some(body);
        Ok(())
    }

    /// Fetch needs a live member credential **and** the token.
    pub fn fetch_file(
        &self,
        group_id: GroupId,
        cred: &MemberCred,
        fetch_token: [u8; KEY_LEN],
    ) -> Result<Vec<u8>> {
        self.authorize_live(group_id, cred)?;
        match self.files.get(&fetch_token) {
            Some(slot)
                if slot.group_id == group_id
                    && self.now < slot.expires_at
                    && slot.bytes.is_some() =>
            {
                Ok(slot.bytes.clone().expect("checked"))
            }
            _ => {
                dummy_scan();
                Err(ServerError::Denied)
            }
        }
    }

    fn append_remove(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        body: Vec<u8>,
    ) -> Result<StreamAppend> {
        let bundle = RemoveBundle::decode(&body)?;
        let appender_pk = self
            .signing_pk(group_id, cred.credential_id)
            .ok_or(ServerError::Denied)?;
        let pk = VerifyingKey::from_bytes(&appender_pk).map_err(|_| ServerError::Denied)?;
        bundle.verify_sidecar(&pk)?;
        let named = bundle.credential_id;
        // Fan-out to the pre-revoke set, including the named credential.
        let out = self.fanout_copy(group_id, MessageType::RemoveBundle, body)?;
        if let Some(g) = self.groups.get_mut(&group_id) {
            if let Some(rec) = g.creds.get_mut(&named) {
                rec.live = false;
                rec.fanout = None;
            }
        }
        Ok(out)
    }

    fn append_reserve(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        body: Vec<u8>,
    ) -> Result<StreamAppend> {
        let reserve = AttachmentReserve::decode(&body)?;
        if self.files.contains_key(&reserve.fetch_token) {
            return Err(ServerError::Denied);
        }
        let expires_at = match reserve.ttl_bucket.ttl_secs() {
            Some(s) => self.now.saturating_add(s),
            None => self.now.saturating_add(STREAM_MAX_AGE_SECS),
        };
        let out = self.fanout_copy(group_id, MessageType::AttachmentReserve, body)?;
        self.files.insert(
            reserve.fetch_token,
            FileSlot {
                group_id,
                owner: cred.credential_id,
                size: reserve.size_bucket.inner_len(),
                expires_at,
                bytes: None,
            },
        );
        Ok(out)
    }

    fn gc_pending(&mut self, group_id: GroupId) {
        let now = self.now;
        if let Some(g) = self.groups.get_mut(&group_id) {
            g.pending.retain(|_, p| p.expires_at > now);
            g.invites.retain(|_, i| i.expires_at > now);
        }
    }

    fn apply_signing_replace(
        &mut self,
        group_id: GroupId,
        cred: &MemberCred,
        body: &[u8],
    ) -> Result<()> {
        let replace = SigningKeyReplace::decode(body)?;
        let current = self
            .signing_pk(group_id, cred.credential_id)
            .ok_or(ServerError::Denied)?;
        let pk = VerifyingKey::from_bytes(&current).map_err(|_| ServerError::Denied)?;
        replace.verify(&pk)?;
        if let Some(g) = self.groups.get_mut(&group_id) {
            if let Some(rec) = g.creds.get_mut(&cred.credential_id) {
                rec.signing_pk = Some(replace.new_public_key);
            }
        }
        Ok(())
    }

    fn fanout_copy(
        &mut self,
        group_id: GroupId,
        type_: MessageType,
        body: Vec<u8>,
    ) -> Result<StreamAppend> {
        let g = self.groups.get_mut(&group_id).ok_or(ServerError::Denied)?;
        let add = body.len() as u64;
        if g.bytes.saturating_add(add) > STREAM_MAX_BYTES {
            return Err(ServerError::Denied);
        }
        let seq = g.next_seq;
        g.next_seq += 1;
        g.bytes = g.bytes.saturating_add(add);
        let targets: Vec<FanoutTarget> = g
            .creds
            .values()
            .filter(|rec| rec.live && rec.fanout.is_some())
            .filter_map(|rec| rec.fanout.clone())
            .collect();
        let mut outers = Vec::new();
        for t in targets {
            let inner = nemo_wire::InnerEnvelope {
                delivery_capability: t.delivery_capability,
                ttl_bucket: TtlBucket::DEFAULT,
                idempotency_token: random_token(),
                padded_message: nemo_wire::PaddedMessage::pad(type_, body.clone())?,
            };
            outers.push(seal_to_server(&t.home_hpke_public, &inner)?);
        }
        Ok(StreamAppend { seq, outers })
    }

    fn authorize_live(&self, group_id: GroupId, cred: &MemberCred) -> Result<()> {
        let Some(g) = self.groups.get(&group_id) else {
            dummy_scan();
            return Err(ServerError::Denied);
        };
        match g.creds.get(&cred.credential_id) {
            Some(rec) if rec.live && bool::from(rec.secret.ct_eq(&cred.credential_secret)) => {
                Ok(())
            }
            _ => {
                dummy_scan();
                Err(ServerError::Denied)
            }
        }
    }

    fn signing_pk(&self, group_id: GroupId, credential_id: [u8; KEY_LEN]) -> Option<[u8; KEY_LEN]> {
        self.groups
            .get(&group_id)?
            .creds
            .get(&credential_id)?
            .signing_pk
    }
}

impl Default for GroupHost {
    fn default() -> Self {
        Self::new()
    }
}

fn dummy_scan() {
    let _ = 0u8;
}
