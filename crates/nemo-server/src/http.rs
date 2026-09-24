//! Local HTTP (phase 8). Caddy terminates TLS; this process binds `NEMO_LISTEN` (default `0.0.0.0:8787`).

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{MessageType, OuterEnvelope};
use nemo_wire::ids::{self, IdentityId, KEY_LEN};
use nemo_wire::{
    ContactCard, DiscoveryRecord, GroupAdmit, GroupInvite, MailboxOwnerAuth, RevocationStatement,
};
use sqlx::PgPool;
use tokio::sync::Notify;

use crate::error::ServerError;
use crate::federation::Enqueue;
use crate::group::{FanoutTarget, GroupHost, GroupId, MemberCred};
use crate::home::{DiscoveryRow, HomeServer};

/// A4 outer plus a little headroom.
const BODY_LIMIT: usize = 18_000_000;
/// At most one empty wake frame per mailbox connection (ADR-0020 / phase 6).
const WAKE_COALESCE: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct AppState {
    pub home: Arc<tokio::sync::Mutex<HomeServer>>,
    pub groups: Arc<tokio::sync::Mutex<GroupHost>>,
    pub db: Option<PgPool>,
    pub wakes: Arc<Notify>,
}

impl AppState {
    pub fn new() -> Self {
        Self::from_parts(HomeServer::new(), GroupHost::new())
    }

    pub fn from_parts(home: HomeServer, groups: GroupHost) -> Self {
        Self {
            home: Arc::new(tokio::sync::Mutex::new(home)),
            groups: Arc::new(tokio::sync::Mutex::new(groups)),
            db: None,
            wakes: Arc::new(Notify::new()),
        }
    }

    pub fn with_db(mut self, db: PgPool) -> Self {
        self.db = Some(db);
        self
    }

    pub async fn persist(&self) {
        let Some(pool) = &self.db else {
            return;
        };
        let home = self.home.lock().await;
        let groups = self.groups.lock().await;
        if let Err(e) = crate::pg::flush(pool, &home, &groups).await {
            crate::log_ops("nemo-server persist", e);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn router(state: AppState) -> Router {
    let persist_state = state.clone();
    Router::new()
        .route("/v1/bundle", get(bundle))
        .route("/v1/register", post(register))
        .route("/v1/binding", post(observe_card_binding))
        .route("/v1/discovery/{id}", get(discovery))
        .route("/v1/prekeys", get(fetch_prekey).post(upload_prekey))
        .route("/v1/envelopes", post(post_envelope))
        .route("/v1/mailbox/fetch", post(mailbox_fetch))
        .route("/v1/mailbox/ack", post(mailbox_ack))
        .route("/v1/tokens/share", post(mint_share))
        .route("/v1/tokens/contact", post(mint_contact))
        .route("/v1/revocation", post(revocation))
        .route("/v1/wakeup", get(wakeup))
        .route("/v1/turn", post(issue_turn))
        .route("/v1/groups", post(create_group))
        .route("/v1/groups/{id}/append", post(group_append))
        .route("/v1/groups/{id}/invites", post(group_invite))
        .route("/v1/groups/{id}/accept", post(group_accept))
        .route("/v1/groups/{id}/admit", post(group_admit))
        .route("/v1/groups/{id}/fanout", post(group_fanout))
        .route(
            "/v1/groups/{id}/files/{token}",
            get(group_fetch_file).post(group_upload_file),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(persist_state, persist_after))
        .with_state(state)
}

async fn persist_after(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let res = next.run(req).await;
    let mutating = (method != Method::GET || path == "/v1/prekeys") && path != "/v1/turn";
    if mutating && res.status().is_success() {
        st.wakes.notify_waiters();
        if st.db.is_some() {
            st.persist().await;
        }
    }
    res
}

pub async fn serve(addr: &str, state: AppState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn denied() -> StatusCode {
    StatusCode::FORBIDDEN
}

fn map_err(err: ServerError) -> StatusCode {
    match err {
        ServerError::AlreadyRegistered => StatusCode::CONFLICT,
        ServerError::OwnerAuth => StatusCode::UNAUTHORIZED,
        ServerError::FetchLimit => StatusCode::BAD_REQUEST,
        ServerError::Wire(_) => StatusCode::BAD_REQUEST,
        _ => denied(),
    }
}

fn cbor_ok(bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "application/cbor")], bytes).into_response()
}

async fn bundle(State(st): State<AppState>) -> Response {
    let home = st.home.lock().await;
    cbor_ok(home.bundle().encode())
}

async fn register(State(st): State<AppState>, body: Bytes) -> StatusCode {
    let Ok(card) = ContactCard::decode(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    match home.register_from_card(&card) {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

/// Higher `seq` from another home disables this mailbox (README §11). No new table.
async fn observe_card_binding(State(st): State<AppState>, body: Bytes) -> StatusCode {
    let Ok(card) = ContactCard::decode(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    match home.observe_binding(card.identity_id(), &card.binding) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

async fn discovery(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(identity_id) = ids::parse_identity_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let home = st.home.lock().await;
    match home.discovery(identity_id) {
        Ok(row) => cbor_ok(encode_discovery(&row)),
        Err(_) => denied().into_response(),
    }
}

fn encode_discovery(row: &DiscoveryRow) -> Vec<u8> {
    DiscoveryRecord {
        identity_public_key: row.identity_public_key,
        revocation_public_key: row.revocation_public_key,
        binding: row.binding.clone(),
        revocation: row.revocation.clone(),
    }
    .encode()
}

async fn fetch_prekey(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = header_token(&headers, "nemo-token") else {
        return denied().into_response();
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    match home.fetch_prekey(token) {
        Ok(blob) => ([(header::CONTENT_TYPE, "application/octet-stream")], blob).into_response(),
        Err(_) => denied().into_response(),
    }
}

async fn upload_prekey(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let (owner, auth) = match require_owner(&headers) {
        Ok(v) => v,
        Err(s) => return s,
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    if let Err(e) = home.check_owner(owner, &auth, unix_now()) {
        return map_err(e);
    }
    match home.publish_prekey(owner, body.to_vec()) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

async fn post_envelope(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let (owner, auth) = match require_owner(&headers) {
        Ok(v) => v,
        Err(s) => return s.into_response(),
    };
    let Ok(outer) = OuterEnvelope::decode(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    match home.enqueue_from_owner(owner, &auth, unix_now(), outer) {
        Ok(Enqueue::Local(seq)) => cbor_ok(cbor::encode(&Value::Map(vec![
            (0, Value::Text("ok".into())),
            (1, Value::Uint(seq)),
        ]))),
        Ok(Enqueue::Queued) => cbor_ok(cbor::encode(&Value::Map(vec![(
            0,
            Value::Text("queued".into()),
        )]))),
        Err(e) => map_err(e).into_response(),
    }
}

async fn mailbox_fetch(State(st): State<AppState>, body: Bytes) -> Response {
    let Ok(auth) = MailboxOwnerAuth::decode(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(owner) = owner_id(&auth) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    match home.fetch(owner, &auth, unix_now()) {
        Ok(rows) => {
            let mut items = Vec::new();
            for r in rows {
                let Ok(inner) = r.inner.encode_padded() else {
                    continue;
                };
                items.push(Value::Map(vec![
                    (0, Value::Uint(r.seq)),
                    (1, Value::Bytes(inner)),
                ]));
            }
            cbor_ok(cbor::encode(&Value::Array(items)))
        }
        Err(e) => map_err(e).into_response(),
    }
}

async fn mailbox_ack(State(st): State<AppState>, body: Bytes) -> StatusCode {
    let Ok(auth) = MailboxOwnerAuth::decode(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(owner) = owner_id(&auth) else {
        return StatusCode::BAD_REQUEST;
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    match home.ack(owner, &auth, unix_now()) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

async fn mint_share(State(st): State<AppState>, headers: HeaderMap) -> Response {
    mint_token(st, headers, true).await
}

async fn mint_contact(State(st): State<AppState>, headers: HeaderMap) -> Response {
    mint_token(st, headers, false).await
}

async fn issue_turn(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let (owner, auth) = match require_owner(&headers) {
        Ok(v) => v,
        Err(s) => return s.into_response(),
    };
    let home = st.home.lock().await;
    if let Err(e) = home.check_owner(owner, &auth, unix_now()) {
        return map_err(e).into_response();
    }
    drop(home);
    let cred = crate::turn::issue();
    cbor_ok(cbor::encode(&Value::Map(vec![
        (0, Value::Text(cred.url)),
        (1, Value::Text(cred.username)),
        (2, Value::Text(cred.credential)),
        (3, Value::Uint(cred.ttl_secs)),
    ])))
}

async fn mint_token(st: AppState, headers: HeaderMap, share: bool) -> Response {
    let (owner, auth) = match require_owner(&headers) {
        Ok(v) => v,
        Err(s) => return s.into_response(),
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    if let Err(e) = home.check_owner(owner, &auth, unix_now()) {
        return map_err(e).into_response();
    }
    let minted = if share {
        home.mint_share_token(owner, None)
    } else {
        home.mint_contact_capability(owner)
    };
    match minted {
        Ok(token) => cbor_ok(cbor::encode(&Value::Map(vec![(
            0,
            Value::Bytes(token.to_vec()),
        )]))),
        Err(e) => map_err(e).into_response(),
    }
}

async fn revocation(State(st): State<AppState>, body: Bytes) -> Response {
    let Ok(stmt) = RevocationStatement::decode(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut home = st.home.lock().await;
    home.now = unix_now();
    let groups = match home.ingest_revocation(&stmt) {
        Ok(g) => g,
        Err(e) => return map_err(e).into_response(),
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    for gid in groups {
        let _ = host.append_host_revocation(gid, &stmt);
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn wakeup(State(st): State<AppState>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    let (owner, auth) = match require_owner(&headers) {
        Ok(v) => v,
        Err(s) => return s.into_response(),
    };
    {
        let home = st.home.lock().await;
        if let Err(e) = home.check_owner(owner, &auth, unix_now()) {
            return map_err(e).into_response();
        }
    }
    ws.on_upgrade(move |socket| run_wakeup(st, owner, auth.cursor, socket))
}

async fn run_wakeup(st: AppState, owner: IdentityId, cursor: u64, mut ws: WebSocket) {
    let mut last_seen = cursor;
    let mut last_wake = Instant::now() - WAKE_COALESCE;
    loop {
        let wait = st.wakes.notified();
        tokio::pin!(wait);
        wait.as_mut().enable();
        let head = {
            let home = st.home.lock().await;
            home.mailbox_head(owner)
        };
        if head > last_seen && last_wake.elapsed() >= WAKE_COALESCE {
            if ws.send(Message::Binary(Bytes::new())).await.is_err() {
                return;
            }
            last_seen = head;
            last_wake = Instant::now();
        }
        tokio::select! {
            msg = ws.recv() => match msg {
                Some(Ok(Message::Close(_))) | None => return,
                Some(Ok(Message::Ping(p))) => {
                    if ws.send(Message::Pong(p)).await.is_err() {
                        return;
                    }
                }
                Some(Err(_)) => return,
                _ => {}
            },
            _ = wait => {}
        }
    }
}

async fn create_group(State(st): State<AppState>, body: Bytes) -> Response {
    let Some(m) = decode_map(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(signing_pk) = read_key(&m, 0) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(cap) = read_key(&m, 1) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(hpke) = read_key(&m, 2) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    match host.create_group(
        signing_pk,
        FanoutTarget {
            delivery_capability: cap,
            home_hpke_public: hpke,
        },
    ) {
        Ok(created) => cbor_ok(cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(created.group_id.0.to_vec())),
            (1, Value::Bytes(created.cred.credential_id.to_vec())),
            (2, Value::Bytes(created.cred.credential_secret.to_vec())),
        ]))),
        Err(e) => map_err(e).into_response(),
    }
}

async fn group_append(State(st): State<AppState>, Path(id): Path<String>, body: Bytes) -> Response {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(m) = decode_map(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(credential_id) = read_key(&m, 0) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(credential_secret) = read_key(&m, 1) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let cred = MemberCred {
        credential_id,
        credential_secret,
    };
    let Ok(type_u) = read_uint(&m, 2) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if type_u > u8::MAX as u64 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(type_) = MessageType::from_u8(type_u as u8) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(body_bytes) = read_bytes(&m, 3) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    let appended = match host.append(group_id, &cred, type_, body_bytes) {
        Ok(a) => a,
        Err(e) => return map_err(e).into_response(),
    };
    drop(host);
    let mut home = st.home.lock().await;
    home.now = unix_now();
    for outer in appended.outers {
        let _ = home.enqueue(outer);
    }
    cbor_ok(cbor::encode(&Value::Map(vec![(
        0,
        Value::Uint(appended.seq),
    )])))
}

async fn group_invite(
    State(st): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> StatusCode {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(m) = decode_map(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(cred) = read_cred(&m) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(invite_bytes) = read_bytes(&m, 2) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(invite) = GroupInvite::decode(&invite_bytes) else {
        return StatusCode::BAD_REQUEST;
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    match host.store_invite(group_id, &cred, &invite) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

async fn group_accept(State(st): State<AppState>, Path(id): Path<String>, body: Bytes) -> Response {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(m) = decode_map(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(nonce) = read_key(&m, 0) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    match host.accept(group_id, nonce) {
        Ok(pending) => {
            let mut pairs = vec![
                (0, Value::Bytes(pending.pending_id.to_vec())),
                (1, Value::Bytes(pending.cred.credential_id.to_vec())),
                (2, Value::Bytes(pending.cred.credential_secret.to_vec())),
            ];
            if let Some(b) = pending.invitee_binding {
                pairs.push((3, Value::Bytes(b.to_vec())));
            }
            cbor_ok(cbor::encode(&Value::Map(pairs)))
        }
        Err(e) => map_err(e).into_response(),
    }
}

async fn group_admit(State(st): State<AppState>, Path(id): Path<String>, body: Bytes) -> Response {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(m) = decode_map(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(admitter) = read_cred(&m) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(admit_bytes) = read_bytes(&m, 2) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(admit) = GroupAdmit::decode(&admit_bytes) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(joiner_pk) = read_key(&m, 3) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(cap) = read_key(&m, 4) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(hpke) = read_key(&m, 5) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    match host.admit(
        group_id,
        &admitter,
        &admit,
        joiner_pk,
        FanoutTarget {
            delivery_capability: cap,
            home_hpke_public: hpke,
        },
    ) {
        Ok(cred_id) => cbor_ok(cbor::encode(&Value::Map(vec![(
            0,
            Value::Bytes(cred_id.to_vec()),
        )]))),
        Err(e) => map_err(e).into_response(),
    }
}

async fn group_fanout(
    State(st): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> StatusCode {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(m) = decode_map(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(cred) = read_cred(&m) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(cap) = read_key(&m, 2) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(hpke) = read_key(&m, 3) else {
        return StatusCode::BAD_REQUEST;
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    match host.refresh_fanout(
        group_id,
        &cred,
        FanoutTarget {
            delivery_capability: cap,
            home_hpke_public: hpke,
        },
    ) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

async fn group_upload_file(
    State(st): State<AppState>,
    Path((id, token)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(fetch_token) = parse_key_hex(&token) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(cred) = header_cred(&headers) else {
        return denied();
    };
    let mut host = st.groups.lock().await;
    host.now = unix_now();
    match host.upload_file(group_id, &cred, fetch_token, body.to_vec()) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => map_err(e),
    }
}

async fn group_fetch_file(
    State(st): State<AppState>,
    Path((id, token)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let Some(group_id) = parse_group_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(fetch_token) = parse_key_hex(&token) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(cred) = header_cred(&headers) else {
        return denied().into_response();
    };
    let host = st.groups.lock().await;
    match host.fetch_file(group_id, &cred, fetch_token) {
        Ok(blob) => ([(header::CONTENT_TYPE, "application/octet-stream")], blob).into_response(),
        Err(e) => map_err(e).into_response(),
    }
}

fn decode_map(body: &[u8]) -> Option<Vec<(u64, Value)>> {
    match cbor::decode(body) {
        Ok(Value::Map(m)) => Some(m),
        _ => None,
    }
}

fn read_key(m: &[(u64, Value)], k: u64) -> crate::Result<[u8; KEY_LEN]> {
    Ok(ids::copy_fixed(cbor::expect_bytes(cbor::map_get(m, k)?)?)?)
}

fn read_uint(m: &[(u64, Value)], k: u64) -> crate::Result<u64> {
    Ok(cbor::expect_uint(cbor::map_get(m, k)?)?)
}

fn read_bytes(m: &[(u64, Value)], k: u64) -> crate::Result<Vec<u8>> {
    Ok(cbor::expect_bytes(cbor::map_get(m, k)?)?.to_vec())
}

fn read_cred(m: &[(u64, Value)]) -> crate::Result<MemberCred> {
    Ok(MemberCred {
        credential_id: read_key(m, 0)?,
        credential_secret: read_key(m, 1)?,
    })
}

fn parse_group_id(id: &str) -> Option<GroupId> {
    parse_key_hex(id).map(GroupId)
}

fn parse_key_hex(s: &str) -> Option<[u8; KEY_LEN]> {
    ids::copy_fixed(&ids::from_hex(s).ok()?).ok()
}

fn owner_id(auth: &MailboxOwnerAuth) -> Option<IdentityId> {
    ids::copy_fixed(&auth.mailbox_hint).ok()
}

fn require_owner(headers: &HeaderMap) -> Result<(IdentityId, MailboxOwnerAuth), StatusCode> {
    let auth = header_owner(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let owner = owner_id(&auth).ok_or(StatusCode::BAD_REQUEST)?;
    Ok((owner, auth))
}

fn header_owner(headers: &HeaderMap) -> Option<MailboxOwnerAuth> {
    let raw = headers.get("nemo-owner")?.to_str().ok()?;
    let bytes = ids::from_hex(raw).ok()?;
    MailboxOwnerAuth::decode(&bytes).ok()
}

fn header_token(headers: &HeaderMap, name: &str) -> Option<[u8; KEY_LEN]> {
    let raw = headers.get(name)?.to_str().ok()?;
    parse_key_hex(raw)
}

fn header_cred(headers: &HeaderMap) -> Option<MemberCred> {
    Some(MemberCred {
        credential_id: header_token(headers, "nemo-cred-id")?,
        credential_secret: header_token(headers, "nemo-cred-secret")?,
    })
}
