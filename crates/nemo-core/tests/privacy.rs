use nemo_core::mailbox;
use nemo_core::privacy::{
    dummy_outer, hop_for, reject_maximum, EnvelopeSink, Hop, MemSink, PrivacyMode,
    PrivacyTransport, WakeCoalesce, PRIVATE_BATCH_MAX_MS, PRIVATE_EXTRA_MAX_MS,
    PRIVATE_EXTRA_MIN_MS, WAKE_COALESCE_MS,
};
use nemo_core::CoreError;
use nemo_wire::envelope::{MessageType, TtlBucket, INNER_TEXT_OUTER};
use nemo_wire::hpke::HpkeKeypair;

fn sealed() -> (HpkeKeypair, nemo_wire::envelope::OuterEnvelope) {
    let server = HpkeKeypair::generate();
    let outer = mailbox::wrap(
        &server.public,
        mailbox::random_token(),
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1, 2, 3],
    )
    .unwrap();
    (server, outer)
}

#[test]
fn normal_emits_immediately_unchanged() {
    let (_server, outer) = sealed();
    let want = outer.encode();
    let mut t = PrivacyTransport::new(PrivacyMode::Normal, MemSink::default());
    assert_eq!(t.hop(), Hop::Direct);
    t.send(&outer).unwrap();
    assert_eq!(t.pending_count(), 0);
    assert_eq!(t.sink().sent, vec![(Hop::Direct, want)]);
}

#[test]
fn private_holds_then_flushes_unchanged() {
    let (_server, outer) = sealed();
    let want = outer.encode();
    let mut t = PrivacyTransport::new(PrivacyMode::Private, MemSink::default());
    t.send(&outer).unwrap();
    assert_eq!(t.pending_count(), 1);
    assert!(t.sink().sent.is_empty());
    assert_eq!(t.advance(0).unwrap(), 0);
    assert_eq!(t.pending_count(), 1);
    let flushed = t
        .advance(PRIVATE_EXTRA_MAX_MS + PRIVATE_BATCH_MAX_MS)
        .unwrap();
    assert_eq!(flushed, 1);
    assert_eq!(t.pending_count(), 0);
    assert_eq!(t.sink().sent, vec![(Hop::Direct, want)]);
}

#[test]
fn private_delay_is_at_least_extra_min() {
    let (_server, outer) = sealed();
    let mut t = PrivacyTransport::new(PrivacyMode::Private, MemSink::default());
    t.send(&outer).unwrap();
    assert_eq!(
        t.advance(PRIVATE_EXTRA_MIN_MS.saturating_sub(1)).unwrap(),
        0
    );
}

#[test]
fn high_uses_tor_hop_and_same_timing() {
    let (_server, outer) = sealed();
    let want = outer.encode();
    assert_eq!(hop_for(PrivacyMode::High), Hop::Tor);
    let mut t = PrivacyTransport::new(PrivacyMode::High, MemSink::default());
    assert_eq!(t.hop(), Hop::Tor);
    t.send(&outer).unwrap();
    t.advance(PRIVATE_EXTRA_MAX_MS + PRIVATE_BATCH_MAX_MS)
        .unwrap();
    assert_eq!(t.sink().sent, vec![(Hop::Tor, want)]);
}

#[test]
fn cover_emits_unchanged_dummy() {
    assert!(reject_maximum().is_ok());
    let (_server, outer) = sealed();
    let want = outer.encode();
    let mut t = PrivacyTransport::new(PrivacyMode::High, MemSink::default());
    t.send_cover(&outer).unwrap();
    t.advance(PRIVATE_EXTRA_MAX_MS + PRIVATE_BATCH_MAX_MS)
        .unwrap();
    assert_eq!(t.sink().sent, vec![(Hop::Tor, want)]);
}

#[test]
fn maximum_is_constant_rate_tor_hop() {
    assert_eq!(hop_for(PrivacyMode::Maximum), Hop::Tor);
    assert!(!nemo_core::privacy::calls_allowed(PrivacyMode::Maximum));
    let (_server, outer) = sealed();
    let want = outer.encode();
    let mut t = PrivacyTransport::new(PrivacyMode::Maximum, MemSink::default());
    t.send(&outer).unwrap();
    assert_eq!(t.pending_count(), 0);
    assert_eq!(t.sink().sent, vec![(Hop::Tor, want)]);
}

#[test]
fn recv_does_not_rewrite_bytes() {
    let bytes = vec![vec![1, 2, 3], vec![4]];
    assert_eq!(PrivacyTransport::<MemSink>::recv(bytes.clone()), bytes);
}

#[test]
fn dummy_is_type_valid_same_bucket() {
    let server = HpkeKeypair::generate();
    let cap = mailbox::random_token();
    let dummy = dummy_outer(&server.public, cap).unwrap();
    let real = mailbox::wrap(
        &server.public,
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![9, 9, 9],
    )
    .unwrap();
    let dummy_inner = mailbox::deliver(&server, &dummy).unwrap();
    let real_inner = mailbox::deliver(&server, &real).unwrap();
    assert_eq!(dummy_inner.padded_message.type_, MessageType::DoubleRatchet);
    assert_eq!(dummy_inner.encode_padded().unwrap().len(), INNER_TEXT_OUTER);
    assert_eq!(real_inner.encode_padded().unwrap().len(), INNER_TEXT_OUTER);
    assert_eq!(dummy.hpke_ciphertext.len(), real.hpke_ciphertext.len());
}

#[test]
fn wake_coalesce_one_per_ten_seconds() {
    let mut w = WakeCoalesce::default();
    assert!(w.should_wake(0));
    assert!(!w.should_wake(WAKE_COALESCE_MS - 1));
    assert!(w.should_wake(WAKE_COALESCE_MS));
    assert!(!w.should_wake(WAKE_COALESCE_MS + 1));
}

struct FailSink;

impl EnvelopeSink for FailSink {
    fn emit(&mut self, _hop: Hop, _outer_bytes: Vec<u8>) -> nemo_core::Result<()> {
        Err(CoreError::CoverNotShipped)
    }
}

#[test]
fn normal_propagates_sink_error() {
    let (_server, outer) = sealed();
    let mut t = PrivacyTransport::new(PrivacyMode::Normal, FailSink);
    assert!(t.send(&outer).is_err());
}
