use nemo_server::federation::{pin_each_other, Enqueue, OUTBOUND_MAX_AGE_SECS};
use nemo_server::group::GroupHost;
use nemo_server::s2s;
use nemo_server::{AppState, HomeServer};
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::KEY_LEN;
use nemo_wire::{identity_id, MailboxOwnerAuth, SigningKey};
use rand::Rng;
use tokio::net::TcpListener;

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

#[tokio::test]
async fn refused_dialer_cannot_pump() {
    nemo_server::tls_install();
    let listen_a = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listen_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_a = listen_a.local_addr().unwrap().port();
    let port_b = listen_b.local_addr().unwrap().port();

    let mut a = HomeServer::advertise("localhost".to_string(), port_a);
    let mut b = HomeServer::advertise("localhost".to_string(), port_b);
    pin_each_other(&mut a, &mut b).unwrap();
    nemo_server::refuse(&mut a, b.server_id());
    let dest = b.server_id();
    let state_a = AppState::from_parts(a, GroupHost::new());
    let state_b = AppState::from_parts(b, GroupHost::new());
    tokio::spawn(s2s::accept_loop(listen_a, state_a.clone()));
    tokio::spawn(s2s::accept_loop(listen_b, state_b));
    assert!(matches!(
        s2s::dial_and_pump(&state_a, dest).await,
        Err(nemo_server::ServerError::PeerRefused)
    ));
}

#[tokio::test]
async fn rotated_tls_key_without_repin_fails_handshake() {
    nemo_server::tls_install();
    let listen_a = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listen_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_a = listen_a.local_addr().unwrap().port();
    let port_b = listen_b.local_addr().unwrap().port();

    let mut a = HomeServer::advertise("localhost".to_string(), port_a);
    let b_old = HomeServer::advertise("localhost".to_string(), port_b);
    let mut b_old = b_old;
    pin_each_other(&mut a, &mut b_old).unwrap();
    let stale_dest = b_old.server_id();

    // B rotates its identity wholesale but keeps the same addr/port.
    // New B pins A so inbound would accept if TLS passed; A still pins old B.
    let mut b_new = HomeServer::advertise("localhost".to_string(), port_b);
    nemo_server::federation::pin(&mut b_new, a.bundle().clone()).unwrap();

    let state_a = AppState::from_parts(a, GroupHost::new());
    let state_b = AppState::from_parts(b_new, GroupHost::new());
    tokio::spawn(s2s::accept_loop(listen_a, state_a.clone()));
    tokio::spawn(s2s::accept_loop(listen_b, state_b));
    assert!(s2s::dial_and_pump(&state_a, stale_dest).await.is_err());
}

#[tokio::test]
async fn expired_outbound_is_expired_over_wire() {
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
    a.enqueue(wrap(&b.hpke_public(), cap, vec![9])).unwrap();

    let state_a = AppState::from_parts(a, GroupHost::new());
    let state_b = AppState::from_parts(b, GroupHost::new());
    tokio::spawn(s2s::accept_loop(listen_a, state_a.clone()));
    tokio::spawn(s2s::accept_loop(listen_b, state_b.clone()));

    // Age the outbound row past the 14-day TTL on the dialer side.
    {
        let mut home_a = state_a.home.lock().await;
        home_a.now += OUTBOUND_MAX_AGE_SECS + 1;
    }
    let stats = s2s::dial_and_pump(&state_a, dest).await.unwrap();
    assert_eq!(stats.delivered, 0);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.expired, 1);
    assert_eq!(state_a.home.lock().await.outbound_len(), 0);
    let mut home_b = state_b.home.lock().await;
    let now = home_b.now;
    assert!(home_b
        .fetch(bob, &auth(&bob_sk, bob, now), now)
        .unwrap()
        .is_empty());
}
