use nemo_server::federation::{pin_each_other, Enqueue};
use nemo_server::group::GroupHost;
use nemo_server::s2s;
use nemo_server::{AppState, HomeServer};
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::KEY_LEN;
use nemo_wire::{identity_id, MailboxOwnerAuth, SigningKey};
use rand::RngCore;
use tokio::net::TcpListener;

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

#[tokio::test]
async fn mtls_forwards_envelope() {
    nemo_server::tls_install();
    let listen_a = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listen_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_a = listen_a.local_addr().unwrap().port();
    let port_b = listen_b.local_addr().unwrap().port();

    let mut a = HomeServer::advertise("localhost".to_string(), port_a);
    let mut b = HomeServer::advertise("localhost".to_string(), port_b);
    pin_each_other(&mut a, &mut b).unwrap();
    let dest = b.server_id();
    let (bob_sk, bob) = register(&mut b);
    let cap = b.mint_contact_capability(bob).unwrap();
    assert!(matches!(
        a.enqueue(wrap(&b.hpke_public(), cap, vec![7, 7, 7])),
        Ok(Enqueue::Queued)
    ));

    let state_a = AppState::from_parts(a, GroupHost::new());
    let state_b = AppState::from_parts(b, GroupHost::new());
    tokio::spawn(s2s::accept_loop(listen_a, state_a.clone()));
    tokio::spawn(s2s::accept_loop(listen_b, state_b.clone()));

    let stats = s2s::dial_and_pump(&state_a, dest).await.unwrap();
    assert_eq!(stats.delivered, 1);

    let mut home_b = state_b.home.lock().await;
    let now = home_b.now;
    let rows = home_b.fetch(bob, &auth(&bob_sk, bob, now), now).unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn unpinned_peer_cannot_handshake() {
    nemo_server::tls_install();
    let listen_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_b = listen_b.local_addr().unwrap().port();
    let a = HomeServer::advertise("localhost".to_string(), 1);
    let b = HomeServer::advertise("localhost".to_string(), port_b);
    let dest = b.server_id();
    let state_a = AppState::from_parts(a, GroupHost::new());
    let state_b = AppState::from_parts(b, GroupHost::new());
    tokio::spawn(s2s::accept_loop(listen_b, state_b));
    assert!(s2s::dial_and_pump(&state_a, dest).await.is_err());
}
