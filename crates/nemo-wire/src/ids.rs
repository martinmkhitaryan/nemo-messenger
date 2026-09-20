use sha2::{Digest, Sha256};

use crate::error::{Result, WireError};

pub const ID_LEN: usize = 32;
pub const KEY_LEN: usize = 32;
pub const SIG_LEN: usize = 64;

pub type IdentityId = [u8; ID_LEN];
pub type ServerId = [u8; ID_LEN];

pub fn sha256(bytes: &[u8]) -> [u8; ID_LEN] {
    Sha256::digest(bytes).into()
}

/// `identity_id = SHA-256(identity_public_key)` (32-byte raw Ed25519 key).
pub fn identity_id(identity_public_key: &[u8; KEY_LEN]) -> IdentityId {
    sha256(identity_public_key)
}

/// `server_id = SHA-256(server_hpke_public_key)` (32-byte raw X25519 key).
pub fn server_id(server_hpke_public_key: &[u8; KEY_LEN]) -> ServerId {
    sha256(server_hpke_public_key)
}

/// Fingerprint: 64 lowercase hex chars in eight groups of eight, spaces between groups.
pub fn fingerprint(id: &IdentityId) -> String {
    let hex = hex_encode(id);
    hex.as_bytes()
        .chunks(8)
        .map(|c| std::str::from_utf8(c).expect("hex"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn to_hex(bytes: &[u8]) -> String {
    hex_encode(bytes)
}

pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(WireError::Length {
            expected: s.len() / 2,
            got: s.len(),
        });
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let hi = hex_nibble(b[i])?;
        let lo = hex_nibble(b[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

pub fn parse_identity_id(s: &str) -> Result<IdentityId> {
    let bytes = from_hex(s)?;
    copy_fixed(&bytes)
}

fn hex_nibble(b: u8) -> Result<u8> {
    Ok(match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => {
            return Err(WireError::Length {
                expected: 1,
                got: 0,
            })
        }
    })
}

pub fn copy_fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    let arr: [u8; N] = bytes.try_into().map_err(|_| WireError::Length {
        expected: N,
        got: bytes.len(),
    })?;
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_32_bytes() {
        let pk = [7u8; 32];
        assert_eq!(identity_id(&pk).len(), 32);
        // Same hash; domain separation is which key you hash, not a second H.
        assert_eq!(identity_id(&pk), server_id(&pk));
        let other = [8u8; 32];
        assert_ne!(identity_id(&pk), identity_id(&other));
    }

    #[test]
    fn fingerprint_groups() {
        let id = [0u8; 32];
        let fp = fingerprint(&id);
        assert_eq!(fp.matches(' ').count(), 7);
        assert_eq!(fp.replace(' ', "").len(), 64);
        assert!(fp.chars().all(|c| c == ' ' || c.is_ascii_hexdigit()));
        assert_eq!(fp, fp.to_ascii_lowercase());
    }
}
