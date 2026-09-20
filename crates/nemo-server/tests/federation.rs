use nemo_server::federation::{
    apply_sign_rotate, pin_each_other, pump, refuse, Enqueue, OUTBOUND_MAX_AGE_SECS,
};
use nemo_server::{HomeServer, ServerError};
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::KEY_LEN;
use nemo_wire::{identity_id, MailboxOwnerAuth, SigningKey};
use rand::RngCore;

fn wrap(
    dest_pk: &[u8; KEY_LEN],
    capability: [u8; KEY_LEN],
    body: Vec<u8>,
) -> nemo_wire::OuterEnvelope {
    let mut idem = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut idem);
    let inner = InnerEnvelope {
        delivery_capability: capability,
        ttl_bucket: TtlBucket::DEFAULT,
        idempotency_token: idem,
        padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, body).unwrap(),
    };
    seal_to_server(dest_pk, &inner).unwrap()
}

fn register(home: &mut HomeServer) -> (SigningKey, [u8; KEY_LEN]) {
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
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
    let new = SigningKey::generate(&mut rand::rngs::OsRng);
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
