//! Device-held secret mixed into the SQLCipher key (ADR-0034).
//!
//! The secret is never stored next to `store.db` in plaintext. Copying the vault
//! directory to another machine does not unlock a version-2 vault.

use std::fs;
use std::path::{Path, PathBuf};

use nemo_wire::ids;
use rand::RngCore;

use crate::error::{CoreError, Result};
use crate::vault::KEY_LEN;

const SERVICE: &str = "org.nemo.vault";
const FILE_PREFIX: &str = "device-";

pub fn resolve(injected: Option<&[u8]>, salt: &[u8], create: bool) -> Result<[u8; KEY_LEN]> {
    if let Some(bytes) = injected {
        return parse_secret(bytes);
    }
    os_secret(salt, create)
}

fn parse_secret(bytes: &[u8]) -> Result<[u8; KEY_LEN]> {
    if bytes.len() != KEY_LEN {
        return Err(CoreError::VaultCorrupt);
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn account(salt: &[u8]) -> String {
    format!("{FILE_PREFIX}{}", ids::to_hex(salt))
}

fn bind_file(salt: &[u8]) -> Result<PathBuf> {
    Ok(bind_dir()?.join(account(salt)))
}

fn bind_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("NEMO_BIND_DIR") {
        let p = PathBuf::from(dir);
        fs::create_dir_all(&p).map_err(|e| CoreError::VaultIo(e.to_string()))?;
        return Ok(p);
    }
    if let Some(base) = dirs::data_local_dir() {
        let dir = base.join("nemo").join("binds");
        if fs::create_dir_all(&dir).is_ok() {
            return Ok(dir);
        }
    }
    let fallback = std::env::temp_dir().join("nemo-binds");
    fs::create_dir_all(&fallback).map_err(|e| CoreError::VaultIo(e.to_string()))?;
    Ok(fallback)
}

fn read_file(path: &Path) -> Result<[u8; KEY_LEN]> {
    let bytes = fs::read(path).map_err(|e| CoreError::VaultIo(e.to_string()))?;
    parse_secret(&bytes)
}

fn write_file(path: &Path, secret: &[u8; KEY_LEN]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| CoreError::VaultIo(e.to_string()))?;
    }
    fs::write(path, secret).map_err(|e| CoreError::VaultIo(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn os_secret(salt: &[u8], create: bool) -> Result<[u8; KEY_LEN]> {
    let path = bind_file(salt)?;
    if path.is_file() {
        return read_file(&path);
    }
    if let Ok(secret) = keyring_load(salt) {
        return Ok(secret);
    }
    if !create {
        return Err(CoreError::VaultLocked);
    }
    let mut secret = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    let _ = keyring_store(salt, &secret);
    write_file(&path, &secret)?;
    Ok(secret)
}

#[cfg(target_os = "android")]
fn os_secret(_salt: &[u8], _create: bool) -> Result<[u8; KEY_LEN]> {
    Err(CoreError::VaultIo(
        "Android vault requires a Keystore device secret".into(),
    ))
}

#[cfg(not(target_os = "android"))]
fn keyring_load(salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    let entry = keyring::Entry::new(SERVICE, &account(salt))
        .map_err(|e| CoreError::VaultIo(e.to_string()))?;
    parse_secret(
        &entry
            .get_secret()
            .map_err(|e| CoreError::VaultIo(e.to_string()))?,
    )
}

#[cfg(not(target_os = "android"))]
fn keyring_store(salt: &[u8], secret: &[u8; KEY_LEN]) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, &account(salt))
        .map_err(|e| CoreError::VaultIo(e.to_string()))?;
    entry
        .set_secret(secret)
        .map_err(|e| CoreError::VaultIo(e.to_string()))
}
