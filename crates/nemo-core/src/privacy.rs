//! Privacy transport (phase 6 v1). Does not change envelope bytes.
//!
//! Normal and Private ship. High is Private plus a Tor hop flag.
//! Nice-to-have (not shipped): Tor/Arti, cover traffic, Maximum / constant-rate.

use nemo_wire::envelope::{MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::ids::KEY_LEN;
use rand::RngCore;

use crate::error::{CoreError, Result};
use crate::mailbox;

pub const PRIVATE_EXTRA_MIN_MS: u64 = 20;
pub const PRIVATE_EXTRA_MAX_MS: u64 = 200;
pub const PRIVATE_BATCH_MAX_MS: u64 = 5_000;
pub const WAKE_COALESCE_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyMode {
    Normal,
    Private,
    /// Private timing + client→home via Tor when a Tor sink exists.
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hop {
    Direct,
    /// Client → home only. S2S stays mTLS.
    Tor,
}

pub trait EnvelopeSink {
    fn emit(&mut self, hop: Hop, outer_bytes: Vec<u8>) -> Result<()>;
}

/// Collects flushed envelopes. Tests and in-process loops use this.
#[derive(Default)]
pub struct MemSink {
    pub sent: Vec<(Hop, Vec<u8>)>,
}

impl EnvelopeSink for MemSink {
    fn emit(&mut self, hop: Hop, outer_bytes: Vec<u8>) -> Result<()> {
        self.sent.push((hop, outer_bytes));
        Ok(())
    }
}

struct Pending {
    bytes: Vec<u8>,
    ready_at_ms: u64,
}

pub struct PrivacyTransport<S: EnvelopeSink> {
    mode: PrivacyMode,
    hop: Hop,
    now_ms: u64,
    pending: Vec<Pending>,
    sink: S,
}

impl<S: EnvelopeSink> PrivacyTransport<S> {
    pub fn new(mode: PrivacyMode, sink: S) -> Self {
        Self {
            hop: hop_for(mode),
            mode,
            now_ms: 0,
            pending: Vec::new(),
            sink,
        }
    }

    pub fn mode(&self) -> PrivacyMode {
        self.mode
    }

    pub fn hop(&self) -> Hop {
        self.hop
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Queue an already-sealed envelope. Bytes are not rewritten.
    pub fn send(&mut self, outer: &OuterEnvelope) -> Result<()> {
        let bytes = outer.encode();
        let ready_at_ms = self.now_ms.saturating_add(self.delay_ms());
        if self.mode == PrivacyMode::Normal {
            self.sink.emit(self.hop, bytes)?;
        } else {
            self.pending.push(Pending { bytes, ready_at_ms });
        }
        Ok(())
    }

    /// Cover sending is specified for High/Maximum; not shipped.
    pub fn send_cover(&mut self, _outer: &OuterEnvelope) -> Result<()> {
        Err(CoreError::CoverNotShipped)
    }

    /// Fetched mailbox bytes pass through unchanged.
    pub fn recv(fetched_outer_bytes: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        fetched_outer_bytes
    }

    pub fn advance(&mut self, now_ms: u64) -> Result<usize> {
        self.now_ms = now_ms;
        self.flush_ready()
    }

    pub fn flush_ready(&mut self) -> Result<usize> {
        let now = self.now_ms;
        let mut rest = Vec::new();
        let mut n = 0usize;
        for p in self.pending.drain(..) {
            if p.ready_at_ms <= now {
                self.sink.emit(self.hop, p.bytes)?;
                n += 1;
            } else {
                rest.push(p);
            }
        }
        self.pending = rest;
        Ok(n)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    fn delay_ms(&self) -> u64 {
        match self.mode {
            PrivacyMode::Normal => 0,
            PrivacyMode::Private | PrivacyMode::High => extra_delay_ms() + batch_jitter_ms(),
        }
    }
}

pub fn hop_for(mode: PrivacyMode) -> Hop {
    match mode {
        PrivacyMode::Normal | PrivacyMode::Private => Hop::Direct,
        PrivacyMode::High => Hop::Tor,
    }
}

/// Maximum is specified but not shipped (ADR-0021).
pub fn reject_maximum() -> Result<()> {
    Err(CoreError::MaximumNotShipped)
}

/// Valid dummy in a text bucket. Cover *sending* is not shipped; this exists so
/// later modes can add traffic without a wire change.
pub fn dummy_outer(
    dest_hpke_public: &[u8; KEY_LEN],
    delivery_capability: [u8; KEY_LEN],
) -> Result<OuterEnvelope> {
    mailbox::wrap(
        dest_hpke_public,
        delivery_capability,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        Vec::new(),
    )
}

/// Client→home delay for Private (and High) on the live send path.
pub fn private_send_delay_ms() -> u64 {
    extra_delay_ms() + batch_jitter_ms()
}

fn extra_delay_ms() -> u64 {
    uniform(PRIVATE_EXTRA_MIN_MS, PRIVATE_EXTRA_MAX_MS)
}

fn batch_jitter_ms() -> u64 {
    uniform(0, PRIVATE_BATCH_MAX_MS)
}

fn uniform(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let span = max - min + 1;
    let mut b = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut b);
    min + (u64::from_le_bytes(b) % span)
}

/// At most one push wake per 10 s per device.
#[derive(Default)]
pub struct WakeCoalesce {
    last_wake_ms: Option<u64>,
}

impl WakeCoalesce {
    pub fn should_wake(&mut self, now_ms: u64) -> bool {
        match self.last_wake_ms {
            Some(t) if now_ms.saturating_sub(t) < WAKE_COALESCE_MS => false,
            _ => {
                self.last_wake_ms = Some(now_ms);
                true
            }
        }
    }
}
