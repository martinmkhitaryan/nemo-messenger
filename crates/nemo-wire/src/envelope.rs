use crate::error::{Result, WireError};
use crate::ids::{copy_fixed, KEY_LEN};
use crate::PROTOCOL_VERSION;

pub const T1: usize = 1024;
pub const T2: usize = 4096;
pub const T3: usize = 16384;
pub const A1: usize = 262_144;
pub const A2: usize = 1_048_576;
pub const A3: usize = 4_194_304;
pub const A4: usize = 16_777_216;

pub const INNER_TEXT_OUTER: usize = 20_480;
pub const A1_OUTER: usize = 266_240;
pub const A2_OUTER: usize = 1_052_672;
pub const A3_OUTER: usize = 4_202_496;
pub const A4_OUTER: usize = 16_781_312;

const TEXT_INNER: [usize; 3] = [T1, T2, T3];
const ATTACH_INNER: [usize; 4] = [A1, A2, A3, A4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    DoubleRatchet = 0x01,
    MlsApp = 0x02,
    AttachmentDr = 0x03,
    RemoveBundle = 0x10,
    SigningKeyReplace = 0x11,
    AttachmentReserve = 0x12,
    RevocationStatement = 0x13,
    MlsHandshake = 0x14,
}

impl MessageType {
    pub fn from_u8(v: u8) -> Result<Self> {
        Ok(match v {
            0x01 => Self::DoubleRatchet,
            0x02 => Self::MlsApp,
            0x03 => Self::AttachmentDr,
            0x10 => Self::RemoveBundle,
            0x11 => Self::SigningKeyReplace,
            0x12 => Self::AttachmentReserve,
            0x13 => Self::RevocationStatement,
            0x14 => Self::MlsHandshake,
            other => return Err(WireError::UnknownMessageType(other)),
        })
    }

    pub fn is_attachment(self) -> bool {
        matches!(self, Self::AttachmentDr)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TtlBucket(pub u8);

impl TtlBucket {
    pub const DEFAULT: Self = Self(0);
    pub const SECONDS_60: Self = Self(1);
    pub const HOUR: Self = Self(2);
    pub const DAY: Self = Self(3);

    pub fn from_u8(v: u8) -> Result<Self> {
        match v {
            0..=3 => Ok(Self(v)),
            other => Err(WireError::UnknownTtl(other)),
        }
    }

    pub fn ttl_secs(self) -> Option<u64> {
        match self.0 {
            0 => None,
            1 => Some(60),
            2 => Some(3600),
            3 => Some(86400),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaddedMessage {
    pub type_: MessageType,
    pub body: Vec<u8>,
    pub inner_len: usize,
}

impl PaddedMessage {
    pub fn pad(type_: MessageType, body: Vec<u8>) -> Result<Self> {
        let header_and_body = 2 + body.len();
        let buckets: &[usize] = if type_.is_attachment() {
            &ATTACH_INNER
        } else {
            &TEXT_INNER
        };
        let inner_len = *buckets
            .iter()
            .find(|b| **b >= header_and_body)
            .ok_or(WireError::NoBucket)?;
        Ok(Self {
            type_,
            body,
            inner_len,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![PROTOCOL_VERSION, self.type_ as u8];
        out.extend_from_slice(&self.body);
        out.resize(self.inner_len, 0);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(WireError::BadEnvelope);
        }
        if bytes[0] != PROTOCOL_VERSION {
            return Err(WireError::UnknownVersion(bytes[0] as u64));
        }
        let type_ = MessageType::from_u8(bytes[1])?;
        let expected: &[usize] = if type_.is_attachment() {
            &ATTACH_INNER
        } else {
            &TEXT_INNER
        };
        if !expected.contains(&bytes.len()) {
            return Err(WireError::BadEnvelope);
        }
        // Keep padding zeros. Host-framed types carry their own lengths. Opaque
        // DR/MLS bodies are parsed by the next layer from this slice.
        Ok(Self {
            type_,
            body: bytes[2..].to_vec(),
            inner_len: bytes.len(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InnerEnvelope {
    pub delivery_capability: [u8; KEY_LEN],
    pub ttl_bucket: TtlBucket,
    pub idempotency_token: [u8; KEY_LEN],
    pub padded_message: PaddedMessage,
}

impl InnerEnvelope {
    fn header_len() -> usize {
        1 + KEY_LEN + 1 + KEY_LEN
    }

    pub fn outer_len(&self) -> usize {
        if self.padded_message.type_.is_attachment() {
            match self.padded_message.inner_len {
                A1 => A1_OUTER,
                A2 => A2_OUTER,
                A3 => A3_OUTER,
                A4 => A4_OUTER,
                _ => INNER_TEXT_OUTER,
            }
        } else {
            INNER_TEXT_OUTER
        }
    }

    pub fn encode_padded(&self) -> Result<Vec<u8>> {
        let msg = self.padded_message.encode();
        let mut out = Vec::with_capacity(self.outer_len());
        out.push(PROTOCOL_VERSION);
        out.extend_from_slice(&self.delivery_capability);
        out.push(self.ttl_bucket.0);
        out.extend_from_slice(&self.idempotency_token);
        out.extend_from_slice(&msg);
        let outer = self.outer_len();
        if out.len() > outer {
            return Err(WireError::NoBucket);
        }
        out.resize(outer, 0);
        Ok(out)
    }

    pub fn decode_padded(bytes: &[u8]) -> Result<Self> {
        let hdr = Self::header_len();
        if bytes.len() < hdr + 2 {
            return Err(WireError::BadEnvelope);
        }
        if bytes[0] != PROTOCOL_VERSION {
            return Err(WireError::UnknownVersion(bytes[0] as u64));
        }
        let delivery_capability = copy_fixed(&bytes[1..1 + KEY_LEN])?;
        let ttl_bucket = TtlBucket::from_u8(bytes[1 + KEY_LEN])?;
        let idemp_off = 1 + KEY_LEN + 1;
        let idempotency_token = copy_fixed(&bytes[idemp_off..idemp_off + KEY_LEN])?;
        let rest = &bytes[hdr..];
        let inner_len = if bytes.len() == INNER_TEXT_OUTER {
            // Smallest T* whose tail to the outer bucket is zeros. A T2/T3 body
            // occupies bytes past T1, so this recovers the inner bucket.
            TEXT_INNER
                .iter()
                .copied()
                .find(|&n| n <= rest.len() && rest[n..].iter().all(|&b| b == 0))
                .ok_or(WireError::BadEnvelope)?
        } else {
            match bytes.len() {
                A1_OUTER => A1,
                A2_OUTER => A2,
                A3_OUTER => A3,
                A4_OUTER => A4,
                _ => return Err(WireError::BadEnvelope),
            }
        };
        if rest.len() < inner_len {
            return Err(WireError::BadEnvelope);
        }
        let padded_message = PaddedMessage::decode(&rest[..inner_len])?;
        Ok(Self {
            delivery_capability,
            ttl_bucket,
            idempotency_token,
            padded_message,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OuterEnvelope {
    pub destination_server_id: [u8; KEY_LEN],
    pub hpke_ciphertext: Vec<u8>,
}

impl OuterEnvelope {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + KEY_LEN + 4 + self.hpke_ciphertext.len());
        out.push(PROTOCOL_VERSION);
        out.extend_from_slice(&self.destination_server_id);
        out.extend_from_slice(&(self.hpke_ciphertext.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.hpke_ciphertext);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 1 + KEY_LEN + 4 {
            return Err(WireError::BadEnvelope);
        }
        if bytes[0] != PROTOCOL_VERSION {
            return Err(WireError::UnknownVersion(bytes[0] as u64));
        }
        let destination_server_id = copy_fixed(&bytes[1..1 + KEY_LEN])?;
        let len_off = 1 + KEY_LEN;
        let n = u32::from_be_bytes(bytes[len_off..len_off + 4].try_into().unwrap()) as usize;
        let rest = &bytes[len_off + 4..];
        if rest.len() != n {
            return Err(WireError::Length {
                expected: n,
                got: rest.len(),
            });
        }
        Ok(Self {
            destination_server_id,
            hpke_ciphertext: rest.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_pads_to_t1() {
        let m = PaddedMessage::pad(MessageType::DoubleRatchet, vec![1, 2, 3]).unwrap();
        assert_eq!(m.inner_len, T1);
        let bytes = m.encode();
        assert_eq!(bytes.len(), T1);
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[1], 0x01);
        let d = PaddedMessage::decode(&bytes).unwrap();
        assert_eq!(d.type_, MessageType::DoubleRatchet);
        assert_eq!(&d.body[..3], &[1, 2, 3]);
        assert!(d.body[3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn body_over_t1_uses_t2() {
        let m = PaddedMessage::pad(MessageType::DoubleRatchet, vec![1; 2000]).unwrap();
        assert_eq!(m.inner_len, T2);
        let inner = InnerEnvelope {
            delivery_capability: [3u8; 32],
            ttl_bucket: TtlBucket::DEFAULT,
            idempotency_token: [4u8; 32],
            padded_message: m,
        };
        let decoded = InnerEnvelope::decode_padded(&inner.encode_padded().unwrap()).unwrap();
        assert_eq!(decoded.padded_message.inner_len, T2);
        assert_eq!(&decoded.padded_message.body[..2000], &[1u8; 2000]);
    }

    #[test]
    fn inner_text_always_20480() {
        let padded = PaddedMessage::pad(MessageType::MlsApp, vec![9; 100]).unwrap();
        let inner = InnerEnvelope {
            delivery_capability: [1u8; 32],
            ttl_bucket: TtlBucket::SECONDS_60,
            idempotency_token: [2u8; 32],
            padded_message: padded,
        };
        let bytes = inner.encode_padded().unwrap();
        assert_eq!(bytes.len(), INNER_TEXT_OUTER);
        assert_eq!(bytes[0], 1);
        let decoded = InnerEnvelope::decode_padded(&bytes).unwrap();
        assert_eq!(decoded.delivery_capability, [1u8; 32]);
        assert_eq!(decoded.ttl_bucket, TtlBucket::SECONDS_60);
        assert_eq!(&decoded.padded_message.body[..100], &[9u8; 100]);
    }

    #[test]
    fn no_sender_field_layout() {
        let padded = PaddedMessage::pad(MessageType::DoubleRatchet, vec![0xaa]).unwrap();
        let inner = InnerEnvelope {
            delivery_capability: [7u8; 32],
            ttl_bucket: TtlBucket::DEFAULT,
            idempotency_token: [8u8; 32],
            padded_message: padded,
        };
        let bytes = inner.encode_padded().unwrap();
        // version, capability, ttl, idempotency, message — nothing that is a sender id
        assert_eq!(&bytes[1..33], &[7u8; 32]);
    }
}
