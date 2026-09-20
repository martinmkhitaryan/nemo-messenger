//! Public PQXDH prekey bundle packed as the opaque `libsignal_prekey` bstr.

use libsignal_protocol::{
    kem, DeviceId, IdentityKey, KyberPreKeyId, PreKeyBundle, PreKeyId, PublicKey, SignedPreKeyId,
};

use nemo_wire::cbor::{self, Value};
use nemo_wire::PROTOCOL_VERSION;

use crate::error::{CoreError, Result};

pub const DEVICE_ID: u8 = 1;

pub fn device_id() -> DeviceId {
    DeviceId::new(DEVICE_ID).expect("device id 1 is valid")
}

pub fn encode(bundle: &PreKeyBundle) -> Result<Vec<u8>> {
    let otpk_id = bundle.pre_key_id()?.ok_or(CoreError::EmptyPrekeyStock)?;
    let otpk = bundle
        .pre_key_public()?
        .ok_or(CoreError::EmptyPrekeyStock)?;
    Ok(cbor::encode(&Value::Map(vec![
        (0, Value::Uint(PROTOCOL_VERSION as u64)),
        (1, Value::Uint(bundle.registration_id()? as u64)),
        (
            2,
            Value::Uint(u32::from(bundle.signed_pre_key_id()?) as u64),
        ),
        (
            3,
            Value::Bytes(bundle.signed_pre_key_public()?.serialize().to_vec()),
        ),
        (4, Value::Bytes(bundle.signed_pre_key_signature()?.to_vec())),
        (5, Value::Uint(u32::from(otpk_id) as u64)),
        (6, Value::Bytes(otpk.serialize().to_vec())),
        (7, Value::Uint(u32::from(bundle.kyber_pre_key_id()?) as u64)),
        (
            8,
            Value::Bytes(bundle.kyber_pre_key_public()?.serialize().to_vec()),
        ),
        (9, Value::Bytes(bundle.kyber_pre_key_signature()?.to_vec())),
        (
            10,
            Value::Bytes(bundle.identity_key()?.serialize().to_vec()),
        ),
    ])))
}

pub fn decode(bytes: &[u8]) -> Result<PreKeyBundle> {
    let Value::Map(m) = cbor::decode(bytes)? else {
        return Err(CoreError::Wire(nemo_wire::WireError::Cbor(
            "prekey bundle must be a map",
        )));
    };
    let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
    if version != PROTOCOL_VERSION as u64 {
        return Err(CoreError::Wire(nemo_wire::WireError::UnknownVersion(
            version,
        )));
    }
    let registration_id = cbor::expect_uint(cbor::map_get(&m, 1)?)? as u32;
    let signed_pre_key_id = SignedPreKeyId::from(cbor::expect_uint(cbor::map_get(&m, 2)?)? as u32);
    let signed_pre_key_public = PublicKey::deserialize(cbor::expect_bytes(cbor::map_get(&m, 3)?)?)
        .map_err(|_| CoreError::BadKey)?;
    let signed_pre_key_signature = cbor::expect_bytes(cbor::map_get(&m, 4)?)?.to_vec();
    let otpk_id = PreKeyId::from(cbor::expect_uint(cbor::map_get(&m, 5)?)? as u32);
    let otpk = PublicKey::deserialize(cbor::expect_bytes(cbor::map_get(&m, 6)?)?)
        .map_err(|_| CoreError::BadKey)?;
    let kyber_id = KyberPreKeyId::from(cbor::expect_uint(cbor::map_get(&m, 7)?)? as u32);
    let kyber_pk = kem::PublicKey::deserialize(cbor::expect_bytes(cbor::map_get(&m, 8)?)?)?;
    let kyber_sig = cbor::expect_bytes(cbor::map_get(&m, 9)?)?.to_vec();
    let identity_key = IdentityKey::decode(cbor::expect_bytes(cbor::map_get(&m, 10)?)?)?;
    Ok(PreKeyBundle::new(
        registration_id,
        device_id(),
        Some((otpk_id, otpk)),
        signed_pre_key_id,
        signed_pre_key_public,
        signed_pre_key_signature,
        kyber_id,
        kyber_pk,
        kyber_sig,
        identity_key,
    )?)
}
