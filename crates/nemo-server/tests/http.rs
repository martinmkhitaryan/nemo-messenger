use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nemo_server::{router, AppState};
use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::{self, identity_id, KEY_LEN};
use nemo_wire::{
    AttachmentReserve, AttachmentSizeBucket, ContactCard, DiscoveryRecord, GroupAdmit, GroupInvite,
    HomeServerBinding, MailboxOwnerAuth, ServerBundle, SigningKey, INTRO_TTL_30_MIN,
};
use rand::RngCore;
use tower::ServiceExt;

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn signed_card(server_id: [u8; KEY_LEN], hpke: [u8; KEY_LEN], sk: &SigningKey) -> ContactCard {
    let binding = HomeServerBinding::sign(
        sk,
        HomeServerBinding {
            server_id,
            server_hpke_public_key: hpke,
            host: "local".into(),
            seq: 1,
            expires_at: unix_now() + 86_400,
            signature: [0; 64],
        },
    )
    .unwrap();
    ContactCard {
        identity_public_key: sk.verifying_key().to_bytes(),
        revocation_public_key: SigningKey::generate(&mut rand::rngs::OsRng)
            .verifying_key()
            .to_bytes(),
        share_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut t);
            t
        },
        binding,
    }
}

fn owner_auth(sk: &SigningKey, id: [u8; KEY_LEN], cursor: u64, limit: u64) -> MailboxOwnerAuth {
    MailboxOwnerAuth::sign(sk, id.to_vec(), cursor, limit, unix_now()).unwrap()
}

fn owner_header(auth: &MailboxOwnerAuth) -> String {
    ids::to_hex(&auth.encode())
}

fn wrap(dest_pk: &[u8; KEY_LEN], capability: [u8; KEY_LEN]) -> OuterEnvelope {
    let inner = InnerEnvelope {
        delivery_capability: capability,
        ttl_bucket: TtlBucket::DEFAULT,
        idempotency_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut t);
            t
        },
        padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, vec![1, 2, 3]).unwrap(),
    };
    seal_to_server(dest_pk, &inner).unwrap()
}

async fn call(
    app: axum::Router,
    req: Request<Body>,
) -> (StatusCode, Vec<(String, String)>, Vec<u8>) {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let headers: Vec<(String, String)> = res
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, body)
}

fn ct<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[tokio::test]
async fn bundle_is_cbor_not_json() {
    let app = router(AppState::new());
    let (status, headers, body) = call(
        app,
        Request::builder()
            .uri("/v1/bundle")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ct(&headers, "content-type"), Some("application/cbor"));
    let bundle = ServerBundle::decode(&body).unwrap();
    assert_eq!(bundle.host, "local");
}

#[tokio::test]
async fn register_discovery_prekey_mailbox_and_group() {
    let state = AppState::new();
    let home = state.home.lock().await;
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(home.server_id(), home.hpke_public(), &sk);
    let id = identity_id(&sk.verifying_key().to_bytes());
    let hpke = home.hpke_public();
    drop(home);

    let app = router(state.clone());
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/register")
            .body(Body::from(card.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body:?}");

    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/register")
            .body(Body::from(card.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, headers, body) = call(
        app.clone(),
        Request::builder()
            .uri(format!("/v1/discovery/{}", ids::to_hex(&id)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ct(&headers, "content-type"), Some("application/cbor"));
    let rec = DiscoveryRecord::decode(&body).unwrap();
    assert_eq!(rec.identity_public_key, card.identity_public_key);

    let missing = [9u8; KEY_LEN];
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .uri(format!("/v1/discovery/{}", ids::to_hex(&missing)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.is_empty());

    let upload_auth = owner_auth(&sk, id, 0, 1);
    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/prekeys")
            .header("nemo-owner", owner_header(&upload_auth))
            .body(Body::from(b"prekey-blob".to_vec()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, headers, body) = call(
        app.clone(),
        Request::builder()
            .uri("/v1/prekeys")
            .header("nemo-token", ids::to_hex(&card.share_token))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ct(&headers, "content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(body, b"prekey-blob");

    let cap_auth = owner_auth(&sk, id, 0, 1);
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/tokens/contact")
            .header("nemo-owner", owner_header(&cap_auth))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let Value::Map(m) = cbor::decode(&body).unwrap() else {
        panic!("token map");
    };
    let cap = ids::copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 0).unwrap()).unwrap()).unwrap();

    let env_auth = owner_auth(&sk, id, 0, 1);
    let outer = wrap(&hpke, cap);
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/envelopes")
            .header("nemo-owner", owner_header(&env_auth))
            .body(Body::from(outer.encode()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let Value::Map(m) = cbor::decode(&body).unwrap() else {
        panic!("enqueue map");
    };
    assert_eq!(
        cbor::expect_text(cbor::map_get(&m, 0).unwrap()).unwrap(),
        "ok"
    );
    assert_eq!(cbor::expect_uint(cbor::map_get(&m, 1).unwrap()).unwrap(), 1);

    let fetch_auth = owner_auth(&sk, id, 0, 16);
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/mailbox/fetch")
            .body(Body::from(fetch_auth.encode()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let Value::Array(items) = cbor::decode(&body).unwrap() else {
        panic!("fetch array");
    };
    assert_eq!(items.len(), 1);
    let Value::Map(row) = &items[0] else {
        panic!("row");
    };
    assert_eq!(
        cbor::expect_uint(cbor::map_get(row, 0).unwrap()).unwrap(),
        1
    );

    let ack_auth = owner_auth(&sk, id, 1, 1);
    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/mailbox/ack")
            .body(Body::from(ack_auth.encode()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let create = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(sk.verifying_key().to_bytes().to_vec())),
        (1, Value::Bytes(cap.to_vec())),
        (2, Value::Bytes(hpke.to_vec())),
    ]));
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/groups")
            .body(Body::from(create))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let Value::Map(g) = cbor::decode(&body).unwrap() else {
        panic!("group map");
    };
    let gid = cbor::expect_bytes(cbor::map_get(&g, 0).unwrap())
        .unwrap()
        .to_vec();
    let cred_id = cbor::expect_bytes(cbor::map_get(&g, 1).unwrap())
        .unwrap()
        .to_vec();
    let cred_secret = cbor::expect_bytes(cbor::map_get(&g, 2).unwrap())
        .unwrap()
        .to_vec();

    let append = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(cred_id)),
        (1, Value::Bytes(cred_secret)),
        (2, Value::Uint(u64::from(MessageType::MlsHandshake as u8))),
        (3, Value::Bytes(b"commit".to_vec())),
    ]));
    let (status, _, body) = call(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/v1/groups/{}/append", ids::to_hex(&gid)))
            .body(Body::from(append))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let Value::Map(a) = cbor::decode(&body).unwrap() else {
        panic!("append map");
    };
    assert_eq!(cbor::expect_uint(cbor::map_get(&a, 0).unwrap()).unwrap(), 1);
}

#[tokio::test]
async fn owner_auth_rejected_without_header() {
    let app = router(AppState::new());
    let (status, _, _) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/prekeys")
            .body(Body::from(b"x".to_vec()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn owner_auth_rejected_when_older_than_120s() {
    let state = AppState::new();
    let (server_id, hpke) = {
        let home = state.home.lock().await;
        (
            home.bundle().server_id,
            home.bundle().server_hpke_public_key,
        )
    };
    let app = router(state);
    let (sk, id, _, _) = register_and_contact(app.clone(), server_id, hpke).await;
    let stale =
        MailboxOwnerAuth::sign(&sk, id.to_vec(), 0, 16, unix_now().saturating_sub(121)).unwrap();
    let (status, _, _) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/mailbox/fetch")
            .body(Body::from(stale.encode()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn json_register_is_bad_request() {
    let app = router(AppState::new());
    let (status, _, _) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/register")
            .header("content-type", "application/json")
            .body(Body::from(br#"{"hello":"world"}"#.to_vec()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

async fn register_and_contact(
    app: axum::Router,
    server_id: [u8; KEY_LEN],
    hpke: [u8; KEY_LEN],
) -> (SigningKey, [u8; KEY_LEN], [u8; KEY_LEN], [u8; KEY_LEN]) {
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(server_id, hpke, &sk);
    let id = identity_id(&sk.verifying_key().to_bytes());
    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/register")
            .body(Body::from(card.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let auth = owner_auth(&sk, id, 0, 1);
    let (status, _, body) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/tokens/contact")
            .header("nemo-owner", owner_header(&auth))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let Value::Map(m) = cbor::decode(&body).unwrap() else {
        panic!("token");
    };
    let cap = ids::copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 0).unwrap()).unwrap()).unwrap();
    (sk, id, cap, hpke)
}

async fn create_group_http(
    app: axum::Router,
    signing_pk: [u8; KEY_LEN],
    cap: [u8; KEY_LEN],
    hpke: [u8; KEY_LEN],
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let create = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(signing_pk.to_vec())),
        (1, Value::Bytes(cap.to_vec())),
        (2, Value::Bytes(hpke.to_vec())),
    ]));
    let (status, _, body) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/groups")
            .body(Body::from(create))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let Value::Map(g) = cbor::decode(&body).unwrap() else {
        panic!("group");
    };
    (
        cbor::expect_bytes(cbor::map_get(&g, 0).unwrap())
            .unwrap()
            .to_vec(),
        cbor::expect_bytes(cbor::map_get(&g, 1).unwrap())
            .unwrap()
            .to_vec(),
        cbor::expect_bytes(cbor::map_get(&g, 2).unwrap())
            .unwrap()
            .to_vec(),
    )
}

#[tokio::test]
async fn group_invite_accept_admit_and_fanout() {
    let state = AppState::new();
    let (server_id, hpke) = {
        let home = state.home.lock().await;
        (home.server_id(), home.hpke_public())
    };
    let app = router(state.clone());
    let (alice_sk, _, alice_cap, hpke) = register_and_contact(app.clone(), server_id, hpke).await;
    let (bob_sk, _, bob_cap, _) = register_and_contact(app.clone(), server_id, hpke).await;

    let alice_group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let bob_group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let (gid, cred_id, cred_secret) = create_group_http(
        app.clone(),
        alice_group_sk.verifying_key().to_bytes(),
        alice_cap,
        hpke,
    )
    .await;

    let mut nonce = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let gid_arr: [u8; KEY_LEN] = ids::copy_fixed(&gid).unwrap();
    let invite =
        GroupInvite::sign(&alice_group_sk, gid_arr, nonce, INTRO_TTL_30_MIN, None).unwrap();
    let invite_body = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(cred_id.clone())),
        (1, Value::Bytes(cred_secret.clone())),
        (2, Value::Bytes(invite.encode())),
    ]));
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/v1/groups/{}/invites", ids::to_hex(&gid)))
            .body(Body::from(invite_body))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body:?}");

    let accept_body = cbor::encode(&Value::Map(vec![(0, Value::Bytes(nonce.to_vec()))]));
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/v1/groups/{}/accept", ids::to_hex(&gid)))
            .body(Body::from(accept_body))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let Value::Map(p) = cbor::decode(&body).unwrap() else {
        panic!("pending");
    };
    let pending_id = cbor::expect_bytes(cbor::map_get(&p, 0).unwrap())
        .unwrap()
        .to_vec();
    let pending_arr: [u8; KEY_LEN] = ids::copy_fixed(&pending_id).unwrap();

    let admit = GroupAdmit::sign(&alice_group_sk, gid_arr, pending_arr).unwrap();
    let admit_body = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(cred_id)),
        (1, Value::Bytes(cred_secret)),
        (2, Value::Bytes(admit.encode())),
        (
            3,
            Value::Bytes(bob_group_sk.verifying_key().to_bytes().to_vec()),
        ),
        (4, Value::Bytes(bob_cap.to_vec())),
        (5, Value::Bytes(hpke.to_vec())),
    ]));
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/v1/groups/{}/admit", ids::to_hex(&gid)))
            .body(Body::from(admit_body))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let fanout = cbor::encode(&Value::Map(vec![
        (
            0,
            Value::Bytes(
                cbor::expect_bytes(cbor::map_get(&p, 1).unwrap())
                    .unwrap()
                    .to_vec(),
            ),
        ),
        (
            1,
            Value::Bytes(
                cbor::expect_bytes(cbor::map_get(&p, 2).unwrap())
                    .unwrap()
                    .to_vec(),
            ),
        ),
        (2, Value::Bytes(bob_cap.to_vec())),
        (3, Value::Bytes(hpke.to_vec())),
    ]));
    let (status, _, _) = call(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/v1/groups/{}/fanout", ids::to_hex(&gid)))
            .body(Body::from(fanout))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let _ = (alice_sk, bob_sk);
}

#[tokio::test]
async fn group_file_upload_and_fetch() {
    let state = AppState::new();
    let (server_id, hpke) = {
        let home = state.home.lock().await;
        (home.server_id(), home.hpke_public())
    };
    let app = router(state.clone());
    let (sk, _, cap, hpke) = register_and_contact(app.clone(), server_id, hpke).await;

    let group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let (gid, cred_id, cred_secret) =
        create_group_http(app.clone(), group_sk.verifying_key().to_bytes(), cap, hpke).await;

    let token = [0x44u8; KEY_LEN];
    let reserve = AttachmentReserve {
        fetch_token: token,
        size_bucket: AttachmentSizeBucket::A1,
        ttl_bucket: TtlBucket::DAY,
    };
    let append = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(cred_id.clone())),
        (1, Value::Bytes(cred_secret.clone())),
        (
            2,
            Value::Uint(u64::from(MessageType::AttachmentReserve as u8)),
        ),
        (3, Value::Bytes(reserve.encode())),
    ]));
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/v1/groups/{}/append", ids::to_hex(&gid)))
            .body(Body::from(append))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let file = vec![7u8; AttachmentSizeBucket::A1.inner_len()];
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/v1/groups/{}/files/{}",
                ids::to_hex(&gid),
                ids::to_hex(&token)
            ))
            .header("nemo-cred-id", ids::to_hex(&cred_id))
            .header("nemo-cred-secret", ids::to_hex(&cred_secret))
            .body(Body::from(file.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body:?}");

    let (status, headers, body) = call(
        app,
        Request::builder()
            .uri(format!(
                "/v1/groups/{}/files/{}",
                ids::to_hex(&gid),
                ids::to_hex(&token)
            ))
            .header("nemo-cred-id", ids::to_hex(&cred_id))
            .header("nemo-cred-secret", ids::to_hex(&cred_secret))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ct(&headers, "content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(body, file);
    let _ = sk;
}

#[tokio::test]
async fn wakeup_sends_empty_binary_on_ingest() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message as WsMsg;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new();
    let home = state.home.lock().await;
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(home.server_id(), home.hpke_public(), &sk);
    let id = identity_id(&sk.verifying_key().to_bytes());
    let hpke = home.hpke_public();
    drop(home);
    let serve_state = state.clone();
    tokio::spawn(async move {
        axum::serve(listener, router(serve_state)).await.ok();
    });

    let client = reqwest::Client::new();
    let base = format!("http://{addr}");
    let res = client
        .post(format!("{base}/v1/register"))
        .body(card.encode().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::NO_CONTENT);

    let share_auth = owner_auth(&sk, id, 0, 1);
    let res = client
        .post(format!("{base}/v1/tokens/share"))
        .header("nemo-owner", owner_header(&share_auth))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let Value::Map(m) = cbor::decode(&res.bytes().await.unwrap()).unwrap() else {
        panic!("token map");
    };
    let token =
        ids::copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 0).unwrap()).unwrap()).unwrap();

    let mut ws_req = format!("ws://{addr}/v1/wakeup")
        .into_client_request()
        .unwrap();
    let wake_auth = owner_auth(&sk, id, 0, 1);
    ws_req
        .headers_mut()
        .insert("nemo-owner", owner_header(&wake_auth).parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(ws_req).await.unwrap();

    let env_auth = owner_auth(&sk, id, 0, 1);
    let res = client
        .post(format!("{base}/v1/envelopes"))
        .header("nemo-owner", owner_header(&env_auth))
        .body(wrap(&hpke, token).encode())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("wake")
        .unwrap()
        .unwrap();
    assert_eq!(msg, WsMsg::Binary(Vec::new().into()));
    let _ = ws.close(None).await;
}

#[tokio::test]
async fn turn_creds_are_ephemeral_and_not_identity() {
    let state = AppState::new();
    let home = state.home.lock().await;
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(home.server_id(), home.hpke_public(), &sk);
    let id = identity_id(&sk.verifying_key().to_bytes());
    drop(home);
    let app = router(state);
    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/register")
            .body(Body::from(card.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let auth = owner_auth(&sk, id, 0, 1);
    let (status, _, body) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/turn")
            .header("nemo-owner", owner_header(&auth))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let Value::Map(m) = cbor::decode(&body).unwrap() else {
        panic!("turn map");
    };
    let user = cbor::expect_text(cbor::map_get(&m, 1).unwrap()).unwrap();
    let pass = cbor::expect_text(cbor::map_get(&m, 2).unwrap()).unwrap();
    assert!(!user.contains(&ids::to_hex(&id)));
    assert!(user.contains(':'));
    assert!(!pass.is_empty());
    let auth2 = owner_auth(&sk, id, 0, 1);
    let (status, _, body2) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/turn")
            .header("nemo-owner", owner_header(&auth2))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let Value::Map(m2) = cbor::decode(&body2).unwrap() else {
        panic!("turn map 2");
    };
    let user2 = cbor::expect_text(cbor::map_get(&m2, 1).unwrap()).unwrap();
    assert_ne!(user, user2);
}

#[tokio::test]
async fn post_binding_higher_seq_disables_mailbox() {
    let state = AppState::new();
    let home = state.home.lock().await;
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(home.server_id(), home.hpke_public(), &sk);
    let id = identity_id(&sk.verifying_key().to_bytes());
    drop(home);

    let app = router(state.clone());
    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/register")
            .body(Body::from(card.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let mut other_hpke = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut other_hpke);
    let moved = ContactCard {
        identity_public_key: card.identity_public_key,
        revocation_public_key: card.revocation_public_key,
        share_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut t);
            t
        },
        binding: HomeServerBinding::sign(
            &sk,
            HomeServerBinding {
                server_id: ids::server_id(&other_hpke),
                server_hpke_public_key: other_hpke,
                host: "other.example".into(),
                seq: 2,
                expires_at: unix_now() + 86_400,
                signature: [0; 64],
            },
        )
        .unwrap(),
    };
    let (status, _, _) = call(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/v1/binding")
            .body(Body::from(moved.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(state.home.lock().await.mailbox_disabled(id));
    let row = state.home.lock().await.discovery(id).unwrap();
    assert_eq!(row.binding.seq, 2);

    let stranger = SigningKey::generate(&mut rand::rngs::OsRng);
    let mut ghost_hpke = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut ghost_hpke);
    let ghost = signed_card(ids::server_id(&ghost_hpke), ghost_hpke, &stranger);
    let (status, _, _) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/v1/binding")
            .body(Body::from(ghost.encode().unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
