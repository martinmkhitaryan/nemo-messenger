//! Group file ciphertext: HPKE to an ephemeral key carried in MLS plaintext.
//!
//! The host stores one padded A* blob. File keys never leave E2EE (ADR-0019).

use nemo_wire::hostframe::AttachmentSizeBucket;
use nemo_wire::hpke::{self, HpkeKeypair};
use nemo_wire::ids::{self, copy_fixed, KEY_LEN};
use nemo_wire::WireError;

use crate::error::{CoreError, Result};

const KEY_MATERIAL: usize = KEY_LEN * 2;

/// Returns `(mls enc_file key material, bucket, host blob)`.
pub fn seal_group_file(plaintext: &[u8]) -> Result<(Vec<u8>, AttachmentSizeBucket, Vec<u8>)> {
    let kp = HpkeKeypair::generate();
    let dest = kp.server_id();
    let sealed = hpke::seal(&kp.public, &dest, plaintext)?;
    let need = 4usize.saturating_add(sealed.len());
    let bucket = AttachmentSizeBucket::for_len(need)?;
    let mut blob = vec![0u8; bucket.inner_len()];
    let n = u32::try_from(sealed.len()).map_err(|_| CoreError::Wire(WireError::NoBucket))?;
    blob[..4].copy_from_slice(&n.to_be_bytes());
    blob[4..4 + sealed.len()].copy_from_slice(&sealed);
    let mut key = Vec::with_capacity(KEY_MATERIAL);
    key.extend_from_slice(&kp.secret);
    key.extend_from_slice(&kp.public);
    Ok((key, bucket, blob))
}

pub fn open_group_file(key: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    if key.len() != KEY_MATERIAL || blob.len() < 4 {
        return Err(CoreError::BadCiphertext);
    }
    let n = u32::from_be_bytes(blob[..4].try_into().unwrap()) as usize;
    if 4usize.saturating_add(n) > blob.len() {
        return Err(CoreError::BadCiphertext);
    }
    let secret = copy_fixed(&key[..KEY_LEN])?;
    let public = copy_fixed(&key[KEY_LEN..])?;
    let dest = ids::server_id(&public);
    Ok(hpke::open(&secret, &dest, &blob[4..4 + n])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_file_roundtrip_fits_a1() {
        let (key, bucket, blob) = seal_group_file(b"hello file").unwrap();
        assert_eq!(bucket, AttachmentSizeBucket::A1);
        assert_eq!(blob.len(), bucket.inner_len());
        assert_eq!(open_group_file(&key, &blob).unwrap(), b"hello file");
    }
}
