//! MLS application profile (RFC 9420 suite `0x0003`).
//!
//! Join is invite → accept → admit. Clients reject External Commits, external
//! joins, ReInit, and an unframed Remove. A Remove is applied only as a
//! [`nemo_wire::RemoveBundle`].
//!
//! The per-group signing key (leaf extension `0xF002`) is the MLS leaf
//! signature key, so invite/admit/sidecar and MLS Commits use one key.
//!
//! Sequencing (ADR-0017): an own Commit is **staged** here and merged only
//! when the host echoes it back on the group stream. If another member's
//! Commit for the same epoch arrives first, ours is discarded and the caller
//! re-issues it ([`HandshakeOutcome::OwnCommitSuperseded`]).
//!
//! Host metadata (`0xF001` credential ids) is always re-derived from the
//! ratchet tree, so a member who joined via Welcome knows every leaf's id.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::hostframe::{RemoveBundle, SigningKeyReplace};
use nemo_wire::ids::{copy_fixed, IdentityId, KEY_LEN};
use nemo_wire::{identity_id, GroupAdmit, GroupInvite, SigningKey, INTRO_TTL_30_MIN};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use rand::Rng;
use tls_codec::{Deserialize, Serialize};

use crate::error::{mls_err, CoreError, Result};
use crate::identity::Installation;
use crate::mailbox;

/// RFC 9420 `MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519`.
pub const CIPHERSUITE: Ciphersuite =
    Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519;
pub const CREDENTIAL_ID_EXT: u16 = 0xF001;
pub const GROUP_SIGNING_EXT: u16 = 0xF002;
pub const UPDATE_BEFORE_SEND: Duration = Duration::from_secs(72 * 3600);
pub const UPDATE_INTERVAL: Duration = Duration::from_secs(7 * 24 * 3600);
pub const UPDATE_ON_ONLINE: Duration = Duration::from_secs(24 * 3600);

pub type MlsProvider = OpenMlsRustCrypto;

/// KeyPackage produced at accept, with the reserved `credential_id`.
pub struct PendingJoin {
    pub credential_id: [u8; KEY_LEN],
    mls_signer: SignatureKeyPair,
    group_signing: SigningKey,
    pub key_package: Vec<u8>,
}

/// What kind of own Commit is waiting for the host echo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingKind {
    Update,
    Add,
    Remove,
    SigningKeyRotate,
}

struct PendingOwn {
    kind: PendingKind,
    /// Set for `SigningKeyRotate`: becomes the leaf signer once merged.
    new_signer: Option<SigningKey>,
}

/// Result of applying a handshake object from the group stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeOutcome {
    /// Another member's Commit was validated and merged.
    Applied,
    /// The host echoed our staged Commit; it is merged now.
    OwnCommitMerged,
    /// Another member's Commit won this epoch. Our staged Commit was
    /// discarded; the caller re-issues the operation if still wanted.
    OwnCommitSuperseded,
}

pub struct Group {
    mls: MlsGroup,
    mls_signer: SignatureKeyPair,
    group_signing: SigningKey,
    credential_id: [u8; KEY_LEN],
    /// Leaf index → host `credential_id` (`0xF001`), rebuilt from the tree.
    credential_ids: HashMap<LeafNodeIndex, [u8; KEY_LEN]>,
    last_own_update: SystemTime,
    pending: Option<PendingOwn>,
    /// `SigningKeyReplace` seen on the stream: old leaf signature key → new.
    announced_keys: HashMap<Vec<u8>, [u8; KEY_LEN]>,
}

impl Group {
    pub fn credential_id(&self) -> [u8; KEY_LEN] {
        self.credential_id
    }

    pub fn group_signing_public(&self) -> [u8; KEY_LEN] {
        self.group_signing.verifying_key().to_bytes()
    }

    pub fn epoch(&self) -> u64 {
        self.mls.epoch().as_u64()
    }

    pub fn member_count(&self) -> usize {
        self.mls.members().count()
    }

    pub fn has_pending_commit(&self) -> bool {
        self.pending.is_some()
    }

    pub fn pending_kind(&self) -> Option<PendingKind> {
        self.pending.as_ref().map(|p| p.kind)
    }

    /// Drop a staged own Commit (e.g. the host refused the append).
    pub fn discard_pending(&mut self, provider: &MlsProvider) -> Result<()> {
        self.mls
            .clear_pending_commit(provider.storage())
            .map_err(mls_err)?;
        self.pending = None;
        Ok(())
    }

    pub fn member_credential_ids(&self) -> Vec<[u8; KEY_LEN]> {
        let mut ids: Vec<_> = self.credential_ids.values().copied().collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// MLS BasicCredential identities hashed to `identity_id` (ADR-0004 / 0018).
    pub fn member_identity_ids(&self) -> Vec<IdentityId> {
        let mut ids: Vec<_> = self
            .mls
            .members()
            .filter_map(|m| identity_pk_from_credential(&m.credential).ok())
            .map(|pk| identity_id(&pk))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Invitee identity public key from a KeyPackage (not the group signing key).
    pub fn key_package_identity_pk(
        provider: &MlsProvider,
        key_package_bytes: &[u8],
    ) -> Result<[u8; KEY_LEN]> {
        let kp = validate_key_package(provider, key_package_bytes)?;
        identity_pk_from_credential(kp.leaf_node().credential())
    }

    /// `0xF001` carried by a KeyPackage. The admitter sends this to the host
    /// as the expected reserved id (ADR-0018).
    pub fn key_package_credential_id(
        provider: &MlsProvider,
        key_package_bytes: &[u8],
    ) -> Result<[u8; KEY_LEN]> {
        let kp = validate_key_package(provider, key_package_bytes)?;
        check_new_leaf(kp.leaf_node())
    }

    /// Leaf signature key (= group signing key, `0xF002`) of a KeyPackage.
    pub fn key_package_signing_pk(
        provider: &MlsProvider,
        key_package_bytes: &[u8],
    ) -> Result<[u8; KEY_LEN]> {
        let kp = validate_key_package(provider, key_package_bytes)?;
        copy_fixed(kp.leaf_node().signature_key().as_slice())
            .map_err(|_| CoreError::LeafExtensionViolation)
    }

    /// Commit an Update when the last own Update is at least 7 days old, or 24 h
    /// while the client is online (ADR-0005). Never while a Commit is pending.
    pub fn maybe_self_update(
        &mut self,
        provider: &MlsProvider,
        now: SystemTime,
    ) -> Result<Option<Vec<u8>>> {
        if self.pending.is_some() {
            return Ok(None);
        }
        if self.needs_scheduled_update(now) || self.needs_online_update(now) {
            Ok(Some(self.self_update(provider)?))
        } else {
            Ok(None)
        }
    }

    /// Host `credential_id` for an MLS member's identity, if they are still a leaf.
    pub fn credential_id_for_identity(&self, identity: &IdentityId) -> Option<[u8; KEY_LEN]> {
        self.mls.members().find_map(|m| {
            let pk = identity_pk_from_credential(&m.credential).ok()?;
            if identity_id(&pk) == *identity {
                self.credential_ids.get(&m.index).copied()
            } else {
                None
            }
        })
    }

    pub fn mls_group_id(&self) -> Vec<u8> {
        self.mls.group_id().as_slice().to_vec()
    }

    pub fn encode_sidecar(&self) -> Result<Vec<u8>> {
        let updated = self
            .last_own_update
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut announced: Vec<_> = self.announced_keys.iter().collect();
        announced.sort_by(|a, b| a.0.cmp(b.0));
        let announced = announced
            .into_iter()
            .map(|(old, new)| {
                Value::Array(vec![Value::Bytes(old.clone()), Value::Bytes(new.to_vec())])
            })
            .collect();
        let mut pairs = vec![
            (0, Value::Uint(1)),
            (1, Value::Bytes(self.mls_group_id())),
            (2, Value::Bytes(self.group_signing.to_bytes().to_vec())),
            (3, Value::Bytes(self.credential_id.to_vec())),
            // Field 4 used to persist the credential map; it is derived from
            // the ratchet tree now. Kept empty for older readers.
            (4, Value::Array(Vec::new())),
            (5, Value::Uint(updated)),
            (6, Value::Uint(pending_kind_code(self.pending.as_ref().map(|p| p.kind)))),
            (8, Value::Array(announced)),
        ];
        if let Some(seed) = self.pending.as_ref().and_then(|p| p.new_signer.as_ref()) {
            pairs.push((7, Value::Bytes(seed.to_bytes().to_vec())));
        }
        pairs.sort_by_key(|(k, _)| *k);
        Ok(cbor::encode(&Value::Map(pairs)))
    }

    pub fn load_sidecar(provider: &MlsProvider, bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if version != 1 {
            return Err(CoreError::VaultCorrupt);
        }
        let gid = cbor::expect_bytes(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        let sk = cbor::expect_bytes(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        let seed = copy_fixed(sk).map_err(|_| CoreError::VaultCorrupt)?;
        let group_signing = SigningKey::from_bytes(&seed);
        let credential_id = copy_fixed(
            cbor::expect_bytes(cbor::map_get(&m, 3).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?,
        )
        .map_err(|_| CoreError::VaultCorrupt)?;
        let updated = cbor::expect_uint(cbor::map_get(&m, 5).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        let last_own_update = std::time::UNIX_EPOCH + Duration::from_millis(updated);
        let pending_code = match cbor::map_get_opt(&m, 6) {
            Some(v) => cbor::expect_uint(v).map_err(|_| CoreError::VaultCorrupt)?,
            None => 0,
        };
        let new_signer = match cbor::map_get_opt(&m, 7) {
            Some(v) => {
                let s = copy_fixed(cbor::expect_bytes(v).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?;
                Some(SigningKey::from_bytes(&s))
            }
            None => None,
        };
        let mut announced_keys = HashMap::new();
        if let Some(v) = cbor::map_get_opt(&m, 8) {
            for item in cbor::expect_array(v).map_err(|_| CoreError::VaultCorrupt)? {
                let Value::Array(row) = item else {
                    return Err(CoreError::VaultCorrupt);
                };
                if row.len() != 2 {
                    return Err(CoreError::VaultCorrupt);
                }
                let old = cbor::expect_bytes(&row[0])
                    .map_err(|_| CoreError::VaultCorrupt)?
                    .to_vec();
                let new = copy_fixed(cbor::expect_bytes(&row[1]).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?;
                announced_keys.insert(old, new);
            }
        }
        let mls_signer = SignatureKeyPair::from_raw(
            CIPHERSUITE.signature_algorithm(),
            group_signing.to_bytes().to_vec(),
            group_signing.verifying_key().to_bytes().to_vec(),
        );
        let group_id = GroupId::from_slice(gid);
        let mls = MlsGroup::load(provider.storage(), &group_id)
            .map_err(mls_err)?
            .ok_or(CoreError::VaultCorrupt)?;
        if mls.ciphersuite() != CIPHERSUITE {
            return Err(CoreError::WrongCiphersuite);
        }
        let pending = match (pending_kind_from_code(pending_code), mls.pending_commit().is_some()) {
            (Some(kind), true) => Some(PendingOwn { kind, new_signer }),
            _ => None,
        };
        let mut g = Self {
            mls,
            mls_signer,
            group_signing,
            credential_id,
            credential_ids: HashMap::new(),
            last_own_update,
            pending,
            announced_keys,
        };
        if g.pending.is_none() && g.mls.pending_commit().is_some() {
            // Metadata lost: the staged commit cannot be finished correctly.
            g.mls
                .clear_pending_commit(provider.storage())
                .map_err(mls_err)?;
        }
        g.sync_credential_ids();
        Ok(g)
    }

    pub fn needs_scheduled_update(&self, now: SystemTime) -> bool {
        now.duration_since(self.last_own_update)
            .map(|d| d >= UPDATE_INTERVAL)
            .unwrap_or(true)
    }

    pub fn needs_online_update(&self, now: SystemTime) -> bool {
        now.duration_since(self.last_own_update)
            .map(|d| d >= UPDATE_ON_ONLINE)
            .unwrap_or(true)
    }

    pub fn sign_invite(
        &self,
        host_group_id: [u8; KEY_LEN],
        ttl_secs: u64,
        bind_identity_pk: Option<&[u8; KEY_LEN]>,
    ) -> Result<GroupInvite> {
        let mut nonce = [0u8; KEY_LEN];
        rand::rng().fill_bytes(&mut nonce);
        let bind = bind_identity_pk.map(|pk| {
            let mut salt = [0u8; KEY_LEN];
            rand::rng().fill_bytes(&mut salt);
            (nemo_wire::invitee_binding(pk, &salt), salt)
        });
        Ok(GroupInvite::sign(
            &self.group_signing,
            host_group_id,
            nonce,
            ttl_secs,
            bind,
        )?)
    }

    pub fn sign_admit(
        &self,
        host_group_id: [u8; KEY_LEN],
        pending_id: [u8; KEY_LEN],
    ) -> Result<GroupAdmit> {
        Ok(GroupAdmit::sign(
            &self.group_signing,
            host_group_id,
            pending_id,
        )?)
    }

    pub fn default_invite_ttl() -> u64 {
        INTRO_TTL_30_MIN
    }

    pub fn encrypt(&mut self, provider: &MlsProvider, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.ensure_fresh_update(SystemTime::now())?;
        let out = self
            .mls
            .create_message(provider, &self.mls_signer, plaintext)
            .map_err(mls_err)?;
        serialize_msg(&out)
    }

    pub fn decrypt(&mut self, provider: &MlsProvider, ciphertext: &[u8]) -> Result<Vec<u8>> {
        Ok(self.decrypt_with_sender(provider, ciphertext)?.0)
    }

    /// Decrypt an application message and return the sender's `identity_id`
    /// (from the member leaf credential). Non-member senders are rejected.
    pub fn decrypt_with_sender(
        &mut self,
        provider: &MlsProvider,
        ciphertext: &[u8],
    ) -> Result<(Vec<u8>, IdentityId)> {
        let processed = self.process(provider, ciphertext)?;
        let sender = match processed.sender() {
            Sender::Member(index) => {
                let member = self
                    .mls
                    .members()
                    .find(|m| m.index == *index)
                    .ok_or(CoreError::Mls("unknown message sender".into()))?;
                identity_id(&identity_pk_from_credential(&member.credential)?)
            }
            _ => return Err(CoreError::ExternalMlsJoin),
        };
        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(m) => Ok((m.into_bytes(), sender)),
            _ => Err(CoreError::Mls("expected application message".into())),
        }
    }

    pub fn encrypt_to_mailbox(
        &mut self,
        provider: &MlsProvider,
        dest_hpke_public: &[u8; KEY_LEN],
        delivery_capability: [u8; KEY_LEN],
        ttl_bucket: TtlBucket,
        plaintext: &[u8],
    ) -> Result<OuterEnvelope> {
        let body = self.encrypt(provider, plaintext)?;
        mailbox::wrap(
            dest_hpke_public,
            delivery_capability,
            ttl_bucket,
            MessageType::MlsApp,
            body,
        )
    }

    pub fn decrypt_from_mailbox(
        &mut self,
        provider: &MlsProvider,
        inner: &InnerEnvelope,
    ) -> Result<Vec<u8>> {
        Ok(self.decrypt_from_mailbox_with_sender(provider, inner)?.0)
    }

    pub fn decrypt_from_mailbox_with_sender(
        &mut self,
        provider: &MlsProvider,
        inner: &InnerEnvelope,
    ) -> Result<(Vec<u8>, IdentityId)> {
        let body = mailbox::expect_type(inner, MessageType::MlsApp)?;
        self.decrypt_with_sender(provider, body)
    }

    pub fn wrap_handshake(
        dest_hpke_public: &[u8; KEY_LEN],
        delivery_capability: [u8; KEY_LEN],
        ttl_bucket: TtlBucket,
        handshake: Vec<u8>,
    ) -> Result<OuterEnvelope> {
        mailbox::wrap(
            dest_hpke_public,
            delivery_capability,
            ttl_bucket,
            MessageType::MlsHandshake,
            handshake,
        )
    }

    pub fn apply_handshake_from_mailbox(
        &mut self,
        provider: &MlsProvider,
        inner: &InnerEnvelope,
    ) -> Result<HandshakeOutcome> {
        let body = mailbox::expect_type(inner, MessageType::MlsHandshake)?;
        self.apply_handshake(provider, body)
    }

    /// Epoch carried by a handshake ciphertext. Compared with [`Group::epoch`]
    /// when OpenMLS reports `WrongEpoch`: a lower epoch is an echo of a
    /// commit this member already merged.
    pub fn handshake_message_epoch(inner: &InnerEnvelope) -> Result<u64> {
        let body = mailbox::expect_type(inner, MessageType::MlsHandshake)?;
        let msg = decode_mls(body)?;
        let protocol = msg.try_into_protocol_message().map_err(mls_err)?;
        Ok(protocol.epoch().as_u64())
    }

    pub fn wrap_remove(
        dest_hpke_public: &[u8; KEY_LEN],
        delivery_capability: [u8; KEY_LEN],
        ttl_bucket: TtlBucket,
        bundle: &RemoveBundle,
    ) -> Result<OuterEnvelope> {
        mailbox::wrap(
            dest_hpke_public,
            delivery_capability,
            ttl_bucket,
            MessageType::RemoveBundle,
            bundle.encode()?,
        )
    }

    pub fn apply_remove_from_mailbox(
        &mut self,
        provider: &MlsProvider,
        host_group_id: &[u8; KEY_LEN],
        inner: &InnerEnvelope,
    ) -> Result<HandshakeOutcome> {
        let body = mailbox::expect_type(inner, MessageType::RemoveBundle)?;
        let bundle = RemoveBundle::decode(body)?;
        self.apply_remove_bundle(provider, host_group_id, &bundle)
    }

    /// Record a host-framed `SigningKeyReplace` from the stream. The member is
    /// whoever's **current** leaf key verifies it (ADR-0018). A later leaf
    /// update may then move that leaf to the new key, and only to that key.
    pub fn apply_signing_key_replace(&mut self, body: &[u8]) -> Result<()> {
        let replace = SigningKeyReplace::decode(body)?;
        for m in self.mls.members() {
            let Ok(arr) = <[u8; KEY_LEN]>::try_from(m.signature_key.as_slice()) else {
                continue;
            };
            let Ok(pk) = nemo_wire::VerifyingKey::from_bytes(&arr) else {
                continue;
            };
            if replace.verify(&pk).is_ok() {
                self.announced_keys
                    .insert(m.signature_key.clone(), replace.new_public_key);
                return Ok(());
            }
        }
        Err(CoreError::UnannouncedSignerChange)
    }

    /// Apply a non-Remove handshake (Add, Update, or the echo of an own Commit).
    pub fn apply_handshake(
        &mut self,
        provider: &MlsProvider,
        message: &[u8],
    ) -> Result<HandshakeOutcome> {
        let processed = self.process(provider, message)?;
        let sender = processed.sender().clone();
        match processed.into_content() {
            ProcessedMessageContent::OwnPendingCommit => self.finish_own_commit(provider),
            // Some OpenMLS 0.9 paths report an own Commit as a private
            // message rather than OwnPendingCommit. If we staged one, this
            // is the host echo.
            ProcessedMessageContent::OwnPrivateMessage if self.pending.is_some() => {
                self.finish_own_commit(provider)
            }
            ProcessedMessageContent::StagedCommitMessage(commit) => {
                if is_external_commit(&commit) {
                    return Err(CoreError::ExternalMlsJoin);
                }
                if staged_remove_count(&commit) > 0 {
                    return Err(CoreError::UnframedRemove);
                }
                self.validate_commit(&commit, &sender)?;
                self.merge_foreign(provider, *commit)
            }
            ProcessedMessageContent::ExternalJoinProposalMessage(_) => {
                Err(CoreError::ExternalMlsJoin)
            }
            ProcessedMessageContent::ProposalMessage(p) => {
                if is_reinit(&p) {
                    return Err(CoreError::MlsReinit);
                }
                Err(CoreError::Mls(
                    "standalone proposal is not used in v1".into(),
                ))
            }
            ProcessedMessageContent::ApplicationMessage(_) => Err(CoreError::Mls(
                "handshake path got an application message".into(),
            )),
            ProcessedMessageContent::OwnPrivateMessage => Err(CoreError::Mls(
                "own private message in handshake path".into(),
            )),
        }
    }

    /// Stage an Update. Returns the Commit to append; merged on echo.
    pub fn self_update(&mut self, provider: &MlsProvider) -> Result<Vec<u8>> {
        self.ensure_no_pending()?;
        let params = LeafNodeParameters::builder()
            .with_capabilities(nemo_capabilities())
            .with_extensions(self.own_leaf_extensions()?)
            .build();
        let bundle = self
            .mls
            .self_update(provider, &self.mls_signer, params)
            .map_err(mls_err)?;
        self.pending = Some(PendingOwn {
            kind: PendingKind::Update,
            new_signer: None,
        });
        // Issued an Update: sending at the current epoch is allowed while
        // the host echo is in flight.
        self.last_own_update = SystemTime::now();
        serialize_msg(&bundle.into_commit())
    }

    /// Rotate the per-group signing key (02 §7.4). Append the returned
    /// `SigningKeyReplace` **before** the Commit so members accept the new
    /// leaf key. Both are host-framed stream objects.
    pub fn rotate_signing_key(
        &mut self,
        provider: &MlsProvider,
    ) -> Result<(SigningKeyReplace, Vec<u8>)> {
        self.ensure_no_pending()?;
        let (new_mls, new_signing) = new_group_keys(provider)?;
        let new_pk = new_signing.verifying_key().to_bytes();
        let replace = SigningKeyReplace::sign(&self.group_signing, new_pk)?;
        let params = LeafNodeParameters::builder()
            .with_capabilities(nemo_capabilities())
            .with_extensions(leaf_extensions(self.credential_id, new_pk)?)
            .build();
        let cwk = credential_with_key(self.own_identity_pk()?, &new_mls);
        let bundle = self
            .mls
            .self_update_with_new_signer(
                provider,
                &self.mls_signer,
                NewSignerBundle {
                    signer: &new_mls,
                    credential_with_key: cwk,
                },
                params,
            )
            .map_err(mls_err)?;
        self.pending = Some(PendingOwn {
            kind: PendingKind::SigningKeyRotate,
            new_signer: Some(new_signing),
        });
        Ok((replace, serialize_msg(&bundle.into_commit())?))
    }

    /// Stage an Add. `expected_credential_id` is the host-reserved id for this
    /// pending join; the KeyPackage leaf MUST carry exactly it (ADR-0018).
    pub fn admit(
        &mut self,
        provider: &MlsProvider,
        key_package_bytes: &[u8],
        expected_credential_id: [u8; KEY_LEN],
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        self.ensure_no_pending()?;
        let kp = validate_key_package(provider, key_package_bytes)?;
        if kp.ciphersuite() != CIPHERSUITE {
            return Err(CoreError::WrongCiphersuite);
        }
        let cid = check_new_leaf(kp.leaf_node())?;
        if cid != expected_credential_id {
            return Err(CoreError::CredentialIdMismatch);
        }
        if self.credential_ids.values().any(|c| *c == cid) {
            return Err(CoreError::CredentialIdMismatch);
        }
        let (commit, welcome, _info) = self
            .mls
            .add_members(provider, &self.mls_signer, core::slice::from_ref(&kp))
            .map_err(mls_err)?;
        self.pending = Some(PendingOwn {
            kind: PendingKind::Add,
            new_signer: None,
        });
        Ok((serialize_msg(&commit)?, serialize_msg(&welcome)?))
    }

    /// Stage a Remove as a `RemoveBundle` bound to `host_group_id`.
    pub fn remove(
        &mut self,
        provider: &MlsProvider,
        host_group_id: [u8; KEY_LEN],
        credential_id: [u8; KEY_LEN],
    ) -> Result<RemoveBundle> {
        self.ensure_no_pending()?;
        let index = self
            .leaf_index_for(credential_id)
            .ok_or(CoreError::BadRemoveBundle)?;
        let (commit, _welcome, _info) = self
            .mls
            .remove_members(provider, &self.mls_signer, &[index])
            .map_err(mls_err)?;
        self.pending = Some(PendingOwn {
            kind: PendingKind::Remove,
            new_signer: None,
        });
        Ok(RemoveBundle::sign(
            &self.group_signing,
            host_group_id,
            credential_id,
            serialize_msg(&commit)?,
        )?)
    }

    pub fn apply_remove_bundle(
        &mut self,
        provider: &MlsProvider,
        host_group_id: &[u8; KEY_LEN],
        bundle: &RemoveBundle,
    ) -> Result<HandshakeOutcome> {
        let processed = self.process(provider, &bundle.mls_commit)?;
        let sender = processed.sender().clone();
        if matches!(
            processed.content(),
            ProcessedMessageContent::OwnPendingCommit
                | ProcessedMessageContent::OwnPrivateMessage
        ) && self.pending.is_some()
        {
            return self.finish_own_commit(provider);
        }
        let sender_pk = match &sender {
            Sender::Member(index) => self
                .group_signing_at(*index)
                .ok_or(CoreError::BadRemoveBundle)?,
            _ => return Err(CoreError::ExternalMlsJoin),
        };
        bundle.verify_sidecar(&sender_pk, host_group_id)?;
        match processed.into_content() {
            ProcessedMessageContent::StagedCommitMessage(commit) => {
                if is_external_commit(&commit) {
                    return Err(CoreError::ExternalMlsJoin);
                }
                if staged_remove_count(&commit) != 1 {
                    return Err(CoreError::BadRemoveBundle);
                }
                if !self.staged_remove_matches(&commit, &bundle.credential_id) {
                    return Err(CoreError::BadRemoveBundle);
                }
                self.validate_commit(&commit, &sender)?;
                self.merge_foreign(provider, *commit)
            }
            _ => Err(CoreError::Mls("RemoveBundle body is not a Commit".into())),
        }
    }

    fn process(&mut self, provider: &MlsProvider, bytes: &[u8]) -> Result<ProcessedMessage> {
        let msg = decode_mls(bytes)?;
        let protocol = msg.try_into_protocol_message().map_err(mls_err)?;
        self.mls.process_message(provider, protocol).map_err(|e| {
            if matches!(
                e,
                ProcessMessageError::ValidationError(ValidationError::WrongEpoch)
            ) {
                CoreError::MlsWrongEpoch
            } else {
                mls_err(e)
            }
        })
    }

    fn ensure_no_pending(&self) -> Result<()> {
        if self.pending.is_some() {
            return Err(CoreError::PendingCommit);
        }
        Ok(())
    }

    fn finish_own_commit(&mut self, provider: &MlsProvider) -> Result<HandshakeOutcome> {
        self.mls.merge_pending_commit(provider).map_err(mls_err)?;
        if let Some(p) = self.pending.take() {
            if let Some(new_signing) = p.new_signer {
                self.mls_signer = SignatureKeyPair::from_raw(
                    CIPHERSUITE.signature_algorithm(),
                    new_signing.to_bytes().to_vec(),
                    new_signing.verifying_key().to_bytes().to_vec(),
                );
                self.group_signing = new_signing;
            }
        }
        self.last_own_update = SystemTime::now();
        self.sync_credential_ids();
        Ok(HandshakeOutcome::OwnCommitMerged)
    }

    fn merge_foreign(
        &mut self,
        provider: &MlsProvider,
        commit: StagedCommit,
    ) -> Result<HandshakeOutcome> {
        let had_pending = self.pending.is_some();
        self.mls
            .merge_staged_commit(provider, commit)
            .map_err(mls_err)?;
        // merge_staged_commit clears any pending own commit.
        self.pending = None;
        self.sync_credential_ids();
        Ok(if had_pending {
            HandshakeOutcome::OwnCommitSuperseded
        } else {
            HandshakeOutcome::Applied
        })
    }

    /// 02 §7.1 / ADR-0018: `0xF001` is immutable, `0xF002` equals the leaf
    /// signature key, and a leaf signature key only changes after a
    /// `SigningKeyReplace` naming exactly that new key.
    fn validate_commit(&self, commit: &StagedCommit, sender: &Sender) -> Result<()> {
        for up in commit.update_proposals() {
            let Sender::Member(idx) = up.sender() else {
                return Err(CoreError::ExternalMlsJoin);
            };
            self.check_leaf_update(*idx, up.update_proposal().leaf_node())?;
        }
        if let Some(leaf) = commit.update_path_leaf_node() {
            let Sender::Member(idx) = sender else {
                return Err(CoreError::ExternalMlsJoin);
            };
            self.check_leaf_update(*idx, leaf)?;
        }
        for add in commit.add_proposals() {
            let cid = check_new_leaf(add.add_proposal().key_package().leaf_node())?;
            if self.credential_ids.values().any(|c| *c == cid) {
                return Err(CoreError::LeafExtensionViolation);
            }
        }
        Ok(())
    }

    fn check_leaf_update(&self, index: LeafNodeIndex, leaf: &LeafNode) -> Result<()> {
        let cid = check_new_leaf(leaf)?;
        if let Some(prev) = self.credential_ids.get(&index) {
            if *prev != cid {
                return Err(CoreError::LeafExtensionViolation);
            }
        }
        let Some(member) = self.mls.members().find(|m| m.index == index) else {
            return Err(CoreError::LeafExtensionViolation);
        };
        if member.signature_key.as_slice() != leaf.signature_key().as_slice() {
            let new: [u8; KEY_LEN] = copy_fixed(leaf.signature_key().as_slice())
                .map_err(|_| CoreError::LeafExtensionViolation)?;
            if self.announced_keys.get(&member.signature_key) != Some(&new) {
                return Err(CoreError::UnannouncedSignerChange);
            }
        }
        Ok(())
    }

    fn sync_credential_ids(&mut self) {
        let tree = self.mls.export_ratchet_tree();
        let by_sig: HashMap<Vec<u8>, [u8; KEY_LEN]> = tree
            .leaves()
            .filter_map(|leaf| {
                let cid = extension_bytes(leaf.extensions(), CREDENTIAL_ID_EXT)?;
                let cid = copy_fixed(&cid).ok()?;
                Some((leaf.signature_key().as_slice().to_vec(), cid))
            })
            .collect();
        self.credential_ids = self
            .mls
            .members()
            .filter_map(|m| by_sig.get(&m.signature_key).map(|c| (m.index, *c)))
            .collect();
        // Announcements for keys that are no longer in the tree are stale.
        let live: Vec<Vec<u8>> = self.mls.members().map(|m| m.signature_key).collect();
        self.announced_keys.retain(|old, _| live.contains(old));
    }

    fn ensure_fresh_update(&self, now: SystemTime) -> Result<()> {
        let age = now
            .duration_since(self.last_own_update)
            .unwrap_or(Duration::ZERO);
        if age > UPDATE_BEFORE_SEND {
            return Err(CoreError::StaleMlsUpdate);
        }
        Ok(())
    }

    fn own_identity_pk(&self) -> Result<[u8; KEY_LEN]> {
        let own = self
            .mls
            .own_leaf()
            .ok_or(CoreError::Mls("own leaf missing".into()))?;
        identity_pk_from_credential(own.credential())
    }

    fn own_leaf_extensions(&self) -> Result<Extensions<LeafNode>> {
        leaf_extensions(
            self.credential_id,
            self.group_signing.verifying_key().to_bytes(),
        )
    }

    fn leaf_index_for(&self, credential_id: [u8; KEY_LEN]) -> Option<LeafNodeIndex> {
        self.credential_ids
            .iter()
            .find(|(_, c)| **c == credential_id)
            .map(|(i, _)| *i)
    }

    fn credential_id_at(&self, index: LeafNodeIndex) -> Option<[u8; KEY_LEN]> {
        self.credential_ids.get(&index).copied()
    }

    fn group_signing_at(&self, index: LeafNodeIndex) -> Option<nemo_wire::VerifyingKey> {
        let member = self.mls.members().find(|m| m.index == index)?;
        let arr: [u8; KEY_LEN] = member.signature_key.as_slice().try_into().ok()?;
        nemo_wire::VerifyingKey::from_bytes(&arr).ok()
    }

    fn staged_remove_matches(&self, commit: &StagedCommit, credential_id: &[u8; KEY_LEN]) -> bool {
        let mut matched = 0usize;
        for p in commit.remove_proposals() {
            if self
                .credential_id_at(p.remove_proposal().removed())
                .as_ref()
                == Some(credential_id)
            {
                matched += 1;
            } else {
                return false;
            }
        }
        matched == 1
    }

    /// Test helper: pretend the last own Update happened at `t`.
    pub fn set_last_own_update_for_test(&mut self, t: SystemTime) {
        self.last_own_update = t;
    }
}

impl Installation {
    pub fn mls_provider(&self) -> &MlsProvider {
        &self.mls_provider
    }

    pub fn encode_mls_storage(&self) -> Result<Vec<u8>> {
        let values = self
            .mls_provider
            .storage()
            .values
            .read()
            .map_err(|_| CoreError::Mls("storage lock".into()))?;
        let mut pairs: Vec<_> = values.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        let items = pairs
            .into_iter()
            .map(|(k, v)| Value::Array(vec![Value::Bytes(k.clone()), Value::Bytes(v.clone())]))
            .collect();
        Ok(cbor::encode(&Value::Array(items)))
    }

    pub fn apply_mls_storage(&self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let Value::Array(items) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let mut values = self
            .mls_provider
            .storage()
            .values
            .write()
            .map_err(|_| CoreError::Mls("storage lock".into()))?;
        values.clear();
        for item in items {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 2 {
                return Err(CoreError::VaultCorrupt);
            }
            let k = cbor::expect_bytes(&row[0])
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_vec();
            let v = cbor::expect_bytes(&row[1])
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_vec();
            values.insert(k, v);
        }
        Ok(())
    }

    pub fn create_group(&self) -> Result<Group> {
        let credential_id = random_id();
        let (mls_signer, group_signing) = new_group_keys(self.mls_provider())?;
        let credential = credential_with_key(self.identity_public_key(), &mls_signer);
        let leaf = leaf_extensions(credential_id, group_signing.verifying_key().to_bytes())?;
        let config = group_create_config(leaf)?;
        let mls = MlsGroup::new(self.mls_provider(), &mls_signer, &config, credential)
            .map_err(mls_err)?;
        if mls.ciphersuite() != CIPHERSUITE {
            return Err(CoreError::WrongCiphersuite);
        }
        let mut g = Group {
            mls,
            mls_signer,
            group_signing,
            credential_id,
            credential_ids: HashMap::new(),
            last_own_update: SystemTime::now(),
            pending: None,
            announced_keys: HashMap::new(),
        };
        g.sync_credential_ids();
        Ok(g)
    }

    /// Accept: build a KeyPackage that carries the reserved `credential_id`.
    pub fn prepare_join(&self, credential_id: [u8; KEY_LEN]) -> Result<PendingJoin> {
        let (mls_signer, group_signing) = new_group_keys(self.mls_provider())?;
        let credential = credential_with_key(self.identity_public_key(), &mls_signer);
        let leaf = leaf_extensions(credential_id, group_signing.verifying_key().to_bytes())?;
        let bundle = KeyPackage::builder()
            .leaf_node_capabilities(nemo_capabilities())
            .leaf_node_extensions(leaf)
            .build(CIPHERSUITE, self.mls_provider(), &mls_signer, credential)
            .map_err(mls_err)?;
        let key_package = bundle
            .key_package()
            .tls_serialize_detached()
            .map_err(mls_err)?;
        Ok(PendingJoin {
            credential_id,
            mls_signer,
            group_signing,
            key_package,
        })
    }
}

impl PendingJoin {
    pub fn group_signing_public(&self) -> [u8; KEY_LEN] {
        self.group_signing.verifying_key().to_bytes()
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(cbor::encode(&Value::Map(vec![
            (0, Value::Uint(1)),
            (1, Value::Bytes(self.credential_id.to_vec())),
            (2, Value::Bytes(self.group_signing.to_bytes().to_vec())),
            (3, Value::Bytes(self.key_package.clone())),
            (
                4,
                Value::Bytes(self.mls_signer.tls_serialize_detached().map_err(mls_err)?),
            ),
        ])))
    }

    pub fn decode(provider: &MlsProvider, bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if version != 1 {
            return Err(CoreError::VaultCorrupt);
        }
        let credential_id = copy_fixed(
            cbor::expect_bytes(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?,
        )
        .map_err(|_| CoreError::VaultCorrupt)?;
        let seed = copy_fixed(
            cbor::expect_bytes(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?,
        )
        .map_err(|_| CoreError::VaultCorrupt)?;
        let group_signing = SigningKey::from_bytes(&seed);
        let key_package =
            cbor::expect_bytes(cbor::map_get(&m, 3).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_vec();
        let signer_bytes =
            cbor::expect_bytes(cbor::map_get(&m, 4).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?;
        let mls_signer = SignatureKeyPair::tls_deserialize_exact(signer_bytes).map_err(mls_err)?;
        mls_signer.store(provider.storage()).map_err(mls_err)?;
        Ok(Self {
            credential_id,
            mls_signer,
            group_signing,
            key_package,
        })
    }

    pub fn is_welcome(bytes: &[u8]) -> bool {
        matches!(
            decode_mls(bytes).map(|m| m.extract()),
            Ok(MlsMessageBodyIn::Welcome(_))
        )
    }

    /// Does this Welcome address our KeyPackage? Cheap pre-check so a stray
    /// Welcome never consumes a pending join.
    pub fn welcome_matches(&self, provider: &MlsProvider, welcome_bytes: &[u8]) -> bool {
        let Ok(kp) = KeyPackageIn::tls_deserialize_exact(&self.key_package) else {
            return false;
        };
        let Ok(kp) = kp.validate(provider.crypto(), ProtocolVersion::Mls10) else {
            return false;
        };
        let Ok(kp_ref) = kp.hash_ref(provider.crypto()) else {
            return false;
        };
        match decode_mls(welcome_bytes).map(|m| m.extract()) {
            Ok(MlsMessageBodyIn::Welcome(w)) => w
                .secrets()
                .iter()
                .any(|s| s.new_member() == kp_ref),
            _ => false,
        }
    }

    pub fn join(self, provider: &MlsProvider, welcome_bytes: &[u8]) -> Result<Group> {
        let welcome = match decode_mls(welcome_bytes)?.extract() {
            MlsMessageBodyIn::Welcome(w) => w,
            _ => return Err(CoreError::Mls("expected Welcome".into())),
        };
        let config = group_create_config(leaf_extensions(
            self.credential_id,
            self.group_signing.verifying_key().to_bytes(),
        )?)?;
        let mls = StagedWelcome::new_from_welcome(provider, config.join_config(), welcome, None)
            .map_err(mls_err)?
            .into_group(provider)
            .map_err(mls_err)?;
        if mls.ciphersuite() != CIPHERSUITE {
            return Err(CoreError::WrongCiphersuite);
        }
        let mut g = Group {
            mls,
            mls_signer: self.mls_signer,
            group_signing: self.group_signing,
            credential_id: self.credential_id,
            credential_ids: HashMap::new(),
            last_own_update: SystemTime::now(),
            pending: None,
            announced_keys: HashMap::new(),
        };
        g.sync_credential_ids();
        // Our own leaf must carry the id we reserved, and every leaf in the
        // tree must be well-formed under the v1 profile.
        let own_ok = g
            .mls
            .own_leaf()
            .map(|own| check_new_leaf(own).ok() == Some(g.credential_id))
            .unwrap_or(false);
        if !own_ok {
            return Err(CoreError::CredentialIdMismatch);
        }
        let tree = g.mls.export_ratchet_tree();
        for leaf in tree.leaves() {
            check_new_leaf(leaf)?;
        }
        Ok(g)
    }
}

fn group_create_config(leaf: Extensions<LeafNode>) -> Result<MlsGroupCreateConfig> {
    Ok(MlsGroupCreateConfig::builder()
        .ciphersuite(CIPHERSUITE)
        .use_ratchet_tree_extension(true)
        .sender_ratchet_configuration(SenderRatchetConfiguration::new(1000, 2000))
        .capabilities(nemo_capabilities())
        .with_leaf_node_extensions(leaf)
        .map_err(mls_err)?
        .build())
}

fn nemo_capabilities() -> Capabilities {
    Capabilities::new(
        None,
        Some(&[CIPHERSUITE]),
        Some(&[
            ExtensionType::Unknown(CREDENTIAL_ID_EXT),
            ExtensionType::Unknown(GROUP_SIGNING_EXT),
        ]),
        None,
        Some(&[CredentialType::Basic]),
    )
}

fn leaf_extensions(
    credential_id: [u8; KEY_LEN],
    group_signing_pk: [u8; KEY_LEN],
) -> Result<Extensions<LeafNode>> {
    Extensions::from_vec(vec![
        Extension::Unknown(CREDENTIAL_ID_EXT, UnknownExtension(credential_id.to_vec())),
        Extension::Unknown(
            GROUP_SIGNING_EXT,
            UnknownExtension(group_signing_pk.to_vec()),
        ),
    ])
    .map_err(mls_err)
}

/// A leaf is valid under the v1 profile iff it has a 32-byte `0xF001` and a
/// `0xF002` equal to its own signature key. Returns the credential id.
fn check_new_leaf(leaf: &LeafNode) -> Result<[u8; KEY_LEN]> {
    let cid = extension_bytes(leaf.extensions(), CREDENTIAL_ID_EXT)
        .ok_or(CoreError::LeafExtensionViolation)?;
    let cid = copy_fixed(&cid).map_err(|_| CoreError::LeafExtensionViolation)?;
    let gsk = extension_bytes(leaf.extensions(), GROUP_SIGNING_EXT)
        .ok_or(CoreError::LeafExtensionViolation)?;
    if gsk.as_slice() != leaf.signature_key().as_slice() {
        return Err(CoreError::LeafExtensionViolation);
    }
    // Credential type and identity shape (02 §7.1).
    identity_pk_from_credential(leaf.credential())?;
    Ok(cid)
}

fn validate_key_package(provider: &MlsProvider, bytes: &[u8]) -> Result<KeyPackage> {
    KeyPackageIn::tls_deserialize_exact(bytes)
        .map_err(mls_err)?
        .validate(provider.crypto(), ProtocolVersion::Mls10)
        .map_err(mls_err)
}

fn identity_pk_from_credential(cred: &Credential) -> Result<[u8; KEY_LEN]> {
    let basic = BasicCredential::try_from(cred.clone())
        .map_err(|_| CoreError::Mls("basic credential".into()))?;
    copy_fixed(basic.identity()).map_err(|_| CoreError::Mls("identity must be 32 bytes".into()))
}

fn credential_with_key(identity_pk: [u8; KEY_LEN], signer: &SignatureKeyPair) -> CredentialWithKey {
    let credential = BasicCredential::new(identity_pk.to_vec());
    CredentialWithKey {
        credential: credential.into(),
        signature_key: signer.public().into(),
    }
}

fn new_group_keys(provider: &MlsProvider) -> Result<(SignatureKeyPair, SigningKey)> {
    let group_signing = SigningKey::generate(&mut rand::rng());
    let mls_signer = SignatureKeyPair::from_raw(
        CIPHERSUITE.signature_algorithm(),
        group_signing.to_bytes().to_vec(),
        group_signing.verifying_key().to_bytes().to_vec(),
    );
    mls_signer.store(provider.storage()).map_err(mls_err)?;
    Ok((mls_signer, group_signing))
}

fn random_id() -> [u8; KEY_LEN] {
    let mut id = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut id);
    id
}

fn pending_kind_code(kind: Option<PendingKind>) -> u64 {
    match kind {
        None => 0,
        Some(PendingKind::Update) => 1,
        Some(PendingKind::Add) => 2,
        Some(PendingKind::Remove) => 3,
        Some(PendingKind::SigningKeyRotate) => 4,
    }
}

fn pending_kind_from_code(code: u64) -> Option<PendingKind> {
    match code {
        1 => Some(PendingKind::Update),
        2 => Some(PendingKind::Add),
        3 => Some(PendingKind::Remove),
        4 => Some(PendingKind::SigningKeyRotate),
        _ => None,
    }
}

fn serialize_msg(msg: &MlsMessageOut) -> Result<Vec<u8>> {
    msg.tls_serialize_detached().map_err(mls_err)
}

fn decode_mls(bytes: &[u8]) -> Result<MlsMessageIn> {
    if let Ok(m) = MlsMessageIn::tls_deserialize_exact(bytes) {
        return Ok(m);
    }
    let mut cur = bytes;
    MlsMessageIn::tls_deserialize(&mut cur).map_err(mls_err)
}

fn extension_bytes(exts: &Extensions<LeafNode>, ty: u16) -> Option<Vec<u8>> {
    exts.unknown(ty).map(|e| e.0.clone())
}

fn is_reinit(proposal: &QueuedProposal) -> bool {
    matches!(proposal.proposal(), Proposal::ReInit(_))
}

fn is_external_commit(commit: &StagedCommit) -> bool {
    commit.queued_proposals().any(|p| {
        matches!(
            p.proposal(),
            Proposal::ExternalInit(_) | Proposal::ReInit(_)
        )
    })
}

fn staged_remove_count(commit: &StagedCommit) -> usize {
    commit.remove_proposals().count()
}
