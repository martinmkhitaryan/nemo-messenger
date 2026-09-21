//! Passphrase-locked SQLCipher vault (ADR-0034).
//!
//! The revocation mnemonic is never written here.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use nemo_wire::cbor::{self, Value};
use nemo_wire::ids;
use rand::RngCore;
use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::group::{Group, PendingJoin};
use crate::home::{HomeState, HostAccept};
use crate::identity::Installation;

pub const KDF_VERSION: u64 = 2;
pub const KDF_VERSION_PASSPHRASE_ONLY: u64 = 1;
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
const HOME_KEY: &str = "home";
const DISPLAY_KEY: &str = "display";
const PENDING_KEY: &str = "pending_joins";
const INVITES_KEY: &str = "minted_invites";

pub struct Vault {
    dir: PathBuf,
    conn: Connection,
}

impl Vault {
    pub fn create(dir: impl AsRef<Path>, passphrase: &str, install: &Installation) -> Result<Self> {
        Self::create_bound(dir, passphrase, install, None)
    }

    pub fn create_bound(
        dir: impl AsRef<Path>,
        passphrase: &str,
        install: &Installation,
        device_secret: Option<&[u8]>,
    ) -> Result<Self> {
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
        write_kdf(&kdf_path, KDF_VERSION, &salt, M_COST_KIB, T_COST, P_COST)?;
        let mut key = derive(passphrase, &salt, M_COST_KIB, T_COST, P_COST)?;
        let mut device = crate::device_bind::resolve(device_secret, &salt, true)?;
        mix_device_key(&mut key, &device);
        zero(&mut device);
        let conn = open_cipher(&db_path, &key)?;
        zero(&mut key);
        conn.execute_batch("CREATE TABLE kv (k TEXT PRIMARY KEY NOT NULL, v BLOB NOT NULL);")
            .map_err(map_sql)?;
        let vault = Self {
            dir: dir.to_path_buf(),
            conn,
        };
        vault.save(install)?;
        Ok(vault)
    }

    pub fn open(dir: impl AsRef<Path>, passphrase: &str) -> Result<(Self, Installation)> {
        Self::open_bound(dir, passphrase, None)
    }

    pub fn open_bound(
        dir: impl AsRef<Path>,
        passphrase: &str,
        device_secret: Option<&[u8]>,
    ) -> Result<(Self, Installation)> {
        check_passphrase(passphrase)?;
        let dir = dir.as_ref();
        let kdf_path = dir.join(KDF_FILE);
        let db_path = dir.join(STORE_FILE);
        if !kdf_path.is_file() || !db_path.is_file() {
            return Err(CoreError::VaultMissing);
        }
        let kdf = read_kdf(&kdf_path)?;
        let mut key = derive(passphrase, &kdf.salt, kdf.m_cost, kdf.t_cost, kdf.p_cost)?;
        if kdf.version >= KDF_VERSION {
            let mut device = crate::device_bind::resolve(device_secret, &kdf.salt, false)?;
            mix_device_key(&mut key, &device);
            zero(&mut device);
        }
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

    pub fn save_groups<'a>(
        &self,
        install: &Installation,
        groups: impl IntoIterator<Item = &'a Group>,
    ) -> Result<()> {
        self.save(install)?;
        let mut items = Vec::new();
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

    pub fn save_home(&self, install: &Installation, state: &HomeState) -> Result<()> {
        self.save(install)?;
        self.put(HOME_KEY, &state.encode()?)?;
        Ok(())
    }

    pub fn load_home(&self) -> Result<Option<HomeState>> {
        let Some(bytes) = self.get_opt(HOME_KEY)? else {
            return Ok(None);
        };
        Ok(Some(HomeState::decode(&bytes)?))
    }

    /// Nicknames and disappear timers only. Never ratchet or MLS keys.
    pub fn save_display(
        &self,
        nicknames: &HashMap<String, String>,
        disappear: &HashMap<String, u64>,
    ) -> Result<()> {
        let mut nicks = Vec::new();
        let mut keys: Vec<_> = nicknames.keys().cloned().collect();
        keys.sort();
        for k in keys {
            nicks.push(Value::Array(vec![
                Value::Text(k.clone()),
                Value::Text(nicknames[&k].clone()),
            ]));
        }
        let mut timers = Vec::new();
        let mut tkeys: Vec<_> = disappear.keys().cloned().collect();
        tkeys.sort();
        for k in tkeys {
            timers.push(Value::Array(vec![
                Value::Text(k.clone()),
                Value::Uint(disappear[&k]),
            ]));
        }
        self.put(
            DISPLAY_KEY,
            &cbor::encode(&Value::Map(vec![
                (0, Value::Uint(1)),
                (1, Value::Array(nicks)),
                (2, Value::Array(timers)),
            ])),
        )
    }

    pub fn load_display(&self) -> Result<(HashMap<String, String>, HashMap<String, u64>)> {
        let Some(bytes) = self.get_opt(DISPLAY_KEY)? else {
            return Ok((HashMap::new(), HashMap::new()));
        };
        let Value::Map(m) = cbor::decode(&bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if version != 1 {
            return Err(CoreError::VaultCorrupt);
        }
        let mut nicknames = HashMap::new();
        for item in cbor::expect_array(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?
        {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 2 {
                return Err(CoreError::VaultCorrupt);
            }
            let k = cbor::expect_text(&row[0])
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_owned();
            let v = cbor::expect_text(&row[1])
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_owned();
            nicknames.insert(k, v);
        }
        let mut disappear = HashMap::new();
        for item in cbor::expect_array(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?
        {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 2 {
                return Err(CoreError::VaultCorrupt);
            }
            let k = cbor::expect_text(&row[0])
                .map_err(|_| CoreError::VaultCorrupt)?
                .to_owned();
            let secs = cbor::expect_uint(&row[1]).map_err(|_| CoreError::VaultCorrupt)?;
            disappear.insert(k, secs);
        }
        Ok((nicknames, disappear))
    }

    pub fn save_pending(
        &self,
        entries: &[([u8; nemo_wire::ids::KEY_LEN], &PendingJoin, &HostAccept)],
    ) -> Result<()> {
        let mut items = Vec::new();
        for (gid, pending, host) in entries {
            items.push(Value::Array(vec![
                Value::Bytes(gid.to_vec()),
                Value::Bytes(pending.encode()?),
                Value::Bytes(host.encode()),
            ]));
        }
        self.put(PENDING_KEY, &cbor::encode(&Value::Array(items)))
    }

    pub fn load_pending(
        &self,
        install: &Installation,
    ) -> Result<Vec<([u8; nemo_wire::ids::KEY_LEN], PendingJoin, HostAccept)>> {
        let Some(bytes) = self.get_opt(PENDING_KEY)? else {
            return Ok(Vec::new());
        };
        let Value::Array(items) = cbor::decode(&bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let mut out = Vec::new();
        for item in items {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 3 {
                return Err(CoreError::VaultCorrupt);
            }
            let gid =
                ids::copy_fixed(cbor::expect_bytes(&row[0]).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?;
            let pending = PendingJoin::decode(
                install.mls_provider(),
                cbor::expect_bytes(&row[1]).map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            let host = HostAccept::decode(
                cbor::expect_bytes(&row[2]).map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            out.push((gid, pending, host));
        }
        Ok(out)
    }

    pub fn save_invites(&self, invites: &[nemo_wire::GroupInvite]) -> Result<()> {
        let items: Vec<_> = invites.iter().map(|i| Value::Bytes(i.encode())).collect();
        self.put(INVITES_KEY, &cbor::encode(&Value::Array(items)))
    }

    pub fn load_invites(&self) -> Result<Vec<nemo_wire::GroupInvite>> {
        let Some(bytes) = self.get_opt(INVITES_KEY)? else {
            return Ok(Vec::new());
        };
        let Value::Array(items) = cbor::decode(&bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let mut out = Vec::new();
        for item in items {
            let raw = cbor::expect_bytes(&item).map_err(|_| CoreError::VaultCorrupt)?;
            out.push(nemo_wire::GroupInvite::decode(raw)?);
        }
        Ok(out)
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
    version: u64,
    salt: Vec<u8>,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

fn mix_device_key(pass_key: &mut [u8; KEY_LEN], device: &[u8; KEY_LEN]) {
    for (p, d) in pass_key.iter_mut().zip(device.iter()) {
        *p ^= *d;
    }
}

fn check_passphrase(passphrase: &str) -> Result<()> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(CoreError::WeakPassphrase);
    }
    Ok(())
}

fn write_kdf(
    path: &Path,
    version: u64,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<()> {
    let bytes = cbor::encode(&Value::Map(vec![
        (0, Value::Uint(version)),
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
    if version != KDF_VERSION && version != KDF_VERSION_PASSPHRASE_ONLY {
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
        version,
        salt,
        m_cost,
        t_cost,
        p_cost,
    })
}

fn derive(
    passphrase: &str,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<[u8; KEY_LEN]> {
    let params =
        Params::new(m_cost, t_cost, p_cost, Some(KEY_LEN)).map_err(|_| CoreError::VaultCorrupt)?;
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
    if s.contains("not a database")
        || s.contains("file is encrypted")
        || s.contains("file is not a database")
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
