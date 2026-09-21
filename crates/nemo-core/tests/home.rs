use std::collections::HashMap;
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
use nemo_wire::envelope::{MessageType, TtlBucket};
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
    origin: String,
    apps: std::sync::Arc<HashMap<String, axum::Router>>,
}

impl RouterTransport {
    fn single(app: axum::Router) -> Self {
        let mut apps = HashMap::new();
        apps.insert(String::new(), app);
        Self {
            origin: String::new(),
            apps: std::sync::Arc::new(apps),
        }
    }
}

impl HomeTransport for RouterTransport {
    fn redirect(&self, origin: &str) -> Option<Self> {
        let origin = origin.trim_end_matches('/');
        if origin.is_empty() || !self.apps.contains_key(origin) {
            return None;
        }
        Some(Self {
            origin: origin.to_string(),
            apps: std::sync::Arc::clone(&self.apps),
        })
    }

    async fn call(&self, req: HttpRequest) -> nemo_core::Result<HttpResponse> {
        let app = self
            .apps
            .get(&self.origin)
            .or_else(|| self.apps.get(""))
            .ok_or(CoreError::HomeHttp(0))?;
        let mut builder = Request::builder().method(req.method).uri(&req.path);
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let res = app
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
    let transport = RouterTransport::single(router(AppState::new()));
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
    let transport = RouterTransport::single(router(AppState::new()));
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
    let transport = RouterTransport::single(router(AppState::new()));
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
    let transport = RouterTransport::single(router(AppState::new()));
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
    let transport = RouterTransport::single(router(AppState::new()));
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

#[tokio::test]
async fn idle_discovery_notices_revocation_without_send() {
    let transport = RouterTransport::single(router(AppState::new()));
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
    alice
        .contacts
        .get_mut(&bob.identity_id())
        .unwrap()
        .last_discovery_unix = 0;
    let revoked = alice.refresh_idle_discovery(now).await;
    assert!(revoked.contains(&bob.identity_id()));
    assert!(alice.contacts[&bob.identity_id()].revoked);
}

#[tokio::test]
async fn revoked_identity_fails_discovery_check() {
    let transport = RouterTransport::single(router(AppState::new()));
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, bob_export) = Installation::create().unwrap();
    let (alice, _) = HomeSession::register(transport.clone(), alice_inst, now)
        .await
        .unwrap();
    let (bob, _) = HomeSession::register(transport, bob_inst, now)
        .await
        .unwrap();
    let stmt = revocation_from_mnemonic(&bob_export.mnemonic, bob.identity_id(), now).unwrap();
    alice.submit_revocation(&stmt).await.unwrap();
    let err = alice
        .check_discovery_not_revoked(bob.identity_id(), now)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Revoked));
    alice
        .check_discovery_not_revoked(alice.identity_id(), now)
        .await
        .unwrap();
}

#[tokio::test]
async fn rehome_opens_new_mailbox_and_disables_old() {
    let a_state = AppState::new();
    let b_state = AppState::new();
    let a = RouterTransport::single(router(a_state.clone()));
    let b = RouterTransport::single(router(b_state.clone()));
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (mut alice, _) = HomeSession::register(a.clone(), alice_inst, now)
        .await
        .unwrap();
    let created = alice
        .create_group([7u8; 32], alice.mint_contact().await.unwrap())
        .await
        .unwrap();
    alice.remember_group(created);
    let old_seq = alice.install.binding_seq();
    let id = alice.identity_id();

    let same = alice.rehome(a.clone(), "", now).await.unwrap_err();
    assert!(matches!(same, CoreError::AlreadyRegistered));

    let card = alice.rehome(b, "http://b.example", now).await.unwrap();
    assert_eq!(card.binding.seq, old_seq + 1);
    assert_eq!(alice.install.binding_seq(), old_seq + 1);
    assert_eq!(alice.home_base, "http://b.example");
    assert_eq!(alice.cursor, 0);
    assert!(a_state.home.lock().await.mailbox_disabled(id));
    let on_b = b_state.home.lock().await.discovery(id).unwrap();
    assert_eq!(on_b.binding.seq, old_seq + 1);
    let on_a = a_state.home.lock().await.discovery(id).unwrap();
    assert_eq!(on_a.binding.seq, old_seq + 1);
    assert_eq!(on_a.binding.server_id, on_b.binding.server_id);
    alice.mint_contact().await.expect("new mailbox on B");
}

#[tokio::test]
async fn rehome_keeps_group_append_on_old_host() {
    let a_state = AppState::new();
    let b_state = AppState::new();
    let a_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let b_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a_addr = a_listener.local_addr().unwrap();
    let b_addr = b_listener.local_addr().unwrap();
    let a_app = router(a_state.clone());
    let b_app = router(b_state);
    tokio::spawn(async move {
        axum::serve(a_listener, a_app).await.expect("serve a");
    });
    tokio::spawn(async move {
        axum::serve(b_listener, b_app).await.expect("serve b");
    });
    let now = now_unix();
    let a_base = format!("http://{a_addr}");
    let b_base = format!("http://{b_addr}");
    let (alice_inst, _) = Installation::create().unwrap();
    let (mut alice, _) = HomeSession::register(HttpHome::new(&a_base).unwrap(), alice_inst, now)
        .await
        .unwrap();
    alice.set_home_base(&a_base);
    let created = alice
        .create_group([7u8; 32], alice.mint_contact().await.unwrap())
        .await
        .unwrap();
    assert_eq!(created.host_base, a_base);
    alice.remember_group(created.clone());
    alice
        .rehome(HttpHome::new(&b_base).unwrap(), &b_base, now)
        .await
        .unwrap();
    assert_eq!(alice.groups[0].host_base, a_base);
    alice
        .group_append(
            created.group_id,
            &created.cred,
            MessageType::MlsApp,
            vec![1, 2, 3],
        )
        .await
        .expect("append on old group host");
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

#[tokio::test]
async fn add_contact_fetches_discovery_and_prekey_on_peer_home() {
    let mut home_a = nemo_server::home::HomeServer::advertise("home-a", 9443);
    let mut home_b = nemo_server::home::HomeServer::advertise("home-b", 9444);
    nemo_server::pin_each_other(&mut home_a, &mut home_b).unwrap();
    let a_state = AppState::from_parts(home_a, nemo_server::group::GroupHost::new());
    let b_state = AppState::from_parts(home_b, nemo_server::group::GroupHost::new());
    let mut apps = HashMap::new();
    apps.insert("http://home-a".into(), router(a_state));
    apps.insert("http://home-b".into(), router(b_state));
    let apps = std::sync::Arc::new(apps);
    let a_tr = RouterTransport {
        origin: "http://home-a".into(),
        apps: std::sync::Arc::clone(&apps),
    };
    let b_tr = RouterTransport {
        origin: "http://home-b".into(),
        apps,
    };
    let now = now_unix();
    let (alice_inst, _) = Installation::create().unwrap();
    let (bob_inst, _) = Installation::create().unwrap();
    let (mut alice, _) = HomeSession::register(a_tr, alice_inst, now).await.unwrap();
    alice.set_home_base("http://home-a");
    let (mut bob, bob_card) = HomeSession::register(b_tr, bob_inst, now).await.unwrap();
    bob.set_home_base("http://home-b");
    bob.publish_prekey().await.unwrap();
    let cap = bob.mint_contact().await.unwrap();
    alice.add_contact(&bob_card, cap, now).await.unwrap();
    let stored = &alice.contacts[&bob.identity_id()];
    assert_eq!(stored.home_origin, "http://home-b");
    assert_eq!(stored.dest_hpke, bob.hpke_public());
}
