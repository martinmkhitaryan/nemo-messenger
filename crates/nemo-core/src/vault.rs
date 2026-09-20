//! Passphrase-locked SQLCipher vault (ADR-0034).
//!
//! The revocation mnemonic is never written here.

use std::fs;
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use nemo_wire::cbor::{self, Value};
use nemo_wire::ids;
use rand::RngCore;
use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::group::Group;
use crate::identity::Installation;

pub const KDF_VERSION: u64 = 1;
pub const M_COST_KIB: u32 = 64 * 1024;
pub const T_COST: u32 = 3;
pub const P_COST: u32 = 1;
pub const SALT_LEN: usize = 16;
pub const KEY_LEN: usize = 32;
pub const MIN_PASSPHRASE_CHARS: usize = 8;

const KDF_FILE: &str = "kdf.cbor";
const STORE_FILE: &str = "store.db";
const SNAPSHOT_KEY: &str = "installation";
const MLS_KEY: &str = "mls_storage";
const GROUPS_KEY: &str = "groups";

pub struct Vault {
    dir: PathBuf,
    conn: Connection,
}

impl Vault {
    pub fn create(dir: impl AsRef<Path>, passphrase: &str, install: &Installation) -> Result<Self> {
        check_passphrase(passphrase)?;
        let dir = dir.as_ref();
        fs::create_dir_all(dir).map_err(vault_io)?;
        let kdf_path = dir.join(KDF_FILE);
        let db_path = dir.join(STORE_FILE);
        if kdf_path.exists() || db_path.exists() {
            return Err(CoreError::VaultExists);
        }

        let mut salt = [0u8; SALT_LEN];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        write_kdf(&kdf_path, &salt, M_COST_KIB, T_COST, P_COST)?;
        let mut key = derive(passphrase, &salt, M_COST_KIB, T_COST, P_COST)?;
        let conn = open_cipher(&db_path, &key)?;
        zero(&mut key);
        conn.execute_batch(
            "CREATE TABLE kv (k TEXT PRIMARY KEY NOT NULL, v BLOB NOT NULL);",
        )
        .map_err(map_sql)?;
        let vault = Self {
            dir: dir.to_path_buf(),
            conn,
        };
        vault.save(install)?;
        Ok(vault)
    }

    pub fn open(dir: impl AsRef<Path>, passphrase: &str) -> Result<(Self, Installation)> {
        check_passphrase(passphrase)?;
        let dir = dir.as_ref();
        let kdf_path = dir.join(KDF_FILE);
        let db_path = dir.join(STORE_FILE);
        if !kdf_path.is_file() || !db_path.is_file() {
            return Err(CoreError::VaultMissing);
        }
        let kdf = read_kdf(&kdf_path)?;
        let mut key = derive(passphrase, &kdf.salt, kdf.m_cost, kdf.t_cost, kdf.p_cost)?;
        let conn = open_cipher(&db_path, &key)?;
        zero(&mut key);
        let vault = Self {
            dir: dir.to_path_buf(),
            conn,
        };
        let install = vault.load()?;
        if let Some(mls) = vault.get_opt(MLS_KEY)? {
            install.apply_mls_storage(&mls)?;
        }
        Ok((vault, install))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn save(&self, install: &Installation) -> Result<()> {
        self.put(SNAPSHOT_KEY, &install.encode_snapshot()?)?;
        self.put(MLS_KEY, &install.encode_mls_storage()?)?;
        Ok(())
    }

    pub fn save_groups(&self, install: &Installation, groups: &[Group]) -> Result<()> {
        self.save(install)?;
        let mut items = Vec::with_capacity(groups.len());
        for g in groups {
            items.push(Value::Bytes(g.encode_sidecar()?));
        }
        self.put(GROUPS_KEY, &cbor::encode(&Value::Array(items)))?;
        Ok(())
    }

    pub fn load_groups(&self, install: &Installation) -> Result<Vec<Group>> {
        let Some(bytes) = self.get_opt(GROUPS_KEY)? else {
            return Ok(Vec::new());
        };
        let Value::Array(items) = cbor::decode(&bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let mut groups = Vec::with_capacity(items.len());
        for item in items {
            let sidecar = cbor::expect_bytes(&item).map_err(|_| CoreError::VaultCorrupt)?;
            groups.push(Group::load_sidecar(install.mls_provider(), sidecar)?);
        }
        Ok(groups)
    }

    fn put(&self, k: &str, v: &[u8]) -> Result<()> {
        let n = self
            .conn
            .execute(
                "INSERT INTO kv(k, v) VALUES(?1, ?2)
                 ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                params![k, v],
            )
            .map_err(map_sql)?;
        if n == 0 {
            return Err(CoreError::VaultIo("snapshot not written".into()));
        }
        Ok(())
    }

    fn get_opt(&self, k: &str) -> Result<Option<Vec<u8>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT v FROM kv WHERE k = ?1")
            .map_err(map_sql)?;
        let mut rows = stmt.query(params![k]).map_err(map_sql)?;
        match rows.next().map_err(map_sql)? {
            Some(row) => Ok(Some(row.get(0).map_err(map_sql)?)),
            None => Ok(None),
        }
    }

    fn load(&self) -> Result<Installation> {
        let bytes: Vec<u8> = self
            .conn
            .query_row(
                "SELECT v FROM kv WHERE k = ?1",
                params![SNAPSHOT_KEY],
                |row| row.get(0),
            )
            .map_err(map_sql)?;
        Installation::decode_snapshot(&bytes)
    }
}

struct Kdf {
    salt: Vec<u8>,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

fn check_passphrase(passphrase: &str) -> Result<()> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(CoreError::WeakPassphrase);
    }
    Ok(())
}

fn write_kdf(path: &Path, salt: &[u8], m_cost: u32, t_cost: u32, p_cost: u32) -> Result<()> {
    let bytes = cbor::encode(&Value::Map(vec![
        (0, Value::Uint(KDF_VERSION)),
        (1, Value::Bytes(salt.to_vec())),
        (2, Value::Uint(m_cost as u64)),
        (3, Value::Uint(t_cost as u64)),
        (4, Value::Uint(p_cost as u64)),
    ]));
    fs::write(path, bytes).map_err(vault_io)
}

fn read_kdf(path: &Path) -> Result<Kdf> {
    let bytes = fs::read(path).map_err(vault_io)?;
    let Value::Map(m) = cbor::decode(&bytes).map_err(|_| CoreError::VaultCorrupt)? else {
        return Err(CoreError::VaultCorrupt);
    };
    let version = cbor::expect_uint(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
        .map_err(|_| CoreError::VaultCorrupt)?;
    if version != KDF_VERSION {
        return Err(CoreError::VaultCorrupt);
    }
    let salt = cbor::expect_bytes(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
        .map_err(|_| CoreError::VaultCorrupt)?
        .to_vec();
    if salt.len() < 8 {
        return Err(CoreError::VaultCorrupt);
    }
    let m_cost = cbor::expect_uint(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
        .map_err(|_| CoreError::VaultCorrupt)? as u32;
    let t_cost = cbor::expect_uint(cbor::map_get(&m, 3).map_err(|_| CoreError::VaultCorrupt)?)
        .map_err(|_| CoreError::VaultCorrupt)? as u32;
    let p_cost = cbor::expect_uint(cbor::map_get(&m, 4).map_err(|_| CoreError::VaultCorrupt)?)
        .map_err(|_| CoreError::VaultCorrupt)? as u32;
    Ok(Kdf {
        salt,
        m_cost,
        t_cost,
        p_cost,
    })
}

fn derive(passphrase: &str, salt: &[u8], m_cost: u32, t_cost: u32, p_cost: u32) -> Result<[u8; KEY_LEN]> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(KEY_LEN)).map_err(|_| CoreError::VaultCorrupt)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|_| CoreError::VaultCorrupt)?;
    Ok(key)
}

fn open_cipher(path: &Path, key: &[u8; KEY_LEN]) -> Result<Connection> {
    let conn = Connection::open(path).map_err(map_sql)?;
    let pragma = format!("x'{}'", ids::to_hex(key));
    conn.pragma_update(None, "key", &pragma).map_err(map_sql)?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(map_sql)?;
    Ok(conn)
}

fn map_sql(err: rusqlite::Error) -> CoreError {
    let s = err.to_string();
    if s.contains("not a database") || s.contains("file is encrypted") || s.contains("file is not a database")
    {
        CoreError::VaultLocked
    } else {
        CoreError::VaultIo(s)
    }
}

fn vault_io(err: std::io::Error) -> CoreError {
    CoreError::VaultIo(err.to_string())
}

fn zero(key: &mut [u8]) {
    for b in key {
        *b = 0;
    }
}
