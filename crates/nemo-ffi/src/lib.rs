//! UniFFI surface for the Compose shell (ADR-0028). No keys cross this boundary
//! except identifiers, fingerprints, and the one-time revocation mnemonic.

uniffi::setup_scaffolding!("nemo");

use std::sync::{Arc, Mutex};

use nemo_core::Installation;
use nemo_wire::ids;

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FfiError {
    #[error("{0}")]
    Core(String),
}

impl From<nemo_core::CoreError> for FfiError {
    fn from(err: nemo_core::CoreError) -> Self {
        Self::Core(err.to_string())
    }
}

struct Inner {
    install: Installation,
    revocation_mnemonic: Option<String>,
}

/// One installation. The shell must not persist ratchet or MLS keys.
#[derive(uniffi::Object)]
pub struct NemoClient {
    inner: Mutex<Inner>,
}

#[uniffi::export]
impl NemoClient {
    #[uniffi::constructor]
    pub fn create() -> Result<Arc<Self>, FfiError> {
        let (install, export) = Installation::create()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner {
                install,
                revocation_mnemonic: Some(export.mnemonic),
            }),
        }))
    }

    pub fn identity_id_hex(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| FfiError::Core("lock".into()))?;
        Ok(ids::to_hex(&inner.install.identity_id()))
    }

    pub fn fingerprint(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| FfiError::Core("lock".into()))?;
        Ok(inner.install.fingerprint())
    }

    /// Shown once at identity creation. Never stored in the vault.
    pub fn take_revocation_mnemonic(&self) -> Result<Option<String>, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| FfiError::Core("lock".into()))?;
        Ok(inner.revocation_mnemonic.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_exports_ids_and_one_time_mnemonic() {
        let client = NemoClient::create().unwrap();
        let id = client.identity_id_hex().unwrap();
        assert_eq!(id.len(), 64);
        assert_eq!(client.fingerprint().unwrap().len(), 64 + 7);
        let mnemonic = client.take_revocation_mnemonic().unwrap();
        assert!(mnemonic.is_some());
        assert!(client.take_revocation_mnemonic().unwrap().is_none());
    }
}
