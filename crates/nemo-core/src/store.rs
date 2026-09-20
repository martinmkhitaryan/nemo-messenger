//! Ownable libsignal stores so an installation can be snapshotted into the vault.
//!
//! `InMemSignalProtocolStore` keeps session and identity maps private, so a
//! restart would drop Double Ratchet state. These maps are the same shape with
//! a CBOR dump.

use std::collections::HashMap;

use async_trait::async_trait;
use libsignal_protocol::{
    CiphertextMessageType, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KyberPreKeyId, KyberPreKeyRecord, KyberPreKeyStore,
    PreKeyId, PreKeyRecord, PreKeyStore, ProtocolAddress, PublicKey, SessionRecord, SessionStore,
    SignalProtocolError, SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore,
};
use nemo_wire::cbor::{self, Value};

use crate::bundle;
use crate::error::{CoreError, Result};

type SignalResult<T> = std::result::Result<T, SignalProtocolError>;

#[derive(Clone)]
pub struct Identities {
    key_pair: IdentityKeyPair,
    registration_id: u32,
    known: HashMap<ProtocolAddress, IdentityKey>,
}

impl Identities {
    fn new(key_pair: IdentityKeyPair, registration_id: u32) -> Self {
        Self {
            key_pair,
            registration_id,
            known: HashMap::new(),
        }
    }
}

#[async_trait(?Send)]
impl IdentityKeyStore for Identities {
    async fn get_identity_key_pair(&self) -> SignalResult<IdentityKeyPair> {
        Ok(self.key_pair)
    }

    async fn get_local_registration_id(&self) -> SignalResult<u32> {
        Ok(self.registration_id)
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> SignalResult<IdentityChange> {
        match self.known.get(address) {
            None => {
                self.known.insert(address.clone(), *identity);
                Ok(IdentityChange::NewOrUnchanged)
            }
            Some(k) if k == identity => Ok(IdentityChange::NewOrUnchanged),
            Some(_) => {
                self.known.insert(address.clone(), *identity);
                Ok(IdentityChange::ReplacedExisting)
            }
        }
    }

    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        _direction: Direction,
    ) -> SignalResult<bool> {
        Ok(self.known.get(address).map(|k| k == identity).unwrap_or(true))
    }

    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> SignalResult<Option<IdentityKey>> {
        Ok(self.known.get(address).copied())
    }
}

#[derive(Clone, Default)]
pub struct PreKeys {
    keys: HashMap<PreKeyId, PreKeyRecord>,
}

#[async_trait(?Send)]
impl PreKeyStore for PreKeys {
    async fn get_pre_key(&self, id: PreKeyId) -> SignalResult<PreKeyRecord> {
        self.keys
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidPreKeyId)
    }

    async fn save_pre_key(
        &mut self,
        id: PreKeyId,
        record: &PreKeyRecord,
    ) -> SignalResult<()> {
        self.keys.insert(id, record.clone());
        Ok(())
    }

    async fn remove_pre_key(&mut self, id: PreKeyId) -> SignalResult<()> {
        self.keys.remove(&id);
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct SignedPreKeys {
    keys: HashMap<SignedPreKeyId, SignedPreKeyRecord>,
}

#[async_trait(?Send)]
impl SignedPreKeyStore for SignedPreKeys {
    async fn get_signed_pre_key(
        &self,
        id: SignedPreKeyId,
    ) -> SignalResult<SignedPreKeyRecord> {
        self.keys
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidSignedPreKeyId)
    }

    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> SignalResult<()> {
        self.keys.insert(id, record.clone());
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct KyberPreKeys {
    keys: HashMap<KyberPreKeyId, KyberPreKeyRecord>,
    used: HashMap<(KyberPreKeyId, SignedPreKeyId), Vec<PublicKey>>,
}

#[async_trait(?Send)]
impl KyberPreKeyStore for KyberPreKeys {
    async fn get_kyber_pre_key(
        &self,
        id: KyberPreKeyId,
    ) -> SignalResult<KyberPreKeyRecord> {
        self.keys
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidKyberPreKeyId)
    }

    async fn save_kyber_pre_key(
        &mut self,
        id: KyberPreKeyId,
        record: &KyberPreKeyRecord,
    ) -> SignalResult<()> {
        self.keys.insert(id, record.clone());
        Ok(())
    }

    async fn mark_kyber_pre_key_used(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        ec_prekey_id: SignedPreKeyId,
        base_key: &PublicKey,
    ) -> SignalResult<()> {
        let seen = self
            .used
            .entry((kyber_prekey_id, ec_prekey_id))
            .or_default();
        if seen.contains(base_key) {
            return Err(SignalProtocolError::InvalidMessage(
                CiphertextMessageType::PreKey,
                "reused base key".to_owned(),
            ));
        }
        seen.push(*base_key);
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct Sessions {
    sessions: HashMap<ProtocolAddress, SessionRecord>,
}

#[async_trait(?Send)]
impl SessionStore for Sessions {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> SignalResult<Option<SessionRecord>> {
        Ok(self.sessions.get(address).cloned())
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> SignalResult<()> {
        self.sessions.insert(address.clone(), record.clone());
        Ok(())
    }
}

#[derive(Clone)]
pub struct SignalStore {
    pub session_store: Sessions,
    pub pre_key_store: PreKeys,
    pub signed_pre_key_store: SignedPreKeys,
    pub kyber_pre_key_store: KyberPreKeys,
    pub identity_store: Identities,
}

impl SignalStore {
    pub fn new(key_pair: IdentityKeyPair, registration_id: u32) -> Self {
        Self {
            session_store: Sessions::default(),
            pre_key_store: PreKeys::default(),
            signed_pre_key_store: SignedPreKeys::default(),
            kyber_pre_key_store: KyberPreKeys::default(),
            identity_store: Identities::new(key_pair, registration_id),
        }
    }

    pub fn all_pre_key_ids(&self) -> impl Iterator<Item = &PreKeyId> {
        self.pre_key_store.keys.keys()
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(cbor::encode(&self.to_cbor()?))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        Self::from_map(&m)
    }

    fn to_cbor(&self) -> Result<Value> {
        let mut known = Vec::new();
        let mut addrs: Vec<_> = self.identity_store.known.keys().cloned().collect();
        sort_addrs(&mut addrs);
        for addr in addrs {
            let id = self.identity_store.known[&addr];
            known.push(addr_entry(&addr, id.serialize().to_vec()));
        }

        let mut prekeys = Vec::new();
        let mut ids: Vec<_> = self.pre_key_store.keys.keys().copied().collect();
        ids.sort();
        for id in ids {
            let rec = &self.pre_key_store.keys[&id];
            prekeys.push(Value::Array(vec![
                Value::Uint(u32::from(id) as u64),
                Value::Bytes(rec.serialize()?),
            ]));
        }

        let mut signed = Vec::new();
        let mut sids: Vec<_> = self.signed_pre_key_store.keys.keys().copied().collect();
        sids.sort();
        for id in sids {
            let rec = &self.signed_pre_key_store.keys[&id];
            signed.push(Value::Array(vec![
                Value::Uint(u32::from(id) as u64),
                Value::Bytes(rec.serialize()?),
            ]));
        }

        let mut kyber = Vec::new();
        let mut kids: Vec<_> = self.kyber_pre_key_store.keys.keys().copied().collect();
        kids.sort();
        for id in kids {
            let rec = &self.kyber_pre_key_store.keys[&id];
            kyber.push(Value::Array(vec![
                Value::Uint(u32::from(id) as u64),
                Value::Bytes(rec.serialize()?),
            ]));
        }

        let mut used = Vec::new();
        let mut ukeys: Vec<_> = self.kyber_pre_key_store.used.keys().copied().collect();
        ukeys.sort_by_key(|(k, s)| (u32::from(*k), u32::from(*s)));
        for (kid, sid) in ukeys {
            for pk in &self.kyber_pre_key_store.used[&(kid, sid)] {
                used.push(Value::Array(vec![
                    Value::Uint(u32::from(kid) as u64),
                    Value::Uint(u32::from(sid) as u64),
                    Value::Bytes(pk.serialize().to_vec()),
                ]));
            }
        }

        let mut sessions = Vec::new();
        let mut saddrs: Vec<_> = self.session_store.sessions.keys().cloned().collect();
        sort_addrs(&mut saddrs);
        for addr in saddrs {
            let rec = &self.session_store.sessions[&addr];
            sessions.push(Value::Array(vec![
                Value::Text(addr.name().to_owned()),
                Value::Uint(u32::from(addr.device_id()) as u64),
                Value::Bytes(rec.serialize()?),
            ]));
        }

        Ok(Value::Map(vec![
            (
                0,
                Value::Bytes(self.identity_store.key_pair.serialize().to_vec()),
            ),
            (1, Value::Uint(self.identity_store.registration_id as u64)),
            (2, Value::Array(known)),
            (3, Value::Array(prekeys)),
            (4, Value::Array(signed)),
            (5, Value::Array(kyber)),
            (6, Value::Array(used)),
            (7, Value::Array(sessions)),
        ]))
    }

    fn from_map(m: &[(u64, Value)]) -> Result<Self> {
        let pair_bytes = cbor::expect_bytes(cbor::map_get(m, 0).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        let key_pair = IdentityKeyPair::try_from(pair_bytes).map_err(|_| CoreError::VaultCorrupt)?;
        let registration_id =
            cbor::expect_uint(cbor::map_get(m, 1).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)? as u32;
        let mut store = Self::new(key_pair, registration_id);

        for item in array_at(m, 2)? {
            let (addr, bytes) = parse_addr_bytes(item)?;
            let id = IdentityKey::decode(bytes).map_err(|_| CoreError::VaultCorrupt)?;
            store.identity_store.known.insert(addr, id);
        }
        for item in array_at(m, 3)? {
            let (id, bytes) = parse_id_bytes(item)?;
            let rec = PreKeyRecord::deserialize(bytes).map_err(|_| CoreError::VaultCorrupt)?;
            store.pre_key_store.keys.insert(PreKeyId::from(id), rec);
        }
        for item in array_at(m, 4)? {
            let (id, bytes) = parse_id_bytes(item)?;
            let rec =
                SignedPreKeyRecord::deserialize(bytes).map_err(|_| CoreError::VaultCorrupt)?;
            store
                .signed_pre_key_store
                .keys
                .insert(SignedPreKeyId::from(id), rec);
        }
        for item in array_at(m, 5)? {
            let (id, bytes) = parse_id_bytes(item)?;
            let rec = KyberPreKeyRecord::deserialize(bytes).map_err(|_| CoreError::VaultCorrupt)?;
            store
                .kyber_pre_key_store
                .keys
                .insert(KyberPreKeyId::from(id), rec);
        }
        for item in array_at(m, 6)? {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 3 {
                return Err(CoreError::VaultCorrupt);
            }
            let kid = cbor::expect_uint(&row[0]).map_err(|_| CoreError::VaultCorrupt)? as u32;
            let sid = cbor::expect_uint(&row[1]).map_err(|_| CoreError::VaultCorrupt)? as u32;
            let pk = PublicKey::deserialize(
                cbor::expect_bytes(&row[2]).map_err(|_| CoreError::VaultCorrupt)?,
            )
            .map_err(|_| CoreError::VaultCorrupt)?;
            store
                .kyber_pre_key_store
                .used
                .entry((KyberPreKeyId::from(kid), SignedPreKeyId::from(sid)))
                .or_default()
                .push(pk);
        }
        for item in array_at(m, 7)? {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 3 {
                return Err(CoreError::VaultCorrupt);
            }
            let name = cbor::expect_text(&row[0])
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_owned();
            let dev = cbor::expect_uint(&row[1]).map_err(|_| CoreError::VaultCorrupt)?;
            let rec = SessionRecord::deserialize(
                cbor::expect_bytes(&row[2]).map_err(|_| CoreError::VaultCorrupt)?,
            )
            .map_err(|_| CoreError::VaultCorrupt)?;
            store
                .session_store
                .sessions
                .insert(address(name, dev)?, rec);
        }
        Ok(store)
    }
}

#[async_trait(?Send)]
impl IdentityKeyStore for SignalStore {
    async fn get_identity_key_pair(&self) -> SignalResult<IdentityKeyPair> {
        self.identity_store.get_identity_key_pair().await
    }

    async fn get_local_registration_id(&self) -> SignalResult<u32> {
        self.identity_store.get_local_registration_id().await
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> SignalResult<IdentityChange> {
        self.identity_store.save_identity(address, identity).await
    }

    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        direction: Direction,
    ) -> SignalResult<bool> {
        self.identity_store
            .is_trusted_identity(address, identity, direction)
            .await
    }

    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> SignalResult<Option<IdentityKey>> {
        self.identity_store.get_identity(address).await
    }
}

#[async_trait(?Send)]
impl PreKeyStore for SignalStore {
    async fn get_pre_key(&self, id: PreKeyId) -> SignalResult<PreKeyRecord> {
        self.pre_key_store.get_pre_key(id).await
    }

    async fn save_pre_key(
        &mut self,
        id: PreKeyId,
        record: &PreKeyRecord,
    ) -> SignalResult<()> {
        self.pre_key_store.save_pre_key(id, record).await
    }

    async fn remove_pre_key(&mut self, id: PreKeyId) -> SignalResult<()> {
        self.pre_key_store.remove_pre_key(id).await
    }
}

#[async_trait(?Send)]
impl SignedPreKeyStore for SignalStore {
    async fn get_signed_pre_key(
        &self,
        id: SignedPreKeyId,
    ) -> SignalResult<SignedPreKeyRecord> {
        self.signed_pre_key_store.get_signed_pre_key(id).await
    }

    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> SignalResult<()> {
        self.signed_pre_key_store
            .save_signed_pre_key(id, record)
            .await
    }
}

#[async_trait(?Send)]
impl KyberPreKeyStore for SignalStore {
    async fn get_kyber_pre_key(
        &self,
        id: KyberPreKeyId,
    ) -> SignalResult<KyberPreKeyRecord> {
        self.kyber_pre_key_store.get_kyber_pre_key(id).await
    }

    async fn save_kyber_pre_key(
        &mut self,
        id: KyberPreKeyId,
        record: &KyberPreKeyRecord,
    ) -> SignalResult<()> {
        self.kyber_pre_key_store.save_kyber_pre_key(id, record).await
    }

    async fn mark_kyber_pre_key_used(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        ec_prekey_id: SignedPreKeyId,
        base_key: &PublicKey,
    ) -> SignalResult<()> {
        self.kyber_pre_key_store
            .mark_kyber_pre_key_used(kyber_prekey_id, ec_prekey_id, base_key)
            .await
    }
}

fn sort_addrs(addrs: &mut [ProtocolAddress]) {
    addrs.sort_by(|a, b| {
        a.name()
            .cmp(b.name())
            .then(u32::from(a.device_id()).cmp(&u32::from(b.device_id())))
    });
}

fn addr_entry(addr: &ProtocolAddress, identity: Vec<u8>) -> Value {
    Value::Array(vec![
        Value::Text(addr.name().to_owned()),
        Value::Uint(u32::from(addr.device_id()) as u64),
        Value::Bytes(identity),
    ])
}

fn array_at<'a>(m: &'a [(u64, Value)], key: u64) -> Result<&'a [Value]> {
    cbor::expect_array(cbor::map_get(m, key).map_err(|_| CoreError::VaultCorrupt)?)
        .map_err(|_| CoreError::VaultCorrupt)
}

fn parse_addr_bytes(item: &Value) -> Result<(ProtocolAddress, &[u8])> {
    let Value::Array(row) = item else {
        return Err(CoreError::VaultCorrupt);
    };
    if row.len() != 3 {
        return Err(CoreError::VaultCorrupt);
    }
    let name = cbor::expect_text(&row[0])
        .map_err(|_| CoreError::VaultCorrupt)?
        .to_owned();
    let dev = cbor::expect_uint(&row[1]).map_err(|_| CoreError::VaultCorrupt)?;
    let bytes = cbor::expect_bytes(&row[2]).map_err(|_| CoreError::VaultCorrupt)?;
    Ok((address(name, dev)?, bytes))
}

fn parse_id_bytes(item: &Value) -> Result<(u32, &[u8])> {
    let Value::Array(row) = item else {
        return Err(CoreError::VaultCorrupt);
    };
    if row.len() != 2 {
        return Err(CoreError::VaultCorrupt);
    }
    let id = cbor::expect_uint(&row[0]).map_err(|_| CoreError::VaultCorrupt)? as u32;
    let bytes = cbor::expect_bytes(&row[1]).map_err(|_| CoreError::VaultCorrupt)?;
    Ok((id, bytes))
}

fn address(name: String, device: u64) -> Result<ProtocolAddress> {
    if device > 127 {
        return Err(CoreError::VaultCorrupt);
    }
    let id = bundle::device_id();
    if device == u32::from(id) as u64 {
        return Ok(ProtocolAddress::new(name, id));
    }
    let dev = u8::try_from(device).map_err(|_| CoreError::VaultCorrupt)?;
    let device_id = libsignal_protocol::DeviceId::new(dev).map_err(|_| CoreError::VaultCorrupt)?;
    Ok(ProtocolAddress::new(name, device_id))
}
