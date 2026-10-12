//! Long-term home HPKE and signing secrets live in a 0600 file, not Postgres
//! (N-16). Public bundle fields still persist in `server_identity`.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use nemo_wire::cbor::{self, Value};
use nemo_wire::hpke::HpkeKeypair;
use nemo_wire::ids::{self, KEY_LEN};
use nemo_wire::SigningKey;

#[derive(Debug)]
pub enum IdentityFileError {
    Io(io::Error),
    Corrupt(&'static str),
}

impl std::fmt::Display for IdentityFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "identity file: {e}"),
            Self::Corrupt(m) => write!(f, "identity file: {m}"),
        }
    }
}

impl std::error::Error for IdentityFileError {}

impl From<io::Error> for IdentityFileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// CBOR map `{0: hpke_secret, 1: hpke_public, 2: sign_secret, 3: sign_public}`.
pub fn save(path: &Path, hpke: &HpkeKeypair, sign: &SigningKey) -> Result<(), IdentityFileError> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    let bytes = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(hpke.secret.to_vec())),
        (1, Value::Bytes(hpke.public.to_vec())),
        (2, Value::Bytes(sign.to_bytes().to_vec())),
        (3, Value::Bytes(sign.verifying_key().to_bytes().to_vec())),
    ]));
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &Path) -> Result<(HpkeKeypair, SigningKey), IdentityFileError> {
    let bytes = fs::read(path)?;
    let Value::Map(m) = cbor::decode(&bytes).map_err(|_| IdentityFileError::Corrupt("cbor"))?
    else {
        return Err(IdentityFileError::Corrupt("map"));
    };
    let read = |k: u64| -> Result<[u8; KEY_LEN], IdentityFileError> {
        let v = cbor::map_get(&m, k).map_err(|_| IdentityFileError::Corrupt("field"))?;
        let b = cbor::expect_bytes(v).map_err(|_| IdentityFileError::Corrupt("bytes"))?;
        ids::copy_fixed(b).map_err(|_| IdentityFileError::Corrupt("len"))
    };
    let hpke = HpkeKeypair {
        secret: read(0)?,
        public: read(1)?,
    };
    let sign = SigningKey::from_bytes(&read(2)?);
    if sign.verifying_key().to_bytes() != read(3)? {
        return Err(IdentityFileError::Corrupt("sign mismatch"));
    }
    Ok((hpke, sign))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_wire::hpke::HpkeKeypair;
    use nemo_wire::SigningKey;

    #[test]
    fn roundtrip_and_mode() {
        let hpke = HpkeKeypair::generate();
        let sign = SigningKey::generate(&mut rand::rng());
        let dir = std::env::temp_dir().join(format!(
            "nemo-ident-{}",
            nemo_wire::ids::to_hex(&hpke.public[..8])
        ));
        let path = dir.join("identity.cbor");
        save(&path, &hpke, &sign).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let (loaded_h, loaded_s) = load(&path).unwrap();
        assert_eq!(loaded_h, hpke);
        assert_eq!(loaded_s.to_bytes(), sign.to_bytes());
        let _ = fs::remove_dir_all(&dir);
    }
}
