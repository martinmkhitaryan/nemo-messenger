//! MLS application profile (RFC 9420 suite `0x0003`).
//!
//! Join is invite → accept → admit. Clients reject External Commits, external
//! joins, ReInit, and an unframed Remove. A Remove is applied only as a
//! [`nemo_wire::RemoveBundle`].
//!
//! The per-group signing key (leaf extension `0xF002`) is the MLS leaf
//! signature key, so invite/admit/sidecar and MLS Commits use one key.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::hostframe::RemoveBundle;
use nemo_wire::ids::{copy_fixed, KEY_LEN};
use nemo_wire::{GroupAdmit, GroupInvite, SigningKey, INTRO_TTL_30_MIN};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use rand::RngCore;
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

pub struct Group {
    mls: MlsGroup,
    mls_signer: SignatureKeyPair,
    group_signing: SigningKey,
    credential_id: [u8; KEY_LEN],
    /// MLS leaf signature key → host `credential_id` (`0xF001`).
    credential_ids: HashMap<Vec<u8>, [u8; KEY_LEN]>,
    last_own_update: SystemTime,
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
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let bind = bind_identity_pk.map(|pk| {
            let mut salt = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut salt);
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
        let processed = self.process(provider, ciphertext)?;
        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(m) => Ok(m.into_bytes()),
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
        let body = mailbox::expect_type(inner, MessageType::MlsApp)?;
        self.decrypt(provider, body)
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
    ) -> Result<()> {
        let body = mailbox::expect_type(inner, MessageType::MlsHandshake)?;
        self.apply_handshake(provider, body)
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
        inner: &InnerEnvelope,
    ) -> Result<()> {
        let body = mailbox::expect_type(inner, MessageType::RemoveBundle)?;
        let bundle = RemoveBundle::decode(body)?;
        self.apply_remove_bundle(provider, &bundle)
    }

    /// Apply a non-Remove handshake (Add, Update).
    pub fn apply_handshake(&mut self, provider: &MlsProvider, message: &[u8]) -> Result<()> {
        let processed = self.process(provider, message)?;
        match processed.into_content() {
            ProcessedMessageContent::StagedCommitMessage(commit) => {
                if is_external_commit(&commit) {
                    return Err(CoreError::ExternalMlsJoin);
                }
                if staged_remove_count(&commit) > 0 {
                    return Err(CoreError::UnframedRemove);
                }
                self.note_adds(&commit);
                self.mls
                    .merge_staged_commit(provider, *commit)
                    .map_err(mls_err)?;
                Ok(())
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
        }
    }

    pub fn self_update(&mut self, provider: &MlsProvider) -> Result<Vec<u8>> {
        let params = LeafNodeParameters::builder()
            .with_capabilities(nemo_capabilities())
            .with_extensions(self.own_leaf_extensions()?)
            .build();
        let (msg, _welcome, _info) = self
            .mls
            .self_update(provider, &self.mls_signer, params)
            .map_err(mls_err)?;
        self.mls.merge_pending_commit(provider).map_err(mls_err)?;
        self.last_own_update = SystemTime::now();
        serialize_msg(&msg)
    }

    pub fn admit(
        &mut self,
        provider: &MlsProvider,
        key_package_bytes: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let kp = KeyPackageIn::tls_deserialize_exact(key_package_bytes)
            .map_err(mls_err)?
            .validate(provider.crypto(), ProtocolVersion::Mls10)
            .map_err(mls_err)?;
        if kp.ciphersuite() != CIPHERSUITE {
            return Err(CoreError::WrongCiphersuite);
        }
        self.note_leaf(kp.leaf_node())?;
        let (commit, welcome, _info) = self
            .mls
            .add_members(provider, &self.mls_signer, core::slice::from_ref(&kp))
            .map_err(mls_err)?;
        self.mls.merge_pending_commit(provider).map_err(mls_err)?;
        self.last_own_update = SystemTime::now();
        Ok((serialize_msg(&commit)?, serialize_msg(&welcome)?))
    }

    pub fn remove(
        &mut self,
        provider: &MlsProvider,
        credential_id: [u8; KEY_LEN],
    ) -> Result<RemoveBundle> {
        let index = self
            .leaf_index_for(credential_id)
            .ok_or(CoreError::BadRemoveBundle)?;
        let (commit, _welcome, _info) = self
            .mls
            .remove_members(provider, &self.mls_signer, &[index])
            .map_err(mls_err)?;
        self.mls.merge_pending_commit(provider).map_err(mls_err)?;
        self.last_own_update = SystemTime::now();
        Ok(RemoveBundle::sign(
            &self.group_signing,
            credential_id,
            serialize_msg(&commit)?,
        )?)
    }

    pub fn apply_remove_bundle(
        &mut self,
        provider: &MlsProvider,
        bundle: &RemoveBundle,
    ) -> Result<()> {
        let processed = self.process(provider, &bundle.mls_commit)?;
        let sender_pk = match processed.sender() {
            Sender::Member(index) => self
                .group_signing_at(*index)
                .ok_or(CoreError::BadRemoveBundle)?,
            _ => return Err(CoreError::ExternalMlsJoin),
        };
        bundle.verify_sidecar(&sender_pk)?;
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
                self.mls
                    .merge_staged_commit(provider, *commit)
                    .map_err(mls_err)?;
                Ok(())
            }
            _ => Err(CoreError::Mls("RemoveBundle body is not a Commit".into())),
        }
    }

    fn process(&mut self, provider: &MlsProvider, bytes: &[u8]) -> Result<ProcessedMessage> {
        let msg = decode_mls(bytes)?;
        let protocol = msg.try_into_protocol_message().map_err(mls_err)?;
        self.mls
            .process_message(provider, protocol)
            .map_err(mls_err)
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

    fn own_leaf_extensions(&self) -> Result<Extensions> {
        leaf_extensions(
            self.credential_id,
            self.group_signing.verifying_key().to_bytes(),
        )
    }

    fn note_leaf(&mut self, leaf: &LeafNode) -> Result<()> {
        let cid = extension_bytes(leaf.extensions(), CREDENTIAL_ID_EXT)
            .ok_or(CoreError::Mls("missing 0xF001".into()))?;
        let cid = copy_fixed(&cid).map_err(|_| CoreError::Mls("0xF001 must be 32 bytes".into()))?;
        let sig = leaf.signature_key().as_slice().to_vec();
        if let Some(prev) = self.credential_ids.get(&sig) {
            if *prev != cid {
                return Err(CoreError::Mls("0xF001 is immutable".into()));
            }
        }
        self.credential_ids.insert(sig, cid);
        Ok(())
    }

    fn note_adds(&mut self, commit: &StagedCommit) {
        for add in commit.add_proposals() {
            let _ = self.note_leaf(add.add_proposal().key_package().leaf_node());
        }
    }

    fn leaf_index_for(&self, credential_id: [u8; KEY_LEN]) -> Option<LeafNodeIndex> {
        self.mls.members().find_map(|m| {
            if self.credential_ids.get(&m.signature_key).copied() == Some(credential_id) {
                Some(m.index)
            } else {
                None
            }
        })
    }

    fn credential_id_at(&self, index: LeafNodeIndex) -> Option<[u8; KEY_LEN]> {
        let member = self.mls.members().find(|m| m.index == index)?;
        self.credential_ids.get(&member.signature_key).copied()
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
        let mut credential_ids = HashMap::new();
        credential_ids.insert(mls_signer.public().to_vec(), credential_id);
        Ok(Group {
            mls,
            mls_signer,
            group_signing,
            credential_id,
            credential_ids,
            last_own_update: SystemTime::now(),
        })
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
        let mut credential_ids = HashMap::new();
        credential_ids.insert(self.mls_signer.public().to_vec(), self.credential_id);
        if let Some(own) = mls.own_leaf() {
            if let Some(cid) = extension_bytes(own.extensions(), CREDENTIAL_ID_EXT) {
                if let Ok(id) = copy_fixed(&cid) {
                    credential_ids.insert(own.signature_key().as_slice().to_vec(), id);
                }
            }
        }
        Ok(Group {
            mls,
            mls_signer: self.mls_signer,
            group_signing: self.group_signing,
            credential_id: self.credential_id,
            credential_ids,
            last_own_update: SystemTime::now(),
        })
    }
}

fn group_create_config(leaf: Extensions) -> Result<MlsGroupCreateConfig> {
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
) -> Result<Extensions> {
    Extensions::from_vec(vec![
        Extension::Unknown(CREDENTIAL_ID_EXT, UnknownExtension(credential_id.to_vec())),
        Extension::Unknown(
            GROUP_SIGNING_EXT,
            UnknownExtension(group_signing_pk.to_vec()),
        ),
    ])
    .map_err(mls_err)
}

fn credential_with_key(identity_pk: [u8; KEY_LEN], signer: &SignatureKeyPair) -> CredentialWithKey {
    let credential = BasicCredential::new(identity_pk.to_vec());
    CredentialWithKey {
        credential: credential.into(),
        signature_key: signer.public().into(),
    }
}

fn new_group_keys(provider: &MlsProvider) -> Result<(SignatureKeyPair, SigningKey)> {
    let group_signing = SigningKey::generate(&mut rand::rngs::OsRng);
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
    rand::rngs::OsRng.fill_bytes(&mut id);
    id
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

fn extension_bytes(exts: &Extensions, ty: u16) -> Option<Vec<u8>> {
    for ext in exts.iter() {
        if let Extension::Unknown(t, UnknownExtension(data)) = ext {
            if *t == ty {
                return Some(data.clone());
            }
        }
    }
    None
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
