use nemo_server::federation::{
    apply_sign_rotate, pin, pin_each_other, pump, refuse, Enqueue, OUTBOUND_MAX_AGE_SECS,
};
use nemo_server::{HomeServer, ServerError};
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::KEY_LEN;
use nemo_wire::{identity_id, MailboxOwnerAuth, SigningKey};
use rand::Rng;

fn wrap(
    dest_pk: &[u8; KEY_LEN],
    capability: [u8; KEY_LEN],
    body: Vec<u8>,
) -> nemo_wire::OuterEnvelope {
    let mut idem = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut idem);
    let inner = InnerEnvelope {
        delivery_capability: capability,
        ttl_bucket: TtlBucket::DEFAULT,
        idempotency_token: idem,
        padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, body).unwrap(),
    };
    seal_to_server(dest_pk, &inner).unwrap()
}

fn register(home: &mut HomeServer) -> (SigningKey, [u8; KEY_LEN]) {
    let sk = SigningKey::generate(&mut rand::rng());
    let id = identity_id(&sk.verifying_key().to_bytes());
    home.register(id, sk.verifying_key().to_bytes()).unwrap();
    (sk, id)
}

fn auth(sk: &SigningKey, id: [u8; KEY_LEN], ts: u64) -> MailboxOwnerAuth {
    MailboxOwnerAuth::sign(sk, id.to_vec(), 0, 16, ts).unwrap()
}

#[test]
fn same_server_enqueue_ingests() {
    let mut home = HomeServer::new();
    let (sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    let outer = wrap(&home.hpke_public(), cap, vec![1]);
    assert!(matches!(home.enqueue(outer).unwrap(), Enqueue::Local(1)));
    let rows = home.fetch(id, &auth(&sk, id, home.now), home.now).unwrap();
    assert_eq!(rows.len(), 1);
}

#[test]
fn a_forwards_to_b() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (bob_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    let outer = wrap(&b.hpke_public(), cap, vec![9, 9]);
    assert_eq!(a.enqueue(outer).unwrap(), Enqueue::Queued);
    let stats = pump(&mut a, &mut b).unwrap();
    assert_eq!(stats.delivered, 1);
    assert_eq!(a.outbound_len(), 0);
    let rows = b.fetch(bob, &auth(&bob_sk, bob, b.now), b.now).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(&rows[0].inner.padded_message.body[..2], &[9, 9]);
}

#[test]
fn unpinned_peer_cannot_pump() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    let (_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    a.enqueue(wrap(&b.hpke_public(), cap, vec![1])).unwrap();
    assert!(matches!(pump(&mut a, &mut b), Err(ServerError::NotPinned)));
}

#[test]
fn refuse_stops_retry() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    a.enqueue(wrap(&b.hpke_public(), cap, vec![1])).unwrap();
    refuse(&mut b, a.server_id());
    assert!(matches!(
        pump(&mut a, &mut b),
        Err(ServerError::PeerRefused)
    ));
    assert_eq!(a.outbound_len(), 1);
}

#[test]
fn outbound_expires_after_14_days() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    a.enqueue(wrap(&b.hpke_public(), cap, vec![1])).unwrap();
    a.now += OUTBOUND_MAX_AGE_SECS + 1;
    let stats = pump(&mut a, &mut b).unwrap();
    assert_eq!(stats.expired, 1);
    assert_eq!(a.outbound_len(), 0);
}

#[test]
fn hpke_replay_does_not_double_append() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (bob_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    let outer = wrap(&b.hpke_public(), cap, vec![3]);
    a.enqueue(outer.clone()).unwrap();
    pump(&mut a, &mut b).unwrap();
    a.enqueue(outer).unwrap();
    pump(&mut a, &mut b).unwrap();
    let rows = b.fetch(bob, &auth(&bob_sk, bob, b.now), b.now).unwrap();
    assert_eq!(rows.len(), 1);
}

#[test]
fn sign_rotate_updates_pin() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let new = SigningKey::generate(&mut rand::rng());
    let rotate = b.sign_rotate(new.verifying_key().to_bytes(), 1).unwrap();
    apply_sign_rotate(&mut a, b.server_id(), &rotate).unwrap();
    assert_eq!(
        a.bundle_pin(b.server_id()).unwrap().server_sign_public_key,
        new.verifying_key().to_bytes()
    );
}

#[test]
fn backoff_defers_until_next_attempt() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    a.enqueue(wrap(&b.hpke_public(), cap, vec![1])).unwrap();
    a.defer_outbound(a.now + 10);
    let stats = pump(&mut a, &mut b).unwrap();
    assert_eq!(stats.deferred, 1);
    assert_eq!(stats.delivered, 0);
    assert_eq!(a.outbound_len(), 1);
}

#[test]
fn bogus_capability_is_failed_not_delivered() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (_sk, bob) = register(&mut b);
    let bogus_cap = [0x77u8; KEY_LEN];
    a.enqueue(wrap(&b.hpke_public(), bogus_cap, vec![1]))
        .unwrap();
    let stats = pump(&mut a, &mut b).unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.delivered, 0);
    assert_eq!(a.outbound_len(), 0);
    let rows = b
        .fetch(bob, &auth(&_sk, bob, b.now), b.now)
        .unwrap_or_default();
    assert!(rows.is_empty());
}

#[test]
fn peer_rate_limit_caps_throughput_per_second() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (_sk, bob) = register(&mut b);
    // Fresh capability per message avoids the 30/min per-capability limit,
    // isolating the 100/sec per-peer limit in `handle_ciphertext`.
    for _ in 0..105 {
        let cap = b.mint_contact_capability(bob).unwrap();
        a.enqueue(wrap(&b.hpke_public(), cap, vec![1])).unwrap();
    }
    let stats = pump(&mut a, &mut b).unwrap();
    assert_eq!(stats.delivered, 100);
    assert_eq!(stats.failed, 5);
    assert_eq!(a.outbound_len(), 0);
}

#[test]
fn pump_only_delivers_to_named_peer() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    let mut c = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    pin_each_other(&mut a, &mut c).unwrap();
    let (bob_sk, bob) = register(&mut b);
    let (cara_sk, cara) = register(&mut c);
    let cap_b = b.mint_contact_capability(bob).unwrap();
    let cap_c = c.mint_contact_capability(cara).unwrap();
    a.enqueue(wrap(&b.hpke_public(), cap_b, vec![1])).unwrap();
    a.enqueue(wrap(&c.hpke_public(), cap_c, vec![2])).unwrap();
    let stats = pump(&mut a, &mut b).unwrap();
    assert_eq!(stats.delivered, 1);
    assert_eq!(a.outbound_len(), 1);
    assert_eq!(
        b.fetch(bob, &auth(&bob_sk, bob, b.now), b.now)
            .unwrap()
            .len(),
        1
    );
    let stats = pump(&mut a, &mut c).unwrap();
    assert_eq!(stats.delivered, 1);
    assert_eq!(a.outbound_len(), 0);
    assert_eq!(
        c.fetch(cara, &auth(&cara_sk, cara, c.now), c.now)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn refuse_on_sender_side_blocks_pump() {
    let mut a = HomeServer::new();
    let mut b = HomeServer::new();
    pin_each_other(&mut a, &mut b).unwrap();
    let (_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    a.enqueue(wrap(&b.hpke_public(), cap, vec![1])).unwrap();
    refuse(&mut a, b.server_id());
    assert!(matches!(
        pump(&mut a, &mut b),
        Err(ServerError::PeerRefused)
    ));
    assert_eq!(a.outbound_len(), 1);
}

#[test]
fn pin_rejects_tampered_bundle() {
    let mut a = HomeServer::new();
    let b = HomeServer::new();
    let mut bad = b.bundle().clone();
    bad.signature[0] ^= 0xff;
    assert!(pin(&mut a, bad).is_err());
    assert!(a.bundle_pin(b.server_id()).is_none());
}

#[test]
fn pin_self_is_noop() {
    let mut a = HomeServer::new();
    let bundle = a.bundle().clone();
    pin(&mut a, bundle).unwrap();
    assert!(a.bundle_pin(a.server_id()).is_none());
}

#[test]
fn sign_rotate_rejects_unknown_peer_and_bad_sig() {
    let mut a = HomeServer::new();
    let b = HomeServer::new();
    let new = SigningKey::generate(&mut rand::rng());
    let rotate = b.sign_rotate(new.verifying_key().to_bytes(), 1).unwrap();
    assert!(matches!(
        apply_sign_rotate(&mut a, b.server_id(), &rotate),
        Err(ServerError::NotPinned)
    ));

    let mut c = HomeServer::new();
    let mut d = HomeServer::new();
    pin_each_other(&mut c, &mut d).unwrap();
    let attacker = SigningKey::generate(&mut rand::rng());
    let forged = d
        .sign_rotate(attacker.verifying_key().to_bytes(), 1)
        .unwrap();
    // Corrupt the signature so verification against the pinned key fails.
    let mut tampered = forged.clone();
    tampered.signature[0] ^= 0x01;
    assert!(apply_sign_rotate(&mut c, d.server_id(), &tampered).is_err());
}
