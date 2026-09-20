//! Ephemeral TURN credentials (ADR-0035). No identity in the username; no table.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngCore;
use ring::hmac;

use nemo_wire::ids;

pub const TURN_TTL_SECS: u64 = 3600;

pub struct TurnCred {
    pub url: String,
    pub username: String,
    pub credential: String,
    pub ttl_secs: u64,
}

pub fn issue() -> TurnCred {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let expiry = now.saturating_add(TURN_TTL_SECS);
    let mut nonce = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let username = format!("{expiry}:{}", ids::to_hex(&nonce));
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret());
    let tag = hmac::sign(&key, username.as_bytes());
    let credential =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, tag.as_ref());
    TurnCred {
        url: std::env::var("NEMO_TURN_URL").unwrap_or_else(|_| "turn:127.0.0.1:3478".into()),
        username,
        credential,
        ttl_secs: TURN_TTL_SECS,
    }
}

fn secret() -> &'static [u8] {
    static SECRET: OnceLock<Vec<u8>> = OnceLock::new();
    SECRET
        .get_or_init(|| {
            if let Ok(s) = std::env::var("NEMO_TURN_SECRET") {
                if !s.is_empty() {
                    return s.into_bytes();
                }
            }
            let mut b = vec![0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut b);
            b
        })
        .as_slice()
}
