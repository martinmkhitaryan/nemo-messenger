//! One installation is one Ed25519 identity (ADR-0003). Revocation secret is not stored.

use std::time::{SystemTime, UNIX_EPOCH};

use bip39::Mnemonic;
use libsignal_protocol::{
    kem, message_decrypt, message_encrypt, process_prekey_bundle, CiphertextMessage,
    GenericSignedPreKey, IdentityKeyPair, IdentityKeyStore, KeyPair, KyberPreKeyId,
    KyberPreKeyRecord, KyberPreKeyStore, PreKeyId, PreKeyRecord, PreKeySignalMessage, PreKeyStore,
    ProtocolAddress, SignalMessage, SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore,
    Timestamp,
};
use nemo_wire::cbor::{self, Value};
use nemo_wire::card::{ContactCard, HomeServerBinding};
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::ids::{self, IdentityId, KEY_LEN};
use nemo_wire::prekey::SignedPrekey;
use nemo_wire::revocation::RevocationStatement;
use nemo_wire::InviteeProof;
use nemo_wire::{fingerprint, identity_id, MailboxOwnerAuth, SigningKey, VerifyingKey};
use rand::RngCore;
use rand09::TryRngCore;

use crate::bundle;
use crate::discovery::{resolve_contact, Discovery};
use crate::error::{CoreError, Result};
use crate::mailbox;
use crate::store::SignalStore;
use openmls_rust_crypto::OpenMlsRustCrypto;

pub const PREKEY_STOCK_TARGET: usize = 100;
pub const PREKEY_RESTOCK_BELOW: usize = 25;
const DEVICE_NAME_HEX_LEN: usize = KEY_LEN * 2;

/// Shown once at identity creation. Never written to the vault.
#[derive(Clone, Debug)]
pub struct RevocationExport {
    pub mnemonic: String,
    pub public_key: [u8; KEY_LEN],
}

pub struct Installation {
    identity: SigningKey,
    revocation_public_key: [u8; KEY_LEN],
    address: ProtocolAddress,
    store: SignalStore,
    next_pre_key_id: u32,
    next_kyber_id: u32,
    signed_pre_key_id: SignedPreKeyId,
    binding_seq: u64,
    pub(crate) mls_provider: OpenMlsRustCrypto,
}

impl Installation {
    pub fn create() -> Result<(Self, RevocationExport)> {
        let identity = SigningKey::generate(&mut rand::rngs::OsRng);
        let revocation = SigningKey::generate(&mut rand::rngs::OsRng);
        let mnemonic = Mnemonic::from_entropy(&revocation.to_bytes())
            .map_err(|_| CoreError::Mnemonic)?
            .to_string();
        let export = RevocationExport {
            mnemonic,
            public_key: revocation.verifying_key().to_bytes(),
        };

        let mut rng = rand09::rngs::OsRng.unwrap_err();
        let libsignal_id = IdentityKeyPair::generate(&mut rng);
        let mut rid = [0u8; 4];
        rand::rngs::OsRng.fill_bytes(&mut rid);
        let registration_id = u32::from_le_bytes(rid).max(1);
        let id = identity_id(&identity.verifying_key().to_bytes());
        let address = ProtocolAddress::new(hex_bytes(&id), bundle::device_id());
        let store = SignalStore::new(libsignal_id, registration_id);

        let mut installation = Self {
            identity,
            revocation_public_key: export.public_key,
            address,
            store,
            next_pre_key_id: 1,
            next_kyber_id: 1,
            signed_pre_key_id: SignedPreKeyId::from(1u32),
            binding_seq: 0,
            mls_provider: OpenMlsRustCrypto::default(),
        };
        installation.rotate_signed_prekey()?;
        Ok((installation, export))
    }

    pub fn identity_public_key(&self) -> [u8; KEY_LEN] {
        self.identity.verifying_key().to_bytes()
    }

    pub fn identity_id(&self) -> IdentityId {
        identity_id(&self.identity_public_key())
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.identity_id())
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.identity.verifying_key()
    }

    pub fn revocation_public_key(&self) -> [u8; KEY_LEN] {
        self.revocation_public_key
    }

    pub fn sign_invitee_proof(
        &self,
        group_id: [u8; KEY_LEN],
        pending_id: [u8; KEY_LEN],
    ) -> Result<InviteeProof> {
        Ok(InviteeProof::sign(&self.identity, group_id, pending_id)?)
    }

    pub fn mailbox_owner_auth(
        &self,
        cursor: u64,
        limit: u64,
        now_unix: u64,
    ) -> Result<MailboxOwnerAuth> {
        Ok(MailboxOwnerAuth::sign(
            &self.identity,
            self.identity_id().to_vec(),
            cursor,
            limit,
            now_unix,
        )?)
    }

    pub fn unused_one_time_prekeys(&self) -> usize {
        self.store.all_pre_key_ids().count()
    }

    pub fn needs_restock(&self) -> bool {
        self.unused_one_time_prekeys() < PREKEY_RESTOCK_BELOW
    }

    /// Mint a discovery blob. Always includes a one-time EC prekey and a one-time Kyber prekey.
    pub fn mint_prekey(&mut self) -> Result<SignedPrekey> {
        let mut rng = rand09::rngs::OsRng.unwrap_err();
        let otpk_id = PreKeyId::from(self.next_pre_key_id);
        self.next_pre_key_id = self.next_pre_key_id.saturating_add(1);
        let otpk = KeyPair::generate(&mut rng);
        ready(
            self.store
                .save_pre_key(otpk_id, &PreKeyRecord::new(otpk_id, &otpk)),
        )?;

        let kyber_id = KyberPreKeyId::from(self.next_kyber_id);
        self.next_kyber_id = self.next_kyber_id.saturating_add(1);
        let identity_pair = ready(self.store.get_identity_key_pair())?;
        let kyber = KyberPreKeyRecord::generate(
            kem::KeyType::Kyber1024,
            kyber_id,
            identity_pair.private_key(),
        )?;
        ready(self.store.save_kyber_pre_key(kyber_id, &kyber))?;

        let signed = ready(self.store.get_signed_pre_key(self.signed_pre_key_id))?;
        let bundle = libsignal_protocol::PreKeyBundle::new(
            ready(self.store.get_local_registration_id())?,
            bundle::device_id(),
            Some((otpk_id, otpk.public_key)),
            signed.id()?,
            signed.public_key()?,
            signed.signature()?,
            kyber.id()?,
            kyber.public_key()?,
            kyber.signature()?,
            *identity_pair.identity_key(),
        )?;
        let opaque = bundle::encode(&bundle)?;
        Ok(SignedPrekey::sign(
            &self.identity,
            u32::from(otpk_id) as u64,
            opaque,
        )?)
    }

    pub fn restock(&mut self) -> Result<Vec<SignedPrekey>> {
        let mut out = Vec::new();
        while self.unused_one_time_prekeys() < PREKEY_STOCK_TARGET {
            out.push(self.mint_prekey()?);
        }
        Ok(out)
    }

    /// Each share mints a new share token (ADR-0007).
    pub fn mint_card(
        &mut self,
        server_hpke_public_key: [u8; KEY_LEN],
        host: &str,
        expires_at: u64,
    ) -> Result<ContactCard> {
        self.binding_seq += 1;
        let mut share_token = [0u8; KEY_LEN];
        rand::rngs::OsRng.fill_bytes(&mut share_token);
        let binding = HomeServerBinding::sign(
            &self.identity,
            HomeServerBinding {
                server_id: ids::server_id(&server_hpke_public_key),
                server_hpke_public_key,
                host: host.to_owned(),
                seq: self.binding_seq,
                expires_at,
                signature: [0; 64],
            },
        )?;
        Ok(ContactCard {
            identity_public_key: self.identity_public_key(),
            revocation_public_key: self.revocation_public_key,
            share_token,
            binding,
        })
    }

    /// First contact: resolve the current binding via discovery, then start.
    pub async fn start_session_from_discovery(
        &mut self,
        card: &ContactCard,
        discovery: &Discovery,
        prekey: &SignedPrekey,
        now_unix: u64,
    ) -> Result<()> {
        resolve_contact(card, discovery, now_unix)?;
        self.start_session(card, prekey, now_unix).await
    }

    /// Start a 1:1 after verifying the contact card and the signed prekey blob.
    /// Refuses a bundle with no one-time prekey.
    pub async fn start_session(
        &mut self,
        card: &ContactCard,
        prekey: &SignedPrekey,
        now_unix: u64,
    ) -> Result<()> {
        card.verify(now_unix)?;
        prekey.verify(&card_verifying_key(card)?)?;
        if prekey.identity_public_key != card.identity_public_key {
            return Err(CoreError::IdentityMismatch);
        }
        let bundle = bundle::decode(&prekey.libsignal_prekey)?;
        if bundle.pre_key_id()?.is_none() || bundle.pre_key_public()?.is_none() {
            return Err(CoreError::EmptyPrekeyStock);
        }
        let remote = address_for(&card.identity_id());
        let their_ls = *bundle.identity_key()?;
        if let Some(known) = self.store.get_identity(&remote).await? {
            if known != their_ls {
                return Err(CoreError::IdentityMismatch);
            }
        }
        let mut rng = rand09::rngs::OsRng.unwrap_err();
        process_prekey_bundle(
            &remote,
            &self.address,
            &mut self.store.session_store,
            &mut self.store.identity_store,
            &bundle,
            SystemTime::now(),
            &mut rng,
        )
        .await?;
        Ok(())
    }

    pub async fn encrypt(&mut self, peer: &IdentityId, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut rng = rand09::rngs::OsRng.unwrap_err();
        let remote = address_for(peer);
        let msg = message_encrypt(
            plaintext,
            &remote,
            &self.address,
            &mut self.store.session_store,
            &mut self.store.identity_store,
            SystemTime::now(),
            &mut rng,
        )
        .await?;
        Ok(msg.serialize().to_vec())
    }

    /// Double Ratchet ciphertext, then a sealed-sender mailbox envelope to the
    /// peer's home server. No sender field.
    pub async fn encrypt_to_mailbox(
        &mut self,
        peer: &IdentityId,
        dest_hpke_public: &[u8; KEY_LEN],
        delivery_capability: [u8; KEY_LEN],
        ttl_bucket: TtlBucket,
        plaintext: &[u8],
    ) -> Result<OuterEnvelope> {
        let body = self.encrypt(peer, plaintext).await?;
        mailbox::wrap(
            dest_hpke_public,
            delivery_capability,
            ttl_bucket,
            MessageType::DoubleRatchet,
            body,
        )
    }

    /// 1:1 attachment: DR wrapping file ciphertext, padded to an A* bucket.
    pub async fn encrypt_attachment_to_mailbox(
        &mut self,
        peer: &IdentityId,
        dest_hpke_public: &[u8; KEY_LEN],
        delivery_capability: [u8; KEY_LEN],
        ttl_bucket: TtlBucket,
        plaintext: &[u8],
    ) -> Result<OuterEnvelope> {
        let body = self.encrypt(peer, plaintext).await?;
        mailbox::wrap(
            dest_hpke_public,
            delivery_capability,
            ttl_bucket,
            MessageType::AttachmentDr,
            body,
        )
    }

    pub async fn decrypt_from_mailbox(
        &mut self,
        peer: &IdentityId,
        inner: &InnerEnvelope,
    ) -> Result<Vec<u8>> {
        let body = mailbox::expect_ratchet(inner)?;
        self.decrypt(peer, &parse_padded_dr(body)?).await
    }

    pub async fn decrypt(&mut self, peer: &IdentityId, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut rng = rand09::rngs::OsRng.unwrap_err();
        let remote = address_for(peer);
        let msg = parse_ciphertext(ciphertext)?;
        Ok(message_decrypt(
            &msg,
            &remote,
            &self.address,
            &mut self.store.session_store,
            &mut self.store.identity_store,
            &mut self.store.pre_key_store,
            &self.store.signed_pre_key_store,
            &mut self.store.kyber_pre_key_store,
            &mut rng,
        )
        .await?)
    }

    fn rotate_signed_prekey(&mut self) -> Result<()> {
        let mut rng = rand09::rngs::OsRng.unwrap_err();
        let identity = ready(self.store.get_identity_key_pair())?;
        let kp = KeyPair::generate(&mut rng);
        let signature = identity
            .private_key()
            .calculate_signature(&kp.public_key.serialize(), &mut rng)
            .map_err(|_| CoreError::BadKey)?
            .into_vec();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis();
        let record = SignedPreKeyRecord::new(
            self.signed_pre_key_id,
            Timestamp::from_epoch_millis(now as u64),
            &kp,
            &signature,
        );
        ready(
            self.store
                .save_signed_pre_key(self.signed_pre_key_id, &record),
        )?;
        Ok(())
    }

    pub fn encode_snapshot(&self) -> Result<Vec<u8>> {
        Ok(cbor::encode(&Value::Map(vec![
            (0, Value::Uint(1)),
            (1, Value::Bytes(self.identity.to_bytes().to_vec())),
            (2, Value::Bytes(self.revocation_public_key.to_vec())),
            (3, Value::Bytes(self.store.encode()?)),
            (4, Value::Uint(self.next_pre_key_id as u64)),
            (5, Value::Uint(self.next_kyber_id as u64)),
            (6, Value::Uint(u32::from(self.signed_pre_key_id) as u64)),
            (7, Value::Uint(self.binding_seq)),
        ])))
    }

    pub fn decode_snapshot(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if version != 1 {
            return Err(CoreError::VaultCorrupt);
        }
        let sk = cbor::expect_bytes(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if sk.len() != KEY_LEN {
            return Err(CoreError::VaultCorrupt);
        }
        let mut seed = [0u8; KEY_LEN];
        seed.copy_from_slice(sk);
        let identity = SigningKey::from_bytes(&seed);
        let rev = cbor::expect_bytes(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if rev.len() != KEY_LEN {
            return Err(CoreError::VaultCorrupt);
        }
        let mut revocation_public_key = [0u8; KEY_LEN];
        revocation_public_key.copy_from_slice(rev);
        let store_bytes =
            cbor::expect_bytes(cbor::map_get(&m, 3).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?;
        let store = SignalStore::decode(store_bytes)?;
        let next_pre_key_id =
            cbor::expect_uint(cbor::map_get(&m, 4).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)? as u32;
        let next_kyber_id =
            cbor::expect_uint(cbor::map_get(&m, 5).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)? as u32;
        let signed_pre_key_id = SignedPreKeyId::from(
            cbor::expect_uint(cbor::map_get(&m, 6).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)? as u32,
        );
        let binding_seq =
            cbor::expect_uint(cbor::map_get(&m, 7).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?;
        let id = identity_id(&identity.verifying_key().to_bytes());
        let address = ProtocolAddress::new(hex_bytes(&id), bundle::device_id());
        Ok(Self {
            identity,
            revocation_public_key,
            address,
            store,
            next_pre_key_id,
            next_kyber_id,
            signed_pre_key_id,
            binding_seq,
            mls_provider: OpenMlsRustCrypto::default(),
        })
    }
}

fn ready<T>(fut: impl std::future::Future<Output = T>) -> T {
    futures::executor::block_on(fut)
}

pub fn revocation_from_mnemonic(
    mnemonic: &str,
    identity_id: IdentityId,
    coarse_timestamp: u64,
) -> Result<RevocationStatement> {
    let m = Mnemonic::parse_normalized(mnemonic).map_err(|_| CoreError::Mnemonic)?;
    let entropy = m.to_entropy();
    if entropy.len() != KEY_LEN {
        return Err(CoreError::Mnemonic);
    }
    let mut seed = [0u8; KEY_LEN];
    seed.copy_from_slice(&entropy);
    let sk = SigningKey::from_bytes(&seed);
    Ok(RevocationStatement::sign(
        &sk,
        identity_id,
        coarse_timestamp,
    )?)
}

fn card_verifying_key(card: &ContactCard) -> Result<VerifyingKey> {
    VerifyingKey::from_bytes(&card.identity_public_key)
        .map_err(|_| CoreError::Wire(nemo_wire::WireError::Signature))
}

fn address_for(id: &IdentityId) -> ProtocolAddress {
    ProtocolAddress::new(hex_bytes(id), bundle::device_id())
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(DEVICE_NAME_HEX_LEN);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn parse_ciphertext(bytes: &[u8]) -> Result<CiphertextMessage> {
    if let Ok(m) = PreKeySignalMessage::try_from(bytes) {
        return Ok(CiphertextMessage::PreKeySignalMessage(m));
    }
    if let Ok(m) = SignalMessage::try_from(bytes) {
        return Ok(CiphertextMessage::SignalMessage(m));
    }
    Err(CoreError::BadCiphertext)
}

/// Inner padding is zeros. libsignal messages are length-delimited from the start;
/// extra trailing zeros are not part of the packet.
fn parse_padded_dr(body: &[u8]) -> Result<Vec<u8>> {
    if parse_ciphertext(body).is_ok() {
        return Ok(body.to_vec());
    }
    let last = body
        .iter()
        .rposition(|&b| b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
    for end in last..=body.len() {
        if parse_ciphertext(&body[..end]).is_ok() {
            return Ok(body[..end].to_vec());
        }
    }
    Err(CoreError::BadCiphertext)
}
