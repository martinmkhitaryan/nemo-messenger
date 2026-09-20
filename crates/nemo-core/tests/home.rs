use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use nemo_core::app::{decode_text, encode_text};
use nemo_core::discovery::resolve_contact;
use nemo_core::home::{
    EnqueueResult, HomeSession, HomeTransport, HttpHome, HttpRequest, HttpResponse,
};
use nemo_core::identity::{revocation_from_mnemonic, Installation};
use nemo_core::{CoreError, Vault};
use nemo_server::{router, AppState};
use nemo_wire::envelope::TtlBucket;
use nemo_wire::{GroupAdmit, GroupInvite, SigningKey, INTRO_TTL_30_MIN};
use rand::RngCore;
use tower::ServiceExt;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

#[derive(Clone)]
struct RouterTransport {
    app: axum::Router,
}

impl HomeTransport for RouterTransport {
    async fn call(&self, req: HttpRequest) -> nemo_core::Result<HttpResponse> {
        let mut builder = Request::builder().method(req.method).uri(&req.path);
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let res = self
            .app
            .clone()
            .oneshot(
                builder
                    .body(Body::from(req.body))
                    .map_err(|_| CoreError::HomeHttp(0))?,
            )
            .await
            .map_err(|_| CoreError::HomeHttp(0))?;
        let status = res.status().as_u16();
        let body = res
            .into_body()
            .collect()
            .await
            .map_err(|_| CoreError::HomeHttp(status))?
            .to_bytes()
            .to_vec();
        Ok(HttpResponse { status, body })
    }
}

#[tokio::test]
async fn alice_messages_bob_through_home_http() {
    let transport = RouterTransport {
        app: router(AppState::new()),
    };
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, _) = Installation::create().unwrap();
    let (mut alice, _) = HomeSession::register(transport.clone(), alice_inst, now)
        .await
        .unwrap();
    let (mut bob, bob_card) = HomeSession::register(transport, bob_inst, now)
        .await
        .unwrap();
    bob.publish_prekey().await.unwrap();

    let disc = alice.discovery(bob.identity_id()).await.unwrap();
    resolve_contact(&bob_card, &disc, now).unwrap();
    let prekey = alice.fetch_prekey(bob_card.share_token).await.unwrap();
    alice
        .install
        .start_session(&bob_card, &prekey, now)
        .await
        .unwrap();

    let ptext = encode_text(1, now, "hello bob").unwrap();
    let outer = alice
        .install
        .encrypt_to_mailbox(
            &bob.identity_id(),
            &bob.hpke_public(),
            bob_card.share_token,
            TtlBucket::DEFAULT,
            &ptext,
        )
        .await
        .unwrap();
    assert!(matches!(
        alice.post_envelope(&outer).await.unwrap(),
        EnqueueResult::Local(_)
    ));

    let rows = bob.fetch_mailbox().await.unwrap();
    assert_eq!(rows.len(), 1);
    let opened = bob
        .install
        .decrypt_from_mailbox(&alice.identity_id(), &rows[0].inner)
        .await
        .unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "hello bob".into()));
    bob.ack().await.unwrap();
}

#[tokio::test]
async fn group_join_through_home_client() {
    let transport = RouterTransport {
        app: router(AppState::new()),
    };
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, _) = Installation::create().unwrap();
    let (alice, _) = HomeSession::register(transport.clone(), alice_inst, now)
        .await
        .unwrap();
    let (bob, _) = HomeSession::register(transport, bob_inst, now)
        .await
        .unwrap();
    let alice_cap = alice.mint_contact().await.unwrap();
    let bob_cap = bob.mint_contact().await.unwrap();

    let alice_group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let bob_group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let created = alice
        .create_group(alice_group_sk.verifying_key().to_bytes(), alice_cap)
        .await
        .unwrap();

    let nonce = [0x22u8; 32];
    let invite = GroupInvite::sign(
        &alice_group_sk,
        created.group_id,
        nonce,
        INTRO_TTL_30_MIN,
        None,
    )
    .unwrap();
    alice
        .store_invite(created.group_id, &created.cred, &invite)
        .await
        .unwrap();
    let pending = bob.accept_invite(created.group_id, nonce).await.unwrap();
    let admit = GroupAdmit::sign(&alice_group_sk, created.group_id, pending.pending_id).unwrap();
    let activated = alice
        .admit(
            created.group_id,
            &created.cred,
            &admit,
            bob_group_sk.verifying_key().to_bytes(),
            bob_cap,
            bob.hpke_public(),
        )
        .await
        .unwrap();
    assert_eq!(activated, pending.cred.credential_id);
    bob.refresh_fanout(created.group_id, &pending.cred, bob_cap, bob.hpke_public())
        .await
        .unwrap();
}

#[tokio::test]
async fn unknown_discovery_is_denied() {
    let transport = RouterTransport {
        app: router(AppState::new()),
    };
    let (inst, _) = Installation::create().unwrap();
    let (alice, _) = HomeSession::register(transport, inst, now_unix())
        .await
        .unwrap();
    let err = alice.discovery([0x11u8; 32]).await.unwrap_err();
    assert!(matches!(err, CoreError::Denied));
}

#[tokio::test]
async fn reqwest_talks_to_listening_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(AppState::new()))
            .await
            .expect("serve");
    });
    let t = HttpHome::new(format!("http://{addr}")).unwrap();
    let (inst, _) = Installation::create().unwrap();
    let (alice, _) = HomeSession::register(t, inst, now_unix()).await.unwrap();
    alice
        .discovery(alice.identity_id())
        .await
        .expect("own discovery");
}

#[tokio::test]
async fn wakeup_empty_binary_then_fetch() {
    use nemo_core::app::encode_text;
    use nemo_wire::envelope::TtlBucket;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(AppState::new()))
            .await
            .expect("serve");
    });
    let now = now_unix();
    let base = format!("http://{addr}");
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, _) = Installation::create().unwrap();
    let (mut alice, alice_card) =
        HomeSession::register(HttpHome::new(base.clone()).unwrap(), alice_inst, now)
            .await
            .unwrap();
    let (mut bob, _) = HomeSession::register(HttpHome::new(base).unwrap(), bob_inst, now)
        .await
        .unwrap();
    alice.publish_prekey().await.unwrap();
    let cap = alice.mint_contact().await.unwrap();
    bob.add_contact(&alice_card, cap, now).await.unwrap();

    let wake = tokio::spawn({
        let handle = alice.wakeup_handle().unwrap();
        async move { handle.0.wait_wakeup(&handle.1).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let ptext = encode_text(1, now, "wake me").unwrap();
    bob.send_to(&alice.identity_id(), TtlBucket::DEFAULT, &ptext, now)
        .await
        .unwrap();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(3), wake)
        .await
        .expect("wakeup joined")
        .unwrap()
        .unwrap();
    assert!(frame.is_empty());
    let rows = alice.fetch_mailbox().await.unwrap();
    assert_eq!(rows.len(), 1);
}

fn temp_vault() -> std::path::PathBuf {
    let mut n = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut n);
    let dir = std::env::temp_dir().join(format!("nemo-home-vault-{}", nemo_wire::ids::to_hex(&n)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn vault_restart_keeps_1to1_session() {
    let transport = RouterTransport {
        app: router(AppState::new()),
    };
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, _) = Installation::create().unwrap();
    let (mut alice, _) = HomeSession::register(transport.clone(), alice_inst, now)
        .await
        .unwrap();
    let (mut bob, bob_card) = HomeSession::register(transport.clone(), bob_inst, now)
        .await
        .unwrap();
    bob.publish_prekey().await.unwrap();
    let bob_cap = bob.mint_contact().await.unwrap();
    alice.add_contact(&bob_card, bob_cap, now).await.unwrap();

    let ptext = encode_text(1, now, "hello bob").unwrap();
    assert!(matches!(
        alice
            .send_to(&bob.identity_id(), TtlBucket::DEFAULT, &ptext, now)
            .await
            .unwrap(),
        EnqueueResult::Local(_)
    ));
    let rows = bob.fetch_mailbox().await.unwrap();
    let opened = bob
        .install
        .decrypt_from_mailbox(&alice.identity_id(), &rows[0].inner)
        .await
        .unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "hello bob".into()));
    bob.ack().await.unwrap();

    let alice_dir = temp_vault();
    let bob_dir = temp_vault();
    let alice_vault = Vault::create(&alice_dir, "correct horse", &alice.install).unwrap();
    alice_vault
        .save_home(&alice.install, &alice.snapshot())
        .unwrap();
    let bob_vault = Vault::create(&bob_dir, "correct horse", &bob.install).unwrap();
    bob_vault.save_home(&bob.install, &bob.snapshot()).unwrap();
    drop(alice);
    drop(bob);
    drop(alice_vault);
    drop(bob_vault);

    let (alice_vault, alice_inst) = Vault::open(&alice_dir, "correct horse").unwrap();
    let alice_state = alice_vault.load_home().unwrap().unwrap();
    let mut alice = HomeSession::resume(transport.clone(), alice_inst, alice_state);
    let (bob_vault, bob_inst) = Vault::open(&bob_dir, "correct horse").unwrap();
    let bob_state = bob_vault.load_home().unwrap().unwrap();
    let mut bob = HomeSession::resume(transport, bob_inst, bob_state);

    let ptext = encode_text(2, now, "still there").unwrap();
    alice
        .send_to(&bob.identity_id(), TtlBucket::DEFAULT, &ptext, now)
        .await
        .unwrap();
    let rows = bob.fetch_mailbox().await.unwrap();
    let opened = bob
        .install
        .decrypt_from_mailbox(&alice.identity_id(), &rows[0].inner)
        .await
        .unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (2, "still there".into()));
    let _ = std::fs::remove_dir_all(&alice_dir);
    let _ = std::fs::remove_dir_all(&bob_dir);
}

#[tokio::test]
async fn revoked_contact_refuses_send() {
    let transport = RouterTransport {
        app: router(AppState::new()),
    };
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, bob_export) = Installation::create().unwrap();
    let (mut alice, _) = HomeSession::register(transport.clone(), alice_inst, now)
        .await
        .unwrap();
    let (mut bob, bob_card) = HomeSession::register(transport, bob_inst, now)
        .await
        .unwrap();
    bob.publish_prekey().await.unwrap();
    let bob_cap = bob.mint_contact().await.unwrap();
    alice.add_contact(&bob_card, bob_cap, now).await.unwrap();

    let stmt = revocation_from_mnemonic(&bob_export.mnemonic, bob.identity_id(), now).unwrap();
    alice.submit_revocation(&stmt).await.unwrap();
    let err = alice
        .refresh_contact(&bob.identity_id(), now)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Revoked));
    let ptext = encode_text(1, now, "nope").unwrap();
    let err = alice
        .send_to(&bob.identity_id(), TtlBucket::DEFAULT, &ptext, now)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Revoked));
}

/// Set `NEMO_TEST_HOME_URL` (e.g. `https://localhost:8443` after compose up).
#[tokio::test]
async fn two_installations_register_on_env_home() {
    let Ok(base) = std::env::var("NEMO_TEST_HOME_URL") else {
        eprintln!("skip two_installations_register_on_env_home: set NEMO_TEST_HOME_URL");
        return;
    };
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, _) = Installation::create().unwrap();
    let (mut alice, alice_card) =
        HomeSession::register(HttpHome::new(&base).unwrap(), alice_inst, now)
            .await
            .expect("alice register");
    let (mut bob, bob_card) = HomeSession::register(HttpHome::new(&base).unwrap(), bob_inst, now)
        .await
        .expect("bob register");
    assert_ne!(alice_card.identity_id(), bob_card.identity_id());

    alice.publish_prekey().await.unwrap();
    let cap = alice.mint_contact().await.unwrap();
    bob.add_contact(&alice_card, cap, now).await.unwrap();
    let wake = tokio::spawn({
        let handle = alice.wakeup_handle().unwrap();
        async move { handle.0.wait_wakeup(&handle.1).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let ptext = encode_text(1, now, "https wake").unwrap();
    bob.send_to(&alice.identity_id(), TtlBucket::DEFAULT, &ptext, now)
        .await
        .unwrap();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(8), wake)
        .await
        .expect("wss wakeup joined")
        .unwrap()
        .expect("wss wakeup");
    assert!(frame.is_empty());
}
