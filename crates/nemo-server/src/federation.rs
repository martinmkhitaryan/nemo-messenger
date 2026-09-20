//! In-process S2S hop (phase 5). Client HTTP is `http.rs`; mTLS sockets remain a follow-on.

use std::collections::HashMap;

use nemo_wire::envelope::OuterEnvelope;
use nemo_wire::hpke::ENCAP_LEN;
use nemo_wire::ids::{ServerId, KEY_LEN};
use nemo_wire::{
    fail_payload, hello_payload, ok_payload, parse_payload, S2sFrame, S2sPayload, ServerBundle,
    ServerSignRotate,
};

use crate::error::{Result, ServerError};
use crate::home::{HomeServer, HPKE_ENC_WINDOW};

pub const PEER_RATE_PER_SEC: u32 = 100;
pub const OUTBOUND_MAX_AGE_SECS: u64 = 14 * 24 * 3600;
pub const BACKOFF_CAP_SECS: u64 = 300;

#[derive(Clone, Debug)]
pub struct PeerState {
    pub bundle: ServerBundle,
    pub refused: bool,
    pub last_rx: u64,
    pub next_tx: u64,
    pub hello_done: bool,
    pub window_sec: u64,
    pub window_ok: u32,
    seen_enc: HashMap<[u8; KEY_LEN], u64>,
}

impl PeerState {
    fn new(bundle: ServerBundle) -> Self {
        Self {
            bundle,
            refused: false,
            last_rx: 0,
            next_tx: 1,
            hello_done: false,
            window_sec: 0,
            window_ok: 0,
            seen_enc: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OutboundRow {
    pub dest: ServerId,
    pub outer: OuterEnvelope,
    pub enqueued_at: u64,
    pub next_attempt: u64,
    pub backoff_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enqueue {
    Local(u64),
    Queued,
}

#[derive(Clone, Debug, Default)]
pub struct PumpStats {
    pub delivered: usize,
    pub failed: usize,
    pub deferred: usize,
    pub expired: usize,
}

pub fn pin(home: &mut HomeServer, bundle: ServerBundle) -> Result<()> {
    bundle.verify()?;
    if home.server_id() == bundle.server_id {
        return Ok(());
    }
    home.peers
        .entry(bundle.server_id)
        .or_insert_with(|| PeerState::new(bundle));
    Ok(())
}

pub fn pin_each_other(a: &mut HomeServer, b: &mut HomeServer) -> Result<()> {
    pin(a, b.bundle().clone())?;
    pin(b, a.bundle().clone())?;
    Ok(())
}

pub fn refuse(home: &mut HomeServer, peer: ServerId) {
    if let Some(p) = home.peers.get_mut(&peer) {
        p.refused = true;
    }
}

pub fn apply_sign_rotate(
    home: &mut HomeServer,
    peer: ServerId,
    rotate: &ServerSignRotate,
) -> Result<()> {
    let p = home.peers.get_mut(&peer).ok_or(ServerError::NotPinned)?;
    let current = nemo_wire::VerifyingKey::from_bytes(&p.bundle.server_sign_public_key)
        .map_err(|_| ServerError::NotPinned)?;
    rotate.verify(&current)?;
    p.bundle.server_sign_public_key = rotate.new_public_key;
    Ok(())
}

/// Send due outbound rows from `from` to `to`. No sockets.
pub fn pump(from: &mut HomeServer, to: &mut HomeServer) -> Result<PumpStats> {
    let dest = to.server_id();
    if from.peer_refused(dest) || to.peer_refused(from.server_id()) {
        return Err(ServerError::PeerRefused);
    }
    if !from.peers.contains_key(&dest) || !to.peers.contains_key(&from.server_id()) {
        return Err(ServerError::NotPinned);
    }
    ensure_hello(from, to)?;

    let now = from.now;
    let mut stats = PumpStats::default();
    let mut i = 0;
    while i < from.outbound.len() {
        if from.outbound[i].dest != dest {
            i += 1;
            continue;
        }
        if now.saturating_sub(from.outbound[i].enqueued_at) > OUTBOUND_MAX_AGE_SECS {
            from.outbound.remove(i);
            stats.expired += 1;
            continue;
        }
        if from.outbound[i].next_attempt > now {
            stats.deferred += 1;
            i += 1;
            continue;
        }
        let row = from.outbound[i].clone();
        match forward_one(from, to, &row) {
            Ok(()) => {
                from.outbound.remove(i);
                stats.delivered += 1;
            }
            Err(ServerError::Denied) | Err(ServerError::PeerRefused) => {
                from.outbound.remove(i);
                stats.failed += 1;
            }
            Err(ServerError::CounterGap) => {
                reset_link(from, dest);
                reset_link(to, from.server_id());
                let back = from.outbound[i].backoff_secs;
                from.outbound[i].backoff_secs = (back.saturating_mul(2)).min(BACKOFF_CAP_SECS);
                from.outbound[i].next_attempt = now.saturating_add(from.outbound[i].backoff_secs);
                stats.failed += 1;
                i += 1;
            }
            Err(_) => {
                let back = from.outbound[i].backoff_secs;
                from.outbound[i].backoff_secs = (back.saturating_mul(2)).min(BACKOFF_CAP_SECS);
                from.outbound[i].next_attempt = now.saturating_add(from.outbound[i].backoff_secs);
                stats.failed += 1;
                i += 1;
            }
        }
    }
    Ok(stats)
}

fn reset_link(home: &mut HomeServer, peer: ServerId) {
    if let Some(p) = home.peers.get_mut(&peer) {
        p.last_rx = 0;
        p.next_tx = 1;
        p.hello_done = false;
    }
}

fn ensure_hello(from: &mut HomeServer, to: &mut HomeServer) -> Result<()> {
    let a_id = from.server_id();
    let b_id = to.server_id();
    let a_needs = !from.peers.get(&b_id).map(|p| p.hello_done).unwrap_or(false);
    let b_needs = !to.peers.get(&a_id).map(|p| p.hello_done).unwrap_or(false);
    if !a_needs && !b_needs {
        return Ok(());
    }
    let a_hello = take_frame(from, b_id, hello_payload(&a_id, &b_id))?;
    let b_hello = take_frame(to, a_id, hello_payload(&b_id, &a_id))?;
    accept_frame(to, a_id, &a_hello)?;
    accept_frame(from, b_id, &b_hello)?;
    if let Some(p) = from.peers.get_mut(&b_id) {
        p.hello_done = true;
    }
    if let Some(p) = to.peers.get_mut(&a_id) {
        p.hello_done = true;
    }
    Ok(())
}

fn forward_one(from: &mut HomeServer, to: &mut HomeServer, row: &OutboundRow) -> Result<()> {
    let dest = to.server_id();
    let frame = take_frame(from, dest, row.outer.hpke_ciphertext.clone())?;
    let reply = accept_frame(to, from.server_id(), &frame)?;
    let reply_frame = take_frame(to, from.server_id(), reply)?;
    match accept_frame(from, dest, &reply_frame)? {
        payload if payload == fail_payload() || matches_fail(&payload) => Err(ServerError::Denied),
        _ => Ok(()),
    }
}

fn matches_fail(bytes: &[u8]) -> bool {
    matches!(parse_payload(bytes), Ok(S2sPayload::Fail))
}

fn take_frame(home: &mut HomeServer, peer: ServerId, payload: Vec<u8>) -> Result<S2sFrame> {
    let p = home.peers.get_mut(&peer).ok_or(ServerError::NotPinned)?;
    if p.refused {
        return Err(ServerError::PeerRefused);
    }
    let counter = p.next_tx;
    p.next_tx += 1;
    Ok(S2sFrame { counter, payload })
}

fn accept_frame(home: &mut HomeServer, from: ServerId, frame: &S2sFrame) -> Result<Vec<u8>> {
    let incoming = frame.counter;
    {
        let p = home.peers.get_mut(&from).ok_or(ServerError::NotPinned)?;
        if p.refused {
            return Err(ServerError::PeerRefused);
        }
        if incoming <= p.last_rx {
            return Ok(ok_payload(0));
        }
        if incoming != p.last_rx + 1 {
            return Err(ServerError::CounterGap);
        }
        p.last_rx = incoming;
    }
    match parse_payload(&frame.payload)? {
        S2sPayload::Hello { sender, receiver } => {
            if sender != from || receiver != home.server_id() {
                return Err(ServerError::HelloMismatch);
            }
            Ok(Vec::new())
        }
        S2sPayload::Ok { .. } | S2sPayload::Fail => Ok(frame.payload.clone()),
        S2sPayload::Ciphertext(ct) => handle_ciphertext(home, from, &ct),
    }
}

fn handle_ciphertext(home: &mut HomeServer, from: ServerId, ct: &[u8]) -> Result<Vec<u8>> {
    if ct.len() < ENCAP_LEN {
        return Ok(fail_payload());
    }
    let mut enc = [0u8; KEY_LEN];
    enc.copy_from_slice(&ct[..ENCAP_LEN]);
    let now = home.now;
    {
        let p = home.peers.get_mut(&from).ok_or(ServerError::NotPinned)?;
        if p.window_sec != now {
            p.window_sec = now;
            p.window_ok = 0;
        }
        if p.window_ok >= PEER_RATE_PER_SEC {
            return Ok(fail_payload());
        }
        if let Some(&seq) = p.seen_enc.get(&enc) {
            return Ok(ok_payload(seq));
        }
    }
    let outer = OuterEnvelope {
        destination_server_id: home.server_id(),
        hpke_ciphertext: ct.to_vec(),
    };
    match home.ingest(&outer) {
        Ok(seq) => {
            let p = home.peers.get_mut(&from).ok_or(ServerError::NotPinned)?;
            p.window_ok += 1;
            if p.seen_enc.len() >= HPKE_ENC_WINDOW {
                p.seen_enc.clear();
            }
            p.seen_enc.insert(enc, seq);
            Ok(ok_payload(seq))
        }
        Err(_) => Ok(fail_payload()),
    }
}
