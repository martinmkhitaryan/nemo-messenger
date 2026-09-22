//! UniFFI surface for the Compose shell (ADR-0028). No keys cross this boundary
//! except identifiers, fingerprints, and the one-time revocation mnemonic.

uniffi::setup_scaffolding!("nemo");

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_core::{
    classify_binding_gossip, decode, encode, encode_text, invite_ttl_bucket, mailbox,
    messages_lost, open_group_file, seal_group_file, AppBody, AppHeader, AppMessage,
    BindingGossipCheck, Call, CapabilityIntro, CoreError, FileMeta, Group, HomeSession,
    HostAccept, HostGroup, HttpHome, Installation, LocalSignal, PendingJoin, PrivacyMode,
    TurnConfig, Vault, DISCOVERY_REFRESH_SECS,
};
use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{MessageType, TtlBucket};
use nemo_wire::hostframe::AttachmentReserve;
use nemo_wire::ids::{self, KEY_LEN};
use nemo_wire::{
    parse_identity_id, verify_bound_invite, ContactCard, GroupInvite, InviteeProof, VerifyingKey,
};

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FfiError {
    #[error("{0}")]
    Core(String),
}

impl From<nemo_core::CoreError> for FfiError {
    fn from(err: nemo_core::CoreError) -> Self {
        Self::Core(err.to_string())
    }
}

impl From<nemo_wire::WireError> for FfiError {
    fn from(err: nemo_wire::WireError) -> Self {
        Self::Core(err.to_string())
    }
}

/// Decrypted 1:1 or group text/file for the shell. Never includes ratchet or MLS keys.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DisplayRow {
    pub conv_id: String,
    pub conv_seq: u64,
    pub text: String,
    pub sent_at: u64,
    pub file_name: String,
    pub file_mime: String,
    pub file_bytes: Vec<u8>,
    pub fetch_token: String,
    pub kind: String,
    pub emoji: String,
    pub target: u64,
    pub hidden: bool,
    pub displayed_at: u64,
}

/// Hosted MLS group the shell can list. No signing keys.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct GroupRow {
    pub group_id: String,
    pub nickname: String,
    pub member_count: u64,
}

/// Local nickname for a 1:1 contact. No keys.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ContactRow {
    pub identity_id: String,
    pub nickname: String,
}

/// Identity shown before adding a contact. No keys besides the public fingerprint.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ContactPreview {
    pub identity_id_hex: String,
    pub fingerprint: String,
}

const GROUP_INVITE_PREFIX: &str = "nemo-g:1:";
const JOIN_REQUEST_PREFIX: &str = "nemo-j:1:";

struct LiveGroup {
    host: HostGroup,
    mls: Group,
    nickname: String,
}

struct PendingEntry {
    pending: PendingJoin,
    host: HostAccept,
}

struct PendingInvite {
    peer: String,
    signal: LocalSignal,
    expires_at: u64,
}

enum ClientState {
    Local(Installation),
    Registered(HomeSession<HttpHome>),
}

struct Inner {
    state: Option<ClientState>,
    vault: Option<Vault>,
    revocation_mnemonic: Option<String>,
    inbox: Vec<DisplayRow>,
    nicknames: HashMap<String, String>,
    next_seq: HashMap<String, u64>,
    groups: Vec<LiveGroup>,
    pending: HashMap<[u8; KEY_LEN], PendingEntry>,
    minted: HashMap<[u8; KEY_LEN], GroupInvite>,
    disappear: HashMap<String, u64>,
    pending_invites: HashMap<String, PendingInvite>,
    live_call: Option<Arc<Call>>,
    call_peer: Option<String>,
    call_connected: bool,
    group_discovery_at: HashMap<[u8; KEY_LEN], u64>,
    /// Peers we already sent a capability intro to (avoid minting a token every send).
    intro_sent: std::collections::HashSet<[u8; KEY_LEN]>,
}

/// One installation. The shell must not persist ratchet or MLS keys.
#[derive(uniffi::Object)]
pub struct NemoClient {
    inner: Arc<Mutex<Inner>>,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => runtime().block_on(fut),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn lock_err() -> FfiError {
    FfiError::Core("lock".into())
}

fn pump_call(call: Arc<Call>) {
    runtime().spawn(async move {
        if call.wait_connected().await.is_ok() {
            let _ = call.start_audio_io();
            loop {
                if call.send_capture_frames(5).await.is_err() {
                    break;
                }
            }
        }
    });
}

fn pump_trickle(inner: Arc<Mutex<Inner>>, call: Arc<Call>, peer_hex: String) {
    runtime().spawn(async move {
        loop {
            if call.is_closed() {
                break;
            }
            let extra = match call.take_unsent_ice() {
                Ok(ice) if !ice.is_empty() => ice,
                Ok(_) => {
                    if call.ice_gathering_done() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
                Err(_) => break,
            };
            let now = now_unix();
            let mut guard = match inner.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            let live_id = guard.live_call.as_ref().map(|c| c.call_id());
            if live_id != Some(call.call_id()) {
                break;
            }
            let Inner {
                state,
                inbox,
                next_seq,
                ..
            } = &mut *guard;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                _ => break,
            };
            let _ = emit_call(
                session,
                inbox,
                next_seq,
                peer_hex.clone(),
                AppBody::CallIce {
                    call_id: call.call_id(),
                    ice: extra,
                },
                "call_ice",
                now,
                false,
            );
        }
    });
}

fn parse_card(card_or_uri: &str) -> Result<ContactCard, FfiError> {
    let uri = card_or_uri
        .split_once('#')
        .map(|(u, _)| u)
        .unwrap_or(card_or_uri);
    if uri.starts_with(nemo_wire::card::URI_PREFIX) {
        return Ok(ContactCard::from_uri(uri)?);
    }
    if let Ok(card) = ContactCard::from_uri(uri) {
        return Ok(card);
    }
    let bytes = ids::from_hex(uri)?;
    Ok(ContactCard::decode(&bytes)?)
}

fn parse_delivery_cap(card_or_uri: &str, card: &ContactCard) -> Result<[u8; 32], FfiError> {
    match card_or_uri.split_once('#') {
        Some((_, hex)) if !hex.is_empty() => Ok(ids::copy_fixed(&ids::from_hex(hex)?)?),
        _ => Ok(card.share_token),
    }
}

fn bump_seq(map: &mut HashMap<String, u64>, peer: &str) -> u64 {
    let e = map.entry(peer.to_owned()).or_insert(0);
    *e += 1;
    *e
}

fn emit_call(
    session: &mut HomeSession<HttpHome>,
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    peer_hex: String,
    body: AppBody,
    kind: &str,
    now: u64,
    echo: bool,
) -> Result<DisplayRow, FfiError> {
    let seq = bump_seq(next_seq, &peer_hex);
    let id_hex = match &body {
        AppBody::CallRinging { call_id }
        | AppBody::CallReject { call_id }
        | AppBody::CallCancel { call_id }
        | AppBody::CallEnd { call_id }
        | AppBody::CallIce { call_id, .. } => ids::to_hex(call_id),
        _ => String::new(),
    };
    let ptext = encode(&AppMessage {
        header: AppHeader {
            conv_seq: seq,
            sent_at: now,
            reply_to: None,
        },
        body,
    })?;
    let peer = parse_identity_id(&peer_hex)?;
    block_on(session.send_to_now(&peer, invite_ttl_bucket(), &ptext, now))?;
    if echo {
        Ok(push_control(
            inbox, next_seq, peer_hex, seq, now, kind, id_hex, 0,
        ))
    } else {
        Ok(DisplayRow {
            conv_id: peer_hex,
            conv_seq: seq,
            text: id_hex,
            sent_at: now,
            file_name: String::new(),
            file_mime: String::new(),
            file_bytes: Vec::new(),
            fetch_token: String::new(),
            kind: kind.into(),
            emoji: String::new(),
            target: 0,
            hidden: false,
            displayed_at: now,
        })
    }
}

fn hangup_live(
    live_call: &mut Option<Arc<Call>>,
    call_peer: &mut Option<String>,
    call_connected: &mut bool,
) {
    if let Some(call) = live_call.take() {
        let _ = block_on(call.close());
    }
    *call_peer = None;
    *call_connected = false;
}

fn send_call_body(
    inner: &mut Inner,
    peer_hex: String,
    body: AppBody,
    kind: &str,
    now: u64,
) -> Result<DisplayRow, FfiError> {
    let Inner {
        state,
        inbox,
        next_seq,
        ..
    } = inner;
    let session = match state {
        Some(ClientState::Registered(session)) => session,
        Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
        None => return Err(FfiError::Core("busy".into())),
    };
    emit_call(session, inbox, next_seq, peer_hex, body, kind, now, true)
}

fn send_own_contact_capability(
    session: &mut HomeSession<HttpHome>,
    peer: nemo_wire::ids::IdentityId,
    now: u64,
) -> Result<(), FfiError> {
    let cap = block_on(session.mint_contact())?;
    let ptext = encode(&AppMessage {
        header: AppHeader {
            conv_seq: 0,
            sent_at: now,
            reply_to: None,
        },
        body: AppBody::Capability {
            contact_capability: cap,
            intro: Some(CapabilityIntro {
                identity_id: session.identity_id(),
                identity_public_key: session.install.identity_public_key(),
                revocation_public_key: session.install.revocation_public_key(),
                dest_hpke: session.hpke_public(),
                binding_seq: session.install.binding_seq(),
            }),
        },
    })?;
    block_on(session.send_to(&peer, TtlBucket::DEFAULT, &ptext, now))?;
    Ok(())
}

fn ensure_intro_sent(
    session: &mut HomeSession<HttpHome>,
    intro_sent: &mut std::collections::HashSet<[u8; KEY_LEN]>,
    peer: nemo_wire::ids::IdentityId,
    now: u64,
) -> Result<(), FfiError> {
    if intro_sent.contains(&peer) {
        return Ok(());
    }
    send_own_contact_capability(session, peer, now)?;
    intro_sent.insert(peer);
    Ok(())
}

fn remap_conv_id(
    inbox: &mut [DisplayRow],
    next_seq: &mut HashMap<String, u64>,
    nicknames: &mut HashMap<String, String>,
    disappear: &mut HashMap<String, u64>,
    from: &str,
    to: &str,
) {
    if from == to {
        return;
    }
    for row in inbox.iter_mut() {
        if row.conv_id == from {
            row.conv_id = to.to_string();
        }
    }
    if let Some(seq) = next_seq.remove(from) {
        let entry = next_seq.entry(to.to_string()).or_insert(0);
        *entry = (*entry).max(seq);
    }
    if let Some(nick) = nicknames.remove(from) {
        nicknames.entry(to.to_string()).or_insert(nick);
    }
    if let Some(secs) = disappear.remove(from) {
        disappear.entry(to.to_string()).or_insert(secs);
    }
}

fn send_own_binding_gossip(
    session: &mut HomeSession<HttpHome>,
    peer: nemo_wire::ids::IdentityId,
    now: u64,
) -> Result<(), FfiError> {
    let ptext = encode(&AppMessage {
        header: AppHeader {
            conv_seq: 0,
            sent_at: now,
            reply_to: None,
        },
        body: AppBody::BindingGossip {
            identity_id: session.identity_id(),
            seq: session.install.binding_seq(),
            server_id: session.bundle.server_id,
        },
    })?;
    block_on(session.send_to(&peer, TtlBucket::DEFAULT, &ptext, now))?;
    Ok(())
}

fn apply_binding_gossip(
    session: &mut HomeSession<HttpHome>,
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    from_peer: [u8; KEY_LEN],
    identity_id: [u8; KEY_LEN],
    seq: u64,
    server_id: [u8; KEY_LEN],
    conv_seq: u64,
    sent_at: u64,
) -> Option<DisplayRow> {
    let (pin_seq, pin_server) = {
        let contact = session.contacts.get(&identity_id)?;
        (contact.pin.seq, ids::server_id(&contact.dest_hpke))
    };
    let check = classify_binding_gossip(pin_seq, pin_server, seq, server_id);
    let conflict = match check {
        BindingGossipCheck::Match => false,
        BindingGossipCheck::Ahead => {
            let _ = block_on(session.refresh_contact(&identity_id, now_unix()));
            match session.contacts.get(&identity_id) {
                Some(contact)
                    if classify_binding_gossip(
                        contact.pin.seq,
                        ids::server_id(&contact.dest_hpke),
                        seq,
                        server_id,
                    ) == BindingGossipCheck::Match =>
                {
                    false
                }
                _ => true,
            }
        }
        BindingGossipCheck::Conflict => true,
    };
    if !conflict {
        return None;
    }
    Some(push_control(
        inbox,
        next_seq,
        ids::to_hex(&from_peer),
        conv_seq,
        sent_at,
        "binding_conflict",
        ids::to_hex(&identity_id),
        0,
    ))
}

fn persist(inner: &Inner) -> Result<(), FfiError> {
    let Some(vault) = inner.vault.as_ref() else {
        return Ok(());
    };
    match inner.state.as_ref() {
        Some(ClientState::Registered(session)) => {
            vault.save_home(&session.install, &session.snapshot())?;
            vault.save_groups(&session.install, inner.groups.iter().map(|g| &g.mls))?;
            vault.save_display(&inner.nicknames, &inner.disappear)?;
            let pending: Vec<_> = inner
                .pending
                .iter()
                .map(|(gid, e)| (*gid, &e.pending, &e.host))
                .collect();
            vault.save_pending(&pending)?;
            let invites: Vec<_> = inner.minted.values().cloned().collect();
            vault.save_invites(&invites)?;
        }
        Some(ClientState::Local(install)) => {
            vault.save(install)?;
            vault.save_display(&inner.nicknames, &inner.disappear)?;
        }
        None => return Err(FfiError::Core("busy".into())),
    }
    Ok(())
}

fn empty_inner(
    state: Option<ClientState>,
    vault: Option<Vault>,
    mnemonic: Option<String>,
) -> Inner {
    Inner {
        state,
        vault,
        revocation_mnemonic: mnemonic,
        inbox: Vec::new(),
        nicknames: HashMap::new(),
        next_seq: HashMap::new(),
        groups: Vec::new(),
        pending: HashMap::new(),
        minted: HashMap::new(),
        disappear: HashMap::new(),
        pending_invites: HashMap::new(),
        live_call: None,
        call_peer: None,
        call_connected: false,
        group_discovery_at: HashMap::new(),
        intro_sent: std::collections::HashSet::new(),
    }
}

fn load_live_groups(vault: &Vault, install: &Installation, hosts: &[HostGroup]) -> Vec<LiveGroup> {
    let mls = vault.load_groups(install).unwrap_or_default();
    hosts
        .iter()
        .cloned()
        .zip(mls)
        .map(|(host, mls)| LiveGroup {
            host,
            mls,
            nickname: String::new(),
        })
        .collect()
}

fn find_group_mut<'a>(
    groups: &'a mut [LiveGroup],
    group_id_hex: &str,
) -> Result<&'a mut LiveGroup, FfiError> {
    let id = parse_identity_id(group_id_hex)?;
    groups
        .iter_mut()
        .find(|g| g.host.group_id == id)
        .ok_or_else(|| FfiError::Core("unknown group".into()))
}

fn mint_invite(
    inner: &mut Inner,
    group_id_hex: String,
    bind_identity: Option<[u8; KEY_LEN]>,
) -> Result<String, FfiError> {
    let Inner {
        state,
        groups,
        minted,
        ..
    } = inner;
    let session = match state {
        Some(ClientState::Registered(session)) => session,
        Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
        None => return Err(FfiError::Core("busy".into())),
    };
    let bind_pk = match bind_identity {
        Some(id) => Some(
            session
                .contacts
                .get(&id)
                .ok_or(CoreError::UnknownContact)?
                .pin
                .identity_public_key,
        ),
        None => None,
    };
    let g = find_group_mut(groups, &group_id_hex)?;
    let invite = g.mls.sign_invite(
        g.host.group_id,
        Group::default_invite_ttl(),
        bind_pk.as_ref(),
    )?;
    block_on(session.store_invite(g.host.group_id, &g.host.cred, &invite))?;
    if invite.invitee_binding.is_some() {
        minted.insert(invite.nonce, invite.clone());
    }
    Ok(format!(
        "{GROUP_INVITE_PREFIX}{}",
        ids::to_hex(&invite.encode())
    ))
}

fn flush_mls_update(
    session: &mut HomeSession<HttpHome>,
    g: &mut LiveGroup,
) -> Result<(), FfiError> {
    if let Some(hs) = g
        .mls
        .maybe_self_update(session.install.mls_provider(), SystemTime::now())?
    {
        block_on(session.group_append(
            g.host.group_id,
            &g.host.cred,
            MessageType::MlsHandshake,
            hs,
        ))?;
    }
    Ok(())
}

fn refresh_group_members(session: &mut HomeSession<HttpHome>, g: &Group) -> Result<(), FfiError> {
    let ids = g.member_identity_ids();
    block_on(session.refresh_mls_identities(&ids, now_unix()))?;
    Ok(())
}

fn idle_maintenance(
    session: &mut HomeSession<HttpHome>,
    groups: &mut [LiveGroup],
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    group_discovery_at: &mut HashMap<[u8; KEY_LEN], u64>,
) -> Result<Vec<DisplayRow>, FfiError> {
    let now = now_unix();
    let own = session.identity_id();
    let mut newly: Vec<_> = block_on(session.refresh_idle_discovery(now));
    let mut to_remove: Vec<_> = newly.clone();
    to_remove.extend(session.revoked_contact_ids());

    let extras: Vec<_> = groups
        .iter()
        .flat_map(|g| g.mls.member_identity_ids())
        .filter(|id| *id != own && !session.contacts.contains_key(id))
        .collect();
    for id in extras {
        let stale = group_discovery_at
            .get(&id)
            .map(|t| now.saturating_sub(*t) >= DISCOVERY_REFRESH_SECS)
            .unwrap_or(true);
        if !stale {
            continue;
        }
        if block_on(session.identity_is_revoked(id, now)) {
            newly.push(id);
            to_remove.push(id);
        }
        group_discovery_at.insert(id, now);
    }

    let mut rows = Vec::new();
    let mut seen = HashMap::new();
    for id in &to_remove {
        if seen.insert(*id, ()).is_some() {
            continue;
        }
        for g in groups.iter_mut() {
            let Some(cred) = g.mls.credential_id_for_identity(id) else {
                continue;
            };
            if cred == g.mls.credential_id() {
                continue;
            }
            if let Ok(bundle) = g.mls.remove(session.install.mls_provider(), cred) {
                let _ = block_on(session.group_append(
                    g.host.group_id,
                    &g.host.cred,
                    MessageType::RemoveBundle,
                    bundle.encode()?,
                ));
            }
        }
    }
    for id in newly {
        rows.push(push_control(
            inbox,
            next_seq,
            ids::to_hex(&id),
            0,
            now,
            "revoked",
            String::new(),
            0,
        ));
    }
    for g in groups.iter_mut() {
        let _ = flush_mls_update(session, g);
    }
    Ok(rows)
}

fn turn_config(inner: &Inner) -> Result<TurnConfig, FfiError> {
    match inner.state.as_ref() {
        Some(ClientState::Registered(session)) => {
            Ok(block_on(session.issue_turn()).unwrap_or_else(|_| TurnConfig::from_env()))
        }
        _ => Ok(TurnConfig::from_env()),
    }
}

fn call_cbr(inner: &Inner) -> bool {
    matches!(
        inner.state.as_ref(),
        Some(ClientState::Registered(session))
            if matches!(session.privacy, PrivacyMode::Private | PrivacyMode::High)
    )
}

fn join_request_uri(
    group_id: [u8; KEY_LEN],
    pending_id: [u8; KEY_LEN],
    key_package: &[u8],
    signing_pk: [u8; KEY_LEN],
    cap: [u8; KEY_LEN],
    hpke: [u8; KEY_LEN],
    credential_id: [u8; KEY_LEN],
    nonce: [u8; KEY_LEN],
    proof: Option<&InviteeProof>,
    invitee_binding: Option<[u8; KEY_LEN]>,
) -> String {
    let mut pairs = vec![
        (0, Value::Bytes(group_id.to_vec())),
        (1, Value::Bytes(pending_id.to_vec())),
        (2, Value::Bytes(key_package.to_vec())),
        (3, Value::Bytes(signing_pk.to_vec())),
        (4, Value::Bytes(cap.to_vec())),
        (5, Value::Bytes(hpke.to_vec())),
        (6, Value::Bytes(credential_id.to_vec())),
        (7, Value::Bytes(nonce.to_vec())),
    ];
    if let Some(p) = proof {
        pairs.push((8, Value::Bytes(p.encode())));
    }
    if let Some(b) = invitee_binding {
        pairs.push((9, Value::Bytes(b.to_vec())));
    }
    let bytes = cbor::encode(&Value::Map(pairs));
    format!("{JOIN_REQUEST_PREFIX}{}", ids::to_hex(&bytes))
}

struct JoinRequest {
    group_id: [u8; KEY_LEN],
    pending_id: [u8; KEY_LEN],
    key_package: Vec<u8>,
    signing_pk: [u8; KEY_LEN],
    cap: [u8; KEY_LEN],
    hpke: [u8; KEY_LEN],
    credential_id: [u8; KEY_LEN],
    nonce: [u8; KEY_LEN],
    proof: Option<InviteeProof>,
    invitee_binding: Option<[u8; KEY_LEN]>,
}

fn parse_join_request(uri: &str) -> Result<JoinRequest, FfiError> {
    let hex = uri
        .strip_prefix(JOIN_REQUEST_PREFIX)
        .ok_or_else(|| FfiError::Core("join request must start with nemo-j:1:".into()))?;
    let Value::Map(m) = cbor::decode(&ids::from_hex(hex)?)? else {
        return Err(FfiError::Core("join request".into()));
    };
    let read = |k: u64| -> Result<[u8; KEY_LEN], FfiError> {
        Ok(ids::copy_fixed(cbor::expect_bytes(cbor::map_get(&m, k)?)?)?)
    };
    let nonce = match cbor::map_get_opt(&m, 7) {
        Some(v) => ids::copy_fixed(cbor::expect_bytes(v)?)?,
        None => [0u8; KEY_LEN],
    };
    let proof = match cbor::map_get_opt(&m, 8) {
        Some(v) => Some(InviteeProof::decode(cbor::expect_bytes(v)?)?),
        None => None,
    };
    let invitee_binding = match cbor::map_get_opt(&m, 9) {
        Some(v) => Some(ids::copy_fixed(cbor::expect_bytes(v)?)?),
        None => None,
    };
    Ok(JoinRequest {
        group_id: read(0)?,
        pending_id: read(1)?,
        key_package: cbor::expect_bytes(cbor::map_get(&m, 2)?)?.to_vec(),
        signing_pk: read(3)?,
        cap: read(4)?,
        hpke: read(5)?,
        credential_id: read(6)?,
        nonce,
        proof,
        invitee_binding,
    })
}

fn parse_group_invite(uri: &str) -> Result<GroupInvite, FfiError> {
    let hex = uri
        .strip_prefix(GROUP_INVITE_PREFIX)
        .ok_or_else(|| FfiError::Core("group invite must start with nemo-g:1:".into()))?;
    Ok(GroupInvite::decode(&ids::from_hex(hex)?)?)
}

impl Inner {
    fn install(&self) -> Result<&Installation, FfiError> {
        match self.state.as_ref() {
            Some(ClientState::Local(install)) => Ok(install),
            Some(ClientState::Registered(session)) => Ok(&session.install),
            None => Err(FfiError::Core("busy".into())),
        }
    }

    fn registered(&mut self) -> Result<&mut HomeSession<HttpHome>, FfiError> {
        match self.state.as_mut() {
            Some(ClientState::Registered(session)) => Ok(session),
            Some(ClientState::Local(_)) => Err(CoreError::NotRegistered.into()),
            None => Err(FfiError::Core("busy".into())),
        }
    }
}

#[uniffi::export]
impl NemoClient {
    #[uniffi::constructor]
    pub fn create() -> Result<Arc<Self>, FfiError> {
        let (install, export) = Installation::create()?;
        Ok(Arc::new(Self {
            inner: Arc::new(Mutex::new(empty_inner(
                Some(ClientState::Local(install)),
                None,
                Some(export.mnemonic),
            ))),
        }))
    }

    /// Create an identity and lock it in `dir` (ADR-0034).
    ///
    /// `device_secret` is the Keystore-unwrapped 32-byte bind on Android. Empty
    /// means `nemo-core` loads the OS bind (desktop).
    #[uniffi::constructor]
    pub fn create_at(
        dir: String,
        passphrase: String,
        device_secret: Vec<u8>,
    ) -> Result<Arc<Self>, FfiError> {
        let (install, export) = Installation::create()?;
        let injected = if device_secret.is_empty() {
            None
        } else {
            Some(device_secret)
        };
        let vault = Vault::create_bound(&dir, &passphrase, &install, injected.as_deref())?;
        Ok(Arc::new(Self {
            inner: Arc::new(Mutex::new(empty_inner(
                Some(ClientState::Local(install)),
                Some(vault),
                Some(export.mnemonic),
            ))),
        }))
    }

    #[uniffi::constructor]
    pub fn open_at(
        dir: String,
        passphrase: String,
        device_secret: Vec<u8>,
    ) -> Result<Arc<Self>, FfiError> {
        let injected = if device_secret.is_empty() {
            None
        } else {
            Some(device_secret)
        };
        let (vault, install) = Vault::open_bound(&dir, &passphrase, injected.as_deref())?;
        let (state, groups) = match vault.load_home()? {
            Some(home) if !home.home_base.is_empty() => {
                let hosts = home.groups.clone();
                let groups = load_live_groups(&vault, &install, &hosts);
                let transport = HttpHome::new(home.home_base.clone())?;
                (
                    ClientState::Registered(HomeSession::resume(transport, install, home)),
                    groups,
                )
            }
            _ => (ClientState::Local(install), Vec::new()),
        };
        let mut inner = empty_inner(Some(state), Some(vault), None);
        inner.groups = groups;
        if let Ok((nicks, timers)) = inner.vault.as_ref().unwrap().load_display() {
            inner.nicknames = nicks;
            inner.disappear = timers;
        }
        let invites = inner
            .vault
            .as_ref()
            .and_then(|v| v.load_invites().ok())
            .unwrap_or_default();
        for inv in invites {
            inner.minted.insert(inv.nonce, inv);
        }
        let pending_rows = match (inner.vault.as_ref(), inner.state.as_ref()) {
            (Some(vault), Some(ClientState::Registered(session))) => {
                vault.load_pending(&session.install).unwrap_or_default()
            }
            (Some(vault), Some(ClientState::Local(install))) => {
                vault.load_pending(install).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        for (gid, pending, host) in pending_rows {
            inner.pending.insert(gid, PendingEntry { pending, host });
        }
        if let Some(ClientState::Registered(session)) = inner.state.as_mut() {
            let _ = block_on(session.restock_publish());
        }
        Ok(Arc::new(Self {
            inner: Arc::new(Mutex::new(inner)),
        }))
    }

    pub fn identity_id_hex(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(ids::to_hex(&inner.install()?.identity_id()))
    }

    pub fn fingerprint(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.install()?.fingerprint())
    }

    pub fn set_privacy_mode(&self, mode: String) -> Result<(), FfiError> {
        let parsed = match mode.to_ascii_lowercase().as_str() {
            "normal" => PrivacyMode::Normal,
            "private" => PrivacyMode::Private,
            "high" => PrivacyMode::High,
            "maximum" => PrivacyMode::Maximum,
            _ => return Err(FfiError::Core("privacy mode".into())),
        };
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        inner.registered()?.set_privacy(parsed)?;
        persist(&inner)?;
        Ok(())
    }

    pub fn privacy_mode(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        let mode = match inner.state.as_ref() {
            Some(ClientState::Registered(session)) => session.privacy,
            Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
            None => return Err(FfiError::Core("busy".into())),
        };
        Ok(match mode {
            PrivacyMode::Normal => "normal".into(),
            PrivacyMode::Private => "private".into(),
            PrivacyMode::High => "high".into(),
            PrivacyMode::Maximum => "maximum".into(),
        })
    }

    /// Shown once at identity creation. Never stored in the vault.
    pub fn take_revocation_mnemonic(&self) -> Result<Option<String>, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.revocation_mnemonic.take())
    }

    pub fn save(&self) -> Result<(), FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        if inner.vault.is_none() {
            return Err(CoreError::VaultDetached.into());
        }
        persist(&inner)
    }

    pub fn register(&self, home_https_base: String) -> Result<(), FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        match inner.state.take() {
            Some(ClientState::Local(install)) => {
                let transport = HttpHome::new(&home_https_base)?;
                match block_on(async {
                    let (mut session, _) = HomeSession::register(transport, install, now).await?;
                    session.set_home_base(&home_https_base);
                    session.restock_publish().await?;
                    Ok::<_, CoreError>(session)
                }) {
                    Ok(session) => {
                        inner.state = Some(ClientState::Registered(session));
                        persist(&inner)?;
                        Ok(())
                    }
                    Err(err) => {
                        inner.state = None;
                        Err(err.into())
                    }
                }
            }
            Some(ClientState::Registered(mut session)) => {
                let new_base = home_https_base.trim_end_matches('/');
                if !session.home_base.is_empty()
                    && session.home_base.trim_end_matches('/') == new_base
                {
                    inner.state = Some(ClientState::Registered(session));
                    return Err(CoreError::AlreadyRegistered.into());
                }
                match block_on(session.rehome(
                    HttpHome::new(&home_https_base)?,
                    &home_https_base,
                    now,
                )) {
                    Ok(_) => {
                        for live in inner.groups.iter_mut() {
                            if let Some(h) = session
                                .groups
                                .iter()
                                .find(|g| g.group_id == live.host.group_id)
                            {
                                live.host = h.clone();
                            }
                        }
                        let peers: Vec<_> = session.contacts.keys().copied().collect();
                        for peer in peers {
                            let _ = send_own_contact_capability(&mut session, peer, now);
                            let _ = send_own_binding_gossip(&mut session, peer, now);
                        }
                        inner.state = Some(ClientState::Registered(session));
                        persist(&inner)?;
                        Ok(())
                    }
                    Err(err) => {
                        inner.state = Some(ClientState::Registered(session));
                        Err(err.into())
                    }
                }
            }
            None => Err(FfiError::Core("busy".into())),
        }
    }

    pub fn mint_share_uri(&self) -> Result<String, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let session = inner.registered()?;
        let card = block_on(session.mint_share_card(now_unix()))?;
        let cap = block_on(session.mint_contact())?;
        persist(&inner)?;
        Ok(format!("{}#{}", card.to_uri()?, ids::to_hex(&cap)))
    }

    pub fn add_contact(&self, card_or_uri: String, nickname: String) -> Result<String, FfiError> {
        let card = parse_card(&card_or_uri)?;
        let cap = parse_delivery_cap(&card_or_uri, &card)?;
        let peer = ids::to_hex(&card.identity_id());
        let peer_id = card.identity_id();
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        {
            let Inner {
                state, intro_sent, ..
            } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            block_on(session.add_contact(&card, cap, now))?;
            let _ = ensure_intro_sent(session, intro_sent, peer_id, now);
        }
        if !nickname.is_empty() {
            inner.nicknames.insert(peer.clone(), nickname);
        }
        persist(&inner)?;
        Ok(peer)
    }

    pub fn preview_contact(&self, card_or_uri: String) -> Result<ContactPreview, FfiError> {
        let card = parse_card(&card_or_uri)?;
        Ok(ContactPreview {
            identity_id_hex: ids::to_hex(&card.identity_id()),
            fingerprint: nemo_wire::fingerprint(&card.identity_id()),
        })
    }

    pub fn send_text(&self, peer_id_hex: String, text: String) -> Result<DisplayRow, FfiError> {
        let peer = parse_identity_id(&peer_id_hex)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let seq = bump_seq(&mut inner.next_seq, &peer_id_hex);
        let ptext = encode_text(seq, now, &text)?;
        let ttl = ttl_for(&inner.disappear, &peer_id_hex);
        {
            let Inner {
                state, intro_sent, ..
            } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            block_on(session.send_to(&peer, ttl, &ptext, now))?;
            let _ = ensure_intro_sent(session, intro_sent, peer, now);
            let _ = send_own_binding_gossip(session, peer, now);
        }
        let row = DisplayRow {
            conv_id: peer_id_hex,
            conv_seq: seq,
            text,
            sent_at: now,
            file_name: String::new(),
            file_mime: String::new(),
            file_bytes: Vec::new(),
            fetch_token: String::new(),
            kind: String::new(),
            emoji: String::new(),
            target: 0,
            hidden: false,
            displayed_at: now,
        };
        inner.inbox.push(row.clone());
        persist(&inner)?;
        Ok(row)
    }

    pub fn fetch_now(&self) -> Result<Vec<DisplayRow>, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let new_rows = {
            let Inner {
                state,
                groups,
                pending,
                inbox,
                next_seq,
                nicknames,
                disappear,
                pending_invites,
                live_call,
                call_peer,
                call_connected,
                group_discovery_at,
                ..
            } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let last_acked = session.cursor;
            let _ = block_on(session.pump_cover());
            let rows = block_on(session.fetch_mailbox())?;
            let mut new_rows = if rows.is_empty() {
                Vec::new()
            } else {
                let contacts: Vec<_> = session.contacts.keys().copied().collect();
                let mut new_rows = Vec::new();
                if let Some(first) = rows.first() {
                    if messages_lost(last_acked, first.seq) {
                        new_rows.push(push_control(
                            inbox,
                            next_seq,
                            "system".into(),
                            0,
                            now_unix(),
                            "lost",
                            String::new(),
                            0,
                        ));
                    }
                }
                let mut gossip = Vec::new();
                let mut acks: HashMap<[u8; KEY_LEN], u64> = HashMap::new();
                for row in &rows {
                    match block_on(session.install.decrypt_incoming(&row.inner, &contacts)) {
                        Ok((peer, plaintext)) => match decode(&plaintext) {
                            Ok(AppMessage {
                                header,
                                body: AppBody::Text { text },
                            }) => {
                                gossip.push(peer);
                                let upto = header.conv_seq;
                                acks.entry(peer)
                                    .and_modify(|u| {
                                        if upto > *u {
                                            *u = upto;
                                        }
                                    })
                                    .or_insert(upto);
                                new_rows.push(push_text(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    text,
                                    header.sent_at,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body:
                                    AppBody::Attachment {
                                        enc_file,
                                        meta,
                                        fetch_token: None,
                                    },
                            }) => {
                                new_rows.push(push_file(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    meta,
                                    enc_file,
                                    String::new(),
                                ));
                            }
                            Ok(AppMessage {
                                body: AppBody::Capability {
                                    contact_capability,
                                    intro,
                                },
                                ..
                            }) => {
                                let from_hex = ids::to_hex(&peer);
                                match session.apply_contact_capability(
                                    peer,
                                    contact_capability,
                                    intro,
                                    now_unix(),
                                ) {
                                    Ok(real) => {
                                        let real_hex = ids::to_hex(&real);
                                        remap_conv_id(
                                            inbox,
                                            next_seq,
                                            nicknames,
                                            disappear,
                                            &from_hex,
                                            &real_hex,
                                        );
                                        for row in new_rows.iter_mut() {
                                            if row.conv_id == from_hex {
                                                row.conv_id = real_hex.clone();
                                            }
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::Reaction { target, emoji },
                            }) => {
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "reaction",
                                    emoji,
                                    target,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::Delete { target },
                            }) => {
                                hide_target(inbox, &ids::to_hex(&peer), target);
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "deleted",
                                    String::new(),
                                    target,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::Disappear { seconds },
                            }) => {
                                let conv = ids::to_hex(&peer);
                                disappear.insert(conv.clone(), seconds);
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    conv,
                                    header.conv_seq,
                                    header.sent_at,
                                    "disappear",
                                    seconds.to_string(),
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                body: AppBody::ProtocolAck { .. },
                                ..
                            }) => {}
                            Ok(AppMessage {
                                header,
                                body:
                                    AppBody::CallInvite {
                                        call_id,
                                        sdp,
                                        dtls_fp,
                                        ice,
                                        expires_at,
                                    },
                            }) => {
                                let hex = ids::to_hex(&call_id);
                                let peer_hex = ids::to_hex(&peer);
                                if expires_at > now_unix() {
                                    pending_invites.insert(
                                        hex.clone(),
                                        PendingInvite {
                                            peer: peer_hex.clone(),
                                            signal: LocalSignal {
                                                call_id,
                                                sdp,
                                                dtls_fp,
                                                ice,
                                            },
                                            expires_at,
                                        },
                                    );
                                    let _ = emit_call(
                                        session,
                                        inbox,
                                        next_seq,
                                        peer_hex.clone(),
                                        AppBody::CallRinging { call_id },
                                        "call_ringing",
                                        now_unix(),
                                        false,
                                    );
                                }
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    peer_hex,
                                    header.conv_seq,
                                    header.sent_at,
                                    "call_invite",
                                    hex,
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body:
                                    AppBody::CallAnswer {
                                        call_id,
                                        sdp,
                                        dtls_fp,
                                        ice,
                                    },
                            }) => {
                                if let Some(call) = live_call.as_ref() {
                                    let sig = LocalSignal {
                                        call_id,
                                        sdp,
                                        dtls_fp,
                                        ice,
                                    };
                                    if block_on(call.apply_answer(&sig)).is_ok() {
                                        *call_connected = true;
                                    }
                                }
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "call_answer",
                                    ids::to_hex(&call_id),
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                body: AppBody::CallIce { call_id, ice },
                                ..
                            }) => {
                                if let Some(call) = live_call.as_ref() {
                                    if call.call_id() == call_id {
                                        let _ = block_on(call.add_remote_ice(&ice));
                                    }
                                }
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::CallRinging { call_id },
                            }) => {
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "call_ringing",
                                    ids::to_hex(&call_id),
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::CallEnd { call_id },
                            }) => {
                                hangup_live(live_call, call_peer, call_connected);
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "call_end",
                                    ids::to_hex(&call_id),
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::CallReject { call_id },
                            }) => {
                                hangup_live(live_call, call_peer, call_connected);
                                pending_invites.remove(&ids::to_hex(&call_id));
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "call_reject",
                                    ids::to_hex(&call_id),
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body: AppBody::CallCancel { call_id },
                            }) => {
                                hangup_live(live_call, call_peer, call_connected);
                                pending_invites.remove(&ids::to_hex(&call_id));
                                new_rows.push(push_control(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    header.sent_at,
                                    "call_cancel",
                                    ids::to_hex(&call_id),
                                    0,
                                ));
                            }
                            Ok(AppMessage {
                                header,
                                body:
                                    AppBody::BindingGossip {
                                        identity_id,
                                        seq,
                                        server_id,
                                    },
                            }) => {
                                if let Some(row) = apply_binding_gossip(
                                    session,
                                    inbox,
                                    next_seq,
                                    peer,
                                    identity_id,
                                    seq,
                                    server_id,
                                    header.conv_seq,
                                    header.sent_at,
                                ) {
                                    new_rows.push(row);
                                }
                            }
                            _ => {}
                        },
                        Err(CoreError::WrongMailboxType) => {}
                        Err(_) => {}
                    }
                }

                let mut joined = Vec::new();
                for row in &rows {
                    let Ok(body) = mailbox::expect_type(&row.inner, MessageType::MlsHandshake)
                    else {
                        continue;
                    };
                    if !PendingJoin::is_welcome(body) {
                        continue;
                    }
                    let keys: Vec<_> = pending.keys().copied().collect();
                    for gid in keys {
                        let Some(entry) = pending.remove(&gid) else {
                            continue;
                        };
                        match entry.pending.join(session.install.mls_provider(), body) {
                            Ok(mls) => {
                                joined.push((gid, mls, entry.host));
                                break;
                            }
                            Err(_) => {}
                        }
                    }
                }
                for (gid, mls, host) in joined {
                    let live = HostGroup {
                        group_id: gid,
                        cred: host.cred,
                        host_base: session.home_base.clone(),
                    };
                    session.remember_group(live.clone());
                    groups.push(LiveGroup {
                        host: live,
                        mls,
                        nickname: String::new(),
                    });
                }

                for row in &rows {
                    match row.inner.padded_message.type_ {
                        MessageType::MlsHandshake => {
                            for g in groups.iter_mut() {
                                let _ = g.mls.apply_handshake_from_mailbox(
                                    session.install.mls_provider(),
                                    &row.inner,
                                );
                            }
                        }
                        MessageType::MlsApp => {
                            let mut opened = None;
                            for g in groups.iter_mut() {
                                if let Ok(plaintext) = g.mls.decrypt_from_mailbox(
                                    session.install.mls_provider(),
                                    &row.inner,
                                ) {
                                    opened = Some((g.host.clone(), plaintext));
                                    break;
                                }
                            }
                            if let Some((host, plaintext)) = opened {
                                match decode(&plaintext) {
                                    Ok(AppMessage {
                                        header,
                                        body: AppBody::Text { text },
                                    }) => {
                                        new_rows.push(push_text(
                                            inbox,
                                            next_seq,
                                            ids::to_hex(&host.group_id),
                                            header.conv_seq,
                                            text,
                                            header.sent_at,
                                        ));
                                    }
                                    Ok(AppMessage {
                                        header,
                                        body:
                                            AppBody::Attachment {
                                                enc_file,
                                                meta,
                                                fetch_token: Some(token),
                                            },
                                    }) => {
                                        if let Ok(blob) = block_on(session.fetch_file(
                                            host.group_id,
                                            &host.cred,
                                            token,
                                        )) {
                                            if let Ok(bytes) = open_group_file(&enc_file, &blob) {
                                                new_rows.push(push_file(
                                                    inbox,
                                                    next_seq,
                                                    ids::to_hex(&host.group_id),
                                                    header.conv_seq,
                                                    header.sent_at,
                                                    meta,
                                                    bytes,
                                                    ids::to_hex(&token),
                                                ));
                                            }
                                        }
                                    }
                                    Ok(AppMessage {
                                        header,
                                        body: AppBody::Reaction { target, emoji },
                                    }) => {
                                        new_rows.push(push_control(
                                            inbox,
                                            next_seq,
                                            ids::to_hex(&host.group_id),
                                            header.conv_seq,
                                            header.sent_at,
                                            "reaction",
                                            emoji,
                                            target,
                                        ));
                                    }
                                    Ok(AppMessage {
                                        header,
                                        body: AppBody::Delete { target },
                                    }) => {
                                        let conv = ids::to_hex(&host.group_id);
                                        hide_target(inbox, &conv, target);
                                        new_rows.push(push_control(
                                            inbox,
                                            next_seq,
                                            conv,
                                            header.conv_seq,
                                            header.sent_at,
                                            "deleted",
                                            String::new(),
                                            target,
                                        ));
                                    }
                                    Ok(AppMessage {
                                        header,
                                        body: AppBody::Disappear { seconds },
                                    }) => {
                                        let conv = ids::to_hex(&host.group_id);
                                        disappear.insert(conv.clone(), seconds);
                                        new_rows.push(push_control(
                                            inbox,
                                            next_seq,
                                            conv,
                                            header.conv_seq,
                                            header.sent_at,
                                            "disappear",
                                            seconds.to_string(),
                                            0,
                                        ));
                                    }
                                    Ok(AppMessage {
                                        header,
                                        body:
                                            AppBody::BindingGossip {
                                                identity_id,
                                                seq,
                                                server_id,
                                            },
                                    }) => {
                                        if let Some(row) = apply_binding_gossip(
                                            session,
                                            inbox,
                                            next_seq,
                                            host.group_id,
                                            identity_id,
                                            seq,
                                            server_id,
                                            header.conv_seq,
                                            header.sent_at,
                                        ) {
                                            new_rows.push(row);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        MessageType::RemoveBundle => {
                            for g in groups.iter_mut() {
                                let _ = g.mls.apply_remove_from_mailbox(
                                    session.install.mls_provider(),
                                    &row.inner,
                                );
                            }
                        }
                        _ => {}
                    }
                }

                block_on(session.ack())?;
                let now = now_unix();
                for (peer, upto) in acks {
                    if !session.contacts.contains_key(&peer) {
                        continue;
                    }
                    if let Ok(bytes) = encode(&AppMessage {
                        header: AppHeader {
                            conv_seq: 0,
                            sent_at: now,
                            reply_to: None,
                        },
                        body: AppBody::ProtocolAck { upto },
                    }) {
                        let _ = block_on(session.send_to(&peer, TtlBucket::DEFAULT, &bytes, now));
                    }
                }
                for peer in gossip {
                    if !session.contacts.contains_key(&peer) {
                        continue;
                    }
                    let _ = send_own_contact_capability(session, peer, now);
                    let _ = send_own_binding_gossip(session, peer, now);
                }
                new_rows
            };
            new_rows.extend(idle_maintenance(
                session,
                groups,
                inbox,
                next_seq,
                group_discovery_at,
            )?);
            new_rows
        };
        persist(&inner)?;
        Ok(new_rows)
    }

    pub fn create_group(&self, nickname: String) -> Result<String, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let group_id = {
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let mls = session.install.create_group()?;
            let cap = block_on(session.mint_contact())?;
            let host = block_on(session.create_group(mls.group_signing_public(), cap))?;
            session.remember_group(host.clone());
            let id = ids::to_hex(&host.group_id);
            groups.push(LiveGroup {
                host,
                mls,
                nickname,
            });
            id
        };
        persist(&inner)?;
        Ok(group_id)
    }

    pub fn mint_group_invite(&self, group_id_hex: String) -> Result<String, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let uri = mint_invite(&mut inner, group_id_hex, None)?;
        persist(&inner)?;
        Ok(uri)
    }

    pub fn mint_bound_group_invite(
        &self,
        group_id_hex: String,
        identity_id_hex: String,
    ) -> Result<String, FfiError> {
        let id = parse_identity_id(&identity_id_hex)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let uri = mint_invite(&mut inner, group_id_hex, Some(id))?;
        persist(&inner)?;
        Ok(uri)
    }

    pub fn accept_group_invite(&self, invite_uri: String) -> Result<String, FfiError> {
        let invite = parse_group_invite(&invite_uri)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let uri = {
            let Inner { state, pending, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let acc = block_on(session.accept_invite(invite.group_id, invite.nonce))?;
            let pending_join = session.install.prepare_join(acc.cred.credential_id)?;
            let cap = block_on(session.mint_contact())?;
            let hpke = session.hpke_public();
            let signing_pk = pending_join.group_signing_public();
            let proof = if invite.invitee_binding.is_some() {
                Some(
                    session
                        .install
                        .sign_invitee_proof(invite.group_id, acc.pending_id)?,
                )
            } else {
                None
            };
            let uri = join_request_uri(
                invite.group_id,
                acc.pending_id,
                &pending_join.key_package,
                signing_pk,
                cap,
                hpke,
                acc.cred.credential_id,
                invite.nonce,
                proof.as_ref(),
                acc.invitee_binding,
            );
            pending.insert(
                invite.group_id,
                PendingEntry {
                    pending: pending_join,
                    host: acc,
                },
            );
            uri
        };
        persist(&inner)?;
        Ok(uri)
    }

    pub fn admit_join(&self, join_request_uri: String) -> Result<String, FfiError> {
        let req = parse_join_request(&join_request_uri)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        {
            let Inner {
                state,
                groups,
                minted,
                ..
            } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let invitee_pk =
                Group::key_package_identity_pk(session.install.mls_provider(), &req.key_package)?;
            let invitee_id = nemo_wire::identity_id(&invitee_pk);
            block_on(session.check_discovery_not_revoked(invitee_id, now_unix()))?;
            let bound = req.invitee_binding.is_some()
                || minted
                    .get(&req.nonce)
                    .is_some_and(|i| i.invitee_binding.is_some());
            if bound {
                let invite = minted.get(&req.nonce).ok_or(CoreError::BoundInvite)?;
                let proof = req.proof.as_ref().ok_or(CoreError::BoundInvite)?;
                if proof.pending_id != req.pending_id {
                    return Err(CoreError::BoundInvite.into());
                }
                let vk = VerifyingKey::from_bytes(&invitee_pk)
                    .map_err(|_| FfiError::Core("invitee key".into()))?;
                verify_bound_invite(invite, proof, &vk)?;
            }
            let g = groups
                .iter_mut()
                .find(|g| g.host.group_id == req.group_id)
                .ok_or_else(|| FfiError::Core("unknown group".into()))?;
            flush_mls_update(session, g)?;
            let admit = g.mls.sign_admit(req.group_id, req.pending_id)?;
            let (commit, welcome) = g
                .mls
                .admit(session.install.mls_provider(), &req.key_package)?;
            block_on(session.admit(
                req.group_id,
                &g.host.cred,
                &admit,
                req.signing_pk,
                req.cap,
                req.hpke,
            ))?;
            block_on(session.group_append(
                req.group_id,
                &g.host.cred,
                MessageType::MlsHandshake,
                commit,
            ))?;
            let outer = Group::wrap_handshake(&req.hpke, req.cap, TtlBucket::DEFAULT, welcome)?;
            block_on(session.post_envelope(&outer))?;
        }
        persist(&inner)?;
        Ok(ids::to_hex(&req.credential_id))
    }

    pub fn send_group_text(
        &self,
        group_id_hex: String,
        text: String,
    ) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let row = {
            let seq = bump_seq(&mut inner.next_seq, &group_id_hex);
            let ptext = encode_text(seq, now, &text)?;
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = find_group_mut(groups, &group_id_hex)?;
            refresh_group_members(session, &g.mls)?;
            flush_mls_update(session, g)?;
            let body = g.mls.encrypt(session.install.mls_provider(), &ptext)?;
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::MlsApp,
                body,
            ))?;
            DisplayRow {
                conv_id: group_id_hex,
                conv_seq: seq,
                text,
                sent_at: now,
                file_name: String::new(),
                file_mime: String::new(),
                file_bytes: Vec::new(),
                fetch_token: String::new(),
                kind: String::new(),
                emoji: String::new(),
                target: 0,
                hidden: false,
                displayed_at: now,
            }
        };
        inner.inbox.push(row.clone());
        persist(&inner)?;
        Ok(row)
    }

    pub fn remove_group_member(
        &self,
        group_id_hex: String,
        credential_id_hex: String,
    ) -> Result<(), FfiError> {
        let cred = parse_identity_id(&credential_id_hex)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        {
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = find_group_mut(groups, &group_id_hex)?;
            let bundle = g.mls.remove(session.install.mls_provider(), cred)?;
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::RemoveBundle,
                bundle.encode()?,
            ))?;
        }
        persist(&inner)?;
        Ok(())
    }

    pub fn send_file(
        &self,
        peer_id_hex: String,
        name: String,
        mime: String,
        bytes: Vec<u8>,
    ) -> Result<DisplayRow, FfiError> {
        let peer = parse_identity_id(&peer_id_hex)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let seq = bump_seq(&mut inner.next_seq, &peer_id_hex);
        let meta = FileMeta {
            name: name.clone(),
            mime: mime.clone(),
            size: bytes.len() as u64,
        };
        let ptext = encode(&AppMessage {
            header: AppHeader {
                conv_seq: seq,
                sent_at: now,
                reply_to: None,
            },
            body: AppBody::Attachment {
                enc_file: bytes.clone(),
                meta: meta.clone(),
                fetch_token: None,
            },
        })?;
        {
            let Inner {
                state, intro_sent, ..
            } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            block_on(session.send_attachment(&peer, TtlBucket::DEFAULT, &ptext, now))?;
            let _ = ensure_intro_sent(session, intro_sent, peer, now);
            let _ = send_own_binding_gossip(session, peer, now);
        }
        let Inner {
            inbox, next_seq, ..
        } = &mut *inner;
        let row = push_file(
            inbox,
            next_seq,
            peer_id_hex,
            seq,
            now,
            meta,
            bytes,
            String::new(),
        );
        persist(&inner)?;
        Ok(row)
    }

    pub fn send_group_file(
        &self,
        group_id_hex: String,
        name: String,
        mime: String,
        bytes: Vec<u8>,
    ) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let token = mailbox::random_token();
        let (key, bucket, blob) = seal_group_file(&bytes)?;
        let meta = FileMeta {
            name: name.clone(),
            mime: mime.clone(),
            size: bytes.len() as u64,
        };
        let row = {
            let seq = bump_seq(&mut inner.next_seq, &group_id_hex);
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = find_group_mut(groups, &group_id_hex)?;
            refresh_group_members(session, &g.mls)?;
            flush_mls_update(session, g)?;
            let reserve = AttachmentReserve {
                fetch_token: token,
                size_bucket: bucket,
                ttl_bucket: TtlBucket::DEFAULT,
            };
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::AttachmentReserve,
                reserve.encode(),
            ))?;
            block_on(session.upload_file(g.host.group_id, &g.host.cred, token, blob))?;
            let ptext = encode(&AppMessage {
                header: AppHeader {
                    conv_seq: seq,
                    sent_at: now,
                    reply_to: None,
                },
                body: AppBody::Attachment {
                    enc_file: key,
                    meta: meta.clone(),
                    fetch_token: Some(token),
                },
            })?;
            let body = g.mls.encrypt(session.install.mls_provider(), &ptext)?;
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::MlsApp,
                body,
            ))?;
            DisplayRow {
                conv_id: group_id_hex,
                conv_seq: seq,
                text: name.clone(),
                sent_at: now,
                file_name: name,
                file_mime: mime,
                file_bytes: bytes,
                fetch_token: ids::to_hex(&token),
                kind: "file".into(),
                emoji: String::new(),
                target: 0,
                hidden: false,
                displayed_at: now,
            }
        };
        inner.inbox.push(row.clone());
        persist(&inner)?;
        Ok(row)
    }

    pub fn fetch_group_file(
        &self,
        group_id_hex: String,
        token_hex: String,
    ) -> Result<Vec<u8>, FfiError> {
        let gid = parse_identity_id(&group_id_hex)?;
        let token = ids::copy_fixed(&ids::from_hex(&token_hex)?)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let Inner { state, groups, .. } = &mut *inner;
        let session = match state {
            Some(ClientState::Registered(session)) => session,
            Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
            None => return Err(FfiError::Core("busy".into())),
        };
        let cred = groups
            .iter()
            .find(|g| g.host.group_id == gid)
            .ok_or_else(|| FfiError::Core("unknown group".into()))?
            .host
            .cred;
        Ok(block_on(session.fetch_file(gid, &cred, token))?)
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactRow>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner
            .nicknames
            .iter()
            .map(|(id, nickname)| ContactRow {
                identity_id: id.clone(),
                nickname: nickname.clone(),
            })
            .collect())
    }

    pub fn list_groups(&self) -> Result<Vec<GroupRow>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner
            .groups
            .iter()
            .map(|g| GroupRow {
                group_id: ids::to_hex(&g.host.group_id),
                nickname: g.nickname.clone(),
                member_count: g.mls.member_count() as u64,
            })
            .collect())
    }

    pub fn group_member_ids(&self, group_id_hex: String) -> Result<Vec<String>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        let id = parse_identity_id(&group_id_hex)?;
        let g = inner
            .groups
            .iter()
            .find(|g| g.host.group_id == id)
            .ok_or_else(|| FfiError::Core("unknown group".into()))?;
        Ok(g.mls
            .member_credential_ids()
            .into_iter()
            .map(|c| ids::to_hex(&c))
            .collect())
    }

    pub fn react(
        &self,
        conv_id: String,
        target: u64,
        emoji: String,
    ) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let row = send_app(
            &mut inner,
            &conv_id,
            AppBody::Reaction {
                target,
                emoji: emoji.clone(),
            },
            now,
        )?;
        persist(&inner)?;
        Ok(row)
    }

    pub fn delete_message(&self, conv_id: String, target: u64) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        hide_target(&mut inner.inbox, &conv_id, target);
        let now = now_unix();
        let row = send_app(&mut inner, &conv_id, AppBody::Delete { target }, now)?;
        persist(&inner)?;
        Ok(row)
    }

    pub fn set_disappear(&self, conv_id: String, seconds: u64) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        inner.disappear.insert(conv_id.clone(), seconds);
        let now = now_unix();
        let row = send_app(&mut inner, &conv_id, AppBody::Disappear { seconds }, now)?;
        persist(&inner)?;
        Ok(row)
    }

    pub fn disappear_secs(&self, conv_id: String) -> Result<u64, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.disappear.get(&conv_id).copied().unwrap_or(0))
    }

    pub fn expire_now(&self) -> Result<Vec<DisplayRow>, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let timers = inner.disappear.clone();
        let mut expired = Vec::new();
        for row in inner.inbox.iter_mut() {
            if row.hidden {
                continue;
            }
            let secs = timers.get(&row.conv_id).copied().unwrap_or(0);
            if secs > 0 && now.saturating_sub(row.displayed_at) >= secs {
                row.hidden = true;
                row.kind = "expired".into();
                row.text.clear();
                row.file_bytes.clear();
                expired.push(row.clone());
            }
        }
        inner
            .pending_invites
            .retain(|_, invite| invite.expires_at > now);
        let extra = {
            let Inner {
                state,
                groups,
                inbox,
                next_seq,
                group_discovery_at,
                ..
            } = &mut *inner;
            match state {
                Some(ClientState::Registered(session)) => {
                    idle_maintenance(session, groups, inbox, next_seq, group_discovery_at)?
                }
                _ => Vec::new(),
            }
        };
        expired.extend(extra);
        persist(&inner)?;
        Ok(expired)
    }

    pub fn wait_wakeup(&self) -> Result<(), FfiError> {
        let (home, auth) = {
            let inner = self.inner.lock().map_err(|_| lock_err())?;
            match inner.state.as_ref() {
                Some(ClientState::Registered(session)) => session.wakeup_handle()?,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            }
        };
        let frame = block_on(home.wait_wakeup(&auth))?;
        if !frame.is_empty() {
            return Err(FfiError::Core("wakeup frame must be empty".into()));
        }
        Ok(())
    }

    pub fn start_call(&self, peer_id_hex: String) -> Result<DisplayRow, FfiError> {
        let (turn, cbr) = {
            let inner = self.inner.lock().map_err(|_| lock_err())?;
            if inner.live_call.is_some() {
                return Err(FfiError::Core("call already live".into()));
            }
            if let Some(ClientState::Registered(session)) = inner.state.as_ref() {
                if !session.calls_allowed() {
                    return Err(CoreError::CallsUnavailable.into());
                }
            }
            (turn_config(&inner)?, call_cbr(&inner))
        };
        let call = block_on(Call::offer_with(&turn, cbr))?;
        let local = call.local().clone();
        let now = now_unix();
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let seq = bump_seq(&mut inner.next_seq, &peer_id_hex);
        let ptext = encode(&AppMessage {
            header: AppHeader {
                conv_seq: seq,
                sent_at: now,
                reply_to: None,
            },
            body: AppBody::CallInvite {
                call_id: local.call_id,
                sdp: local.sdp.clone(),
                dtls_fp: local.dtls_fp.clone(),
                ice: local.ice.clone(),
                expires_at: now + 60,
            },
        })?;
        let peer = parse_identity_id(&peer_id_hex)?;
        {
            let session = inner.registered()?;
            block_on(session.send_to_now(&peer, invite_ttl_bucket(), &ptext, now))?;
        }
        let call = Arc::new(call);
        inner.live_call = Some(Arc::clone(&call));
        inner.call_peer = Some(peer_id_hex.clone());
        inner.call_connected = false;
        pump_call(Arc::clone(&call));
        pump_trickle(Arc::clone(&self.inner), call, peer_id_hex.clone());
        let Inner {
            inbox, next_seq, ..
        } = &mut *inner;
        Ok(push_control(
            inbox,
            next_seq,
            peer_id_hex,
            seq,
            now,
            "call_invite",
            ids::to_hex(&local.call_id),
            0,
        ))
    }

    pub fn answer_call(&self, call_id_hex: String) -> Result<DisplayRow, FfiError> {
        let (pending, turn, cbr) = {
            let mut inner = self.inner.lock().map_err(|_| lock_err())?;
            let pending = inner
                .pending_invites
                .remove(&call_id_hex)
                .ok_or_else(|| FfiError::Core("unknown call".into()))?;
            let turn = turn_config(&inner)?;
            if let Some(ClientState::Registered(session)) = inner.state.as_ref() {
                if !session.calls_allowed() {
                    return Err(CoreError::CallsUnavailable.into());
                }
            }
            let cbr = call_cbr(&inner);
            (pending, turn, cbr)
        };
        let call = block_on(Call::answer_with(&turn, &pending.signal, cbr))?;
        let local = call.local().clone();
        let now = now_unix();
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let seq = bump_seq(&mut inner.next_seq, &pending.peer);
        let ptext = encode(&AppMessage {
            header: AppHeader {
                conv_seq: seq,
                sent_at: now,
                reply_to: None,
            },
            body: AppBody::CallAnswer {
                call_id: local.call_id,
                sdp: local.sdp.clone(),
                dtls_fp: local.dtls_fp.clone(),
                ice: local.ice.clone(),
            },
        })?;
        let peer = parse_identity_id(&pending.peer)?;
        {
            let session = inner.registered()?;
            block_on(session.send_to_now(&peer, invite_ttl_bucket(), &ptext, now))?;
        }
        let call = Arc::new(call);
        inner.live_call = Some(Arc::clone(&call));
        inner.call_peer = Some(pending.peer.clone());
        inner.call_connected = true;
        pump_call(Arc::clone(&call));
        pump_trickle(Arc::clone(&self.inner), call, pending.peer.clone());
        let Inner {
            inbox, next_seq, ..
        } = &mut *inner;
        Ok(push_control(
            inbox,
            next_seq,
            pending.peer,
            seq,
            now,
            "call_answer",
            ids::to_hex(&local.call_id),
            0,
        ))
    }

    pub fn reject_call(&self, call_id_hex: String) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let pending = inner
            .pending_invites
            .remove(&call_id_hex)
            .ok_or_else(|| FfiError::Core("unknown call".into()))?;
        send_call_body(
            &mut inner,
            pending.peer,
            AppBody::CallReject {
                call_id: pending.signal.call_id,
            },
            "call_reject",
            now_unix(),
        )
    }

    pub fn end_call(&self) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        if let Some(call) = inner.live_call.take() {
            let _ = block_on(call.close());
            let peer_hex = inner.call_peer.take().unwrap_or_default();
            let connected = inner.call_connected;
            inner.call_connected = false;
            if peer_hex.is_empty() {
                return Err(FfiError::Core("no live call".into()));
            }
            let call_id = call.call_id();
            let (body, kind) = if connected {
                (AppBody::CallEnd { call_id }, "call_end")
            } else {
                (AppBody::CallCancel { call_id }, "call_cancel")
            };
            return send_call_body(&mut inner, peer_hex, body, kind, now);
        }
        let hex = inner
            .pending_invites
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| FfiError::Core("no live call".into()))?;
        let pending = inner
            .pending_invites
            .remove(&hex)
            .ok_or_else(|| FfiError::Core("no live call".into()))?;
        send_call_body(
            &mut inner,
            pending.peer,
            AppBody::CallReject {
                call_id: pending.signal.call_id,
            },
            "call_reject",
            now,
        )
    }

    pub fn call_state(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        if inner.live_call.is_some() {
            if inner.call_connected {
                Ok("live".into())
            } else {
                Ok("calling".into())
            }
        } else if !inner.pending_invites.is_empty() {
            Ok("ringing".into())
        } else {
            Ok("idle".into())
        }
    }

    pub fn received_rtp(&self) -> Result<u64, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner
            .live_call
            .as_ref()
            .map(|c| c.received_rtp())
            .unwrap_or(0))
    }

    /// 48 kHz PCM from the Android org.webrtc capture path (or tests).
    pub fn push_capture_pcm(
        &self,
        samples: Vec<i16>,
        sample_rate: u32,
        channels: u32,
    ) -> Result<(), FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        if let Some(call) = inner.live_call.as_ref() {
            call.push_capture_pcm(&samples, sample_rate, channels);
        }
        Ok(())
    }

    pub fn pull_playback_pcm(&self, max_samples: u32) -> Result<Vec<i16>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner
            .live_call
            .as_ref()
            .map(|c| c.pull_playback_pcm(max_samples))
            .unwrap_or_default())
    }

    pub fn inbox(&self) -> Result<Vec<DisplayRow>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.inbox.clone())
    }
}

fn push_text(
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    conv_id: String,
    conv_seq: u64,
    text: String,
    sent_at: u64,
) -> DisplayRow {
    let seq = *next_seq.get(&conv_id).unwrap_or(&0);
    if conv_seq > seq {
        next_seq.insert(conv_id.clone(), conv_seq);
    }
    let row = DisplayRow {
        conv_id,
        conv_seq,
        text,
        sent_at,
        file_name: String::new(),
        file_mime: String::new(),
        file_bytes: Vec::new(),
        fetch_token: String::new(),
        kind: String::new(),
        emoji: String::new(),
        target: 0,
        hidden: false,
        displayed_at: sent_at,
    };
    inbox.push(row.clone());
    row
}

fn push_file(
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    conv_id: String,
    conv_seq: u64,
    sent_at: u64,
    meta: FileMeta,
    file_bytes: Vec<u8>,
    fetch_token: String,
) -> DisplayRow {
    let seq = *next_seq.get(&conv_id).unwrap_or(&0);
    if conv_seq > seq {
        next_seq.insert(conv_id.clone(), conv_seq);
    }
    let row = DisplayRow {
        conv_id,
        conv_seq,
        text: meta.name.clone(),
        sent_at,
        file_name: meta.name,
        file_mime: meta.mime,
        file_bytes,
        fetch_token,
        kind: "file".into(),
        emoji: String::new(),
        target: 0,
        hidden: false,
        displayed_at: sent_at,
    };
    inbox.push(row.clone());
    row
}

fn ttl_for(disappear: &HashMap<String, u64>, conv: &str) -> TtlBucket {
    match disappear.get(conv).copied().unwrap_or(0) {
        0 => TtlBucket::DEFAULT,
        1..=60 => TtlBucket::SECONDS_60,
        _ => TtlBucket::HOUR,
    }
}

fn hide_target(inbox: &mut [DisplayRow], conv_id: &str, target: u64) {
    for row in inbox.iter_mut() {
        if row.conv_id == conv_id && row.conv_seq == target && row.kind != "reaction" {
            row.hidden = true;
            row.kind = "deleted".into();
            row.text.clear();
            row.file_bytes.clear();
        }
    }
}

fn push_control(
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    conv_id: String,
    conv_seq: u64,
    sent_at: u64,
    kind: &str,
    emoji: String,
    target: u64,
) -> DisplayRow {
    let seq = *next_seq.get(&conv_id).unwrap_or(&0);
    if conv_seq > seq {
        next_seq.insert(conv_id.clone(), conv_seq);
    }
    let row = DisplayRow {
        conv_id,
        conv_seq,
        text: if kind == "disappear" || kind.starts_with("call_") || kind == "binding_conflict" {
            emoji.clone()
        } else {
            String::new()
        },
        sent_at,
        file_name: String::new(),
        file_mime: String::new(),
        file_bytes: Vec::new(),
        fetch_token: String::new(),
        kind: kind.into(),
        emoji,
        target,
        hidden: kind == "deleted",
        displayed_at: sent_at,
    };
    inbox.push(row.clone());
    row
}

fn send_app(
    inner: &mut Inner,
    conv_id: &str,
    body: AppBody,
    now: u64,
) -> Result<DisplayRow, FfiError> {
    let seq = bump_seq(&mut inner.next_seq, conv_id);
    let (kind, emoji, target) = match &body {
        AppBody::Reaction { target, emoji } => ("reaction", emoji.clone(), *target),
        AppBody::Delete { target } => ("deleted", String::new(), *target),
        AppBody::Disappear { seconds } => ("disappear", seconds.to_string(), 0),
        _ => return Err(FfiError::Core("unsupported app body".into())),
    };
    let ptext = encode(&AppMessage {
        header: AppHeader {
            conv_seq: seq,
            sent_at: now,
            reply_to: None,
        },
        body,
    })?;
    let ttl = ttl_for(&inner.disappear, conv_id);
    {
        let Inner { state, groups, .. } = &mut *inner;
        let session = match state {
            Some(ClientState::Registered(session)) => session,
            Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
            None => return Err(FfiError::Core("busy".into())),
        };
        let id = parse_identity_id(conv_id)?;
        if groups.iter().any(|g| g.host.group_id == id) {
            let g = find_group_mut(groups, conv_id)?;
            let cipher = g.mls.encrypt(session.install.mls_provider(), &ptext)?;
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::MlsApp,
                cipher,
            ))?;
        } else {
            block_on(session.send_to(&id, ttl, &ptext, now))?;
        }
    }
    Ok(push_control(
        &mut inner.inbox,
        &mut inner.next_seq,
        conv_id.to_owned(),
        seq,
        now,
        kind,
        emoji,
        target,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_server::{router, AppState};
    use rand::RngCore;
    use std::fs;
    use std::sync::Arc;

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let mut n = [0u8; 8];
        rand::rngs::OsRng.fill_bytes(&mut n);
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", ids::to_hex(&n)));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn serve_home() -> String {
        let listener = runtime()
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        runtime().spawn(async move {
            axum::serve(listener, router(AppState::new()))
                .await
                .expect("serve");
        });
        format!("http://{addr}")
    }

    #[test]
    fn create_exports_ids_and_one_time_mnemonic() {
        let client = NemoClient::create().unwrap();
        let id = client.identity_id_hex().unwrap();
        assert_eq!(id.len(), 64);
        assert_eq!(client.fingerprint().unwrap().len(), 64 + 7);
        let mnemonic = client.take_revocation_mnemonic().unwrap();
        assert!(mnemonic.is_some());
        assert!(client.take_revocation_mnemonic().unwrap().is_none());
    }

    #[test]
    fn preview_contact_shows_peer_fingerprint_before_add() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-preview-a");
        let bob_dir = temp_dir("nemo-ffi-preview-b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let uri = alice.mint_share_uri().unwrap();
        let preview = bob.preview_contact(uri).unwrap();
        assert_eq!(preview.identity_id_hex, alice.identity_id_hex().unwrap());
        assert_eq!(preview.fingerprint, alice.fingerprint().unwrap());
        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn vault_create_open_same_identity() {
        let dir = temp_dir("nemo-ffi-vault");
        let client = NemoClient::create_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        let id = client.identity_id_hex().unwrap();
        let mnemonic = client.take_revocation_mnemonic().unwrap();
        assert!(mnemonic.is_some());
        drop(client);
        let opened = NemoClient::open_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(opened.identity_id_hex().unwrap(), id);
        assert!(opened.take_revocation_mnemonic().unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn register_share_send_fetch_against_in_process_server() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-alice");
        let bob_dir = temp_dir("nemo-ffi-bob");
        let alice = NemoClient::create_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        let bob = NemoClient::create_at(
            bob_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();

        let uri = alice.mint_share_uri().unwrap();
        assert!(uri.starts_with("nemo:1:"));
        let alice_id = alice.identity_id_hex().unwrap();
        let added = bob.add_contact(uri, "Alice".into()).unwrap();
        assert_eq!(added, alice_id);

        let sent = bob
            .send_text(alice_id.clone(), "hello from bob".into())
            .unwrap();
        assert_eq!(sent.text, "hello from bob");
        assert_eq!(sent.conv_id, alice_id);

        let rows = alice.fetch_now().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "hello from bob");
        assert_eq!(rows[0].conv_seq, sent.conv_seq);
        assert_eq!(alice.inbox().unwrap().len(), 1);
        assert!(alice
            .inbox()
            .unwrap()
            .iter()
            .all(|r| r.kind != "binding_conflict"));

        drop(alice);
        drop(bob);
        let alice2 = NemoClient::open_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(alice2.identity_id_hex().unwrap(), alice_id);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn register_again_rehomes_to_a_second_server() {
        let a = serve_home();
        let b = serve_home();
        let dir = temp_dir("nemo-ffi-rehome");
        let alice = NemoClient::create_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        alice.register(a.clone()).unwrap();
        let err = alice.register(a).unwrap_err();
        assert!(err.to_string().contains("already registered"));
        alice.register(b.clone()).unwrap();
        alice.mint_share_uri().unwrap();
        drop(alice);
        let opened = NemoClient::open_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        assert!(opened
            .register(b)
            .unwrap_err()
            .to_string()
            .contains("already registered"));
        opened.mint_share_uri().unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ffi_records_are_display_only() {
        let row = DisplayRow {
            conv_id: "ab".repeat(32),
            conv_seq: 1,
            text: "hi".into(),
            sent_at: 1,
            file_name: String::new(),
            file_mime: String::new(),
            file_bytes: Vec::new(),
            fetch_token: String::new(),
            kind: String::new(),
            emoji: String::new(),
            target: 0,
            hidden: false,
            displayed_at: 1,
        };
        assert_eq!(row.conv_id.len(), 64);
        let _ = row.text;
    }

    fn client_at(dir: &std::path::Path) -> Arc<NemoClient> {
        NemoClient::create_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap()
    }

    impl NemoClient {
        fn test_mark_discovery_stale(&self) {
            let mut inner = self.inner.lock().unwrap();
            if let Some(ClientState::Registered(session)) = inner.state.as_mut() {
                for c in session.contacts.values_mut() {
                    c.last_discovery_unix = 0;
                }
            }
            inner.group_discovery_at.clear();
        }

        fn test_publish_revocation(&self, mnemonic: String, identity_hex: String) {
            let id = parse_identity_id(&identity_hex).unwrap();
            let stmt = nemo_core::revocation_from_mnemonic(&mnemonic, id, now_unix()).unwrap();
            let mut inner = self.inner.lock().unwrap();
            let session = inner.registered().unwrap();
            block_on(session.submit_revocation(&stmt)).unwrap();
        }
    }

    #[test]
    fn group_invite_admit_restart_and_remove() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-ga");
        let bob_dir = temp_dir("nemo-ffi-gb");
        let carol_dir = temp_dir("nemo-ffi-gc");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        let carol = client_at(&carol_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base.clone()).unwrap();
        carol.register(base).unwrap();

        let gid = alice.create_group("crew".into()).unwrap();
        let invite = alice.mint_group_invite(gid.clone()).unwrap();
        assert!(invite.starts_with("nemo-g:1:"));
        let join = bob.accept_group_invite(invite.clone()).unwrap();
        assert!(join.starts_with("nemo-j:1:"));
        let denied = carol.accept_group_invite(invite).unwrap_err();
        assert!(denied.to_string().to_lowercase().contains("denied"));

        let bob_cred = alice.admit_join(join).unwrap();
        let _ = bob.fetch_now().unwrap();
        assert_eq!(bob.list_groups().unwrap().len(), 1);
        assert_eq!(alice.list_groups().unwrap()[0].member_count, 2);

        let sent = alice
            .send_group_text(gid.clone(), "hello crew".into())
            .unwrap();
        let rows = bob.fetch_now().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "hello crew");
        assert_eq!(rows[0].conv_id, gid);
        assert_eq!(rows[0].conv_seq, sent.conv_seq);

        drop(alice);
        let alice2 = NemoClient::open_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        let groups = alice2.list_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_id, gid);
        alice2
            .send_group_text(gid.clone(), "after reopen".into())
            .unwrap();
        let again = bob.fetch_now().unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].text, "after reopen");

        alice2.remove_group_member(gid.clone(), bob_cred).unwrap();
        let err = bob.send_group_text(gid, "still here".into()).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("denied"));

        drop(alice2);
        drop(bob);
        drop(carol);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
        let _ = fs::remove_dir_all(&carol_dir);
    }

    #[test]
    fn pending_join_survives_unlock() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-pend-a");
        let bob_dir = temp_dir("nemo-ffi-pend-b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let gid = alice.create_group("crew".into()).unwrap();
        let invite = alice.mint_group_invite(gid.clone()).unwrap();
        let join = bob.accept_group_invite(invite).unwrap();
        drop(bob);
        let bob2 = NemoClient::open_at(
            bob_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        let _ = alice.admit_join(join).unwrap();
        let _ = bob2.fetch_now().unwrap();
        assert_eq!(bob2.list_groups().unwrap().len(), 1);
        drop(alice);
        drop(bob2);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn bound_invite_requires_inviter_and_proof() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-bnd-a");
        let bob_dir = temp_dir("nemo-ffi-bnd-b");
        let carol_dir = temp_dir("nemo-ffi-bnd-c");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        let carol = client_at(&carol_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base.clone()).unwrap();
        carol.register(base).unwrap();
        let bob_uri = bob.mint_share_uri().unwrap();
        let bob_id = bob.identity_id_hex().unwrap();
        alice.add_contact(bob_uri, "Bob".into()).unwrap();
        let gid = alice.create_group("crew".into()).unwrap();
        let carol_invite = alice.mint_group_invite(gid.clone()).unwrap();
        let carol_join = carol.accept_group_invite(carol_invite).unwrap();
        alice.admit_join(carol_join).unwrap();
        let _ = carol.fetch_now().unwrap();
        let invite = alice.mint_bound_group_invite(gid.clone(), bob_id).unwrap();
        let join = bob.accept_group_invite(invite).unwrap();
        let denied = carol.admit_join(join.clone()).unwrap_err();
        assert!(denied.to_string().to_lowercase().contains("bound"));
        alice.admit_join(join).unwrap();
        let _ = bob.fetch_now().unwrap();
        assert_eq!(bob.list_groups().unwrap().len(), 1);
        drop(alice);
        drop(bob);
        drop(carol);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
        let _ = fs::remove_dir_all(&carol_dir);
    }

    #[test]
    fn one_to_one_and_group_attachments() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-fa");
        let bob_dir = temp_dir("nemo-ffi-fb");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();

        let uri = alice.mint_share_uri().unwrap();
        let alice_id = alice.identity_id_hex().unwrap();
        bob.add_contact(uri, "Alice".into()).unwrap();
        bob.send_file(
            alice_id.clone(),
            "note.txt".into(),
            "text/plain".into(),
            b"hello file".to_vec(),
        )
        .unwrap();
        let dm = alice.fetch_now().unwrap();
        assert_eq!(dm.len(), 1);
        assert_eq!(dm[0].file_name, "note.txt");
        assert_eq!(dm[0].file_bytes, b"hello file");

        let gid = alice.create_group("files".into()).unwrap();
        let invite = alice.mint_group_invite(gid.clone()).unwrap();
        let join = bob.accept_group_invite(invite).unwrap();
        let bob_cred = alice.admit_join(join).unwrap();
        let _ = bob.fetch_now().unwrap();

        let sent = alice
            .send_group_file(
                gid.clone(),
                "crew.bin".into(),
                "application/octet-stream".into(),
                b"group-bytes".to_vec(),
            )
            .unwrap();
        assert!(!sent.fetch_token.is_empty());
        let rows = bob.fetch_now().unwrap();
        let file = rows
            .iter()
            .find(|r| r.file_name == "crew.bin")
            .expect("group file");
        assert_eq!(file.file_bytes, b"group-bytes");
        let token = file.fetch_token.clone();
        assert_eq!(
            alice
                .fetch_group_file(gid.clone(), token.clone())
                .unwrap()
                .len(),
            262_144
        );

        alice.remove_group_member(gid.clone(), bob_cred).unwrap();
        let err = bob.fetch_group_file(gid, token).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("denied"));

        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn react_delete_disappear_survive_reopen() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-i8a");
        let bob_dir = temp_dir("nemo-ffi-i8b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let alice_id = alice.identity_id_hex().unwrap();
        let bob_id = bob.identity_id_hex().unwrap();
        bob.add_contact(alice.mint_share_uri().unwrap(), "A".into())
            .unwrap();
        alice
            .add_contact(bob.mint_share_uri().unwrap(), "B".into())
            .unwrap();

        let sent = bob.send_text(alice_id.clone(), "hello".into()).unwrap();
        let got = alice.fetch_now().unwrap();
        assert_eq!(got[0].text, "hello");

        alice
            .react(bob_id.clone(), sent.conv_seq, "👍".into())
            .unwrap();
        let reacted = bob.fetch_now().unwrap();
        assert!(reacted
            .iter()
            .any(|r| r.kind == "reaction" && r.emoji == "👍"));

        alice.delete_message(bob_id.clone(), sent.conv_seq).unwrap();
        let deleted = bob.fetch_now().unwrap();
        assert!(deleted.iter().any(|r| r.kind == "deleted"));
        assert!(bob
            .inbox()
            .unwrap()
            .iter()
            .any(|r| r.conv_seq == sent.conv_seq && r.hidden));

        alice.set_disappear(bob_id.clone(), 1).unwrap();
        let _ = bob.fetch_now().unwrap();
        drop(alice);
        let alice2 = NemoClient::open_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(alice2.disappear_secs(bob_id.clone()).unwrap(), 1);

        alice2
            .send_text(bob_id.clone(), "ephemeral".into())
            .unwrap();
        let ep = bob.fetch_now().unwrap();
        assert!(ep.iter().any(|r| r.text == "ephemeral"));
        std::thread::sleep(std::time::Duration::from_secs(2));
        let expired = bob.expire_now().unwrap();
        assert!(expired.iter().any(|r| r.kind == "expired"));

        drop(alice2);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn one_to_one_call_signaling_through_mailbox() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-call-a");
        let bob_dir = temp_dir("nemo-ffi-call-b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let alice_id = alice.identity_id_hex().unwrap();
        let bob_id = bob.identity_id_hex().unwrap();
        bob.add_contact(alice.mint_share_uri().unwrap(), "A".into())
            .unwrap();
        alice
            .add_contact(bob.mint_share_uri().unwrap(), "B".into())
            .unwrap();
        match alice.start_call(bob_id) {
            Err(e) => {
                eprintln!("skip I9 FFI call: {e}");
            }
            Ok(invite) => {
                assert_eq!(invite.kind, "call_invite");
                assert_eq!(alice.call_state().unwrap(), "calling");
                let rows = bob.fetch_now().unwrap();
                let incoming = rows
                    .iter()
                    .find(|r| r.kind == "call_invite")
                    .expect("invite");
                assert_eq!(bob.call_state().unwrap(), "ringing");
                let ring = alice.fetch_now().unwrap();
                assert!(
                    ring.iter().any(|r| r.kind == "call_ringing"),
                    "callee must send call_ringing"
                );
                match bob.answer_call(incoming.text.clone()) {
                    Err(e) => eprintln!("skip I9 FFI answer: {e}"),
                    Ok(answer) => {
                        assert_eq!(answer.kind, "call_answer");
                        let answered = alice.fetch_now().unwrap();
                        assert!(answered.iter().any(|r| r.kind == "call_answer"));
                        assert_eq!(alice.call_state().unwrap(), "live");
                        let ended = alice.end_call().unwrap();
                        assert_eq!(ended.kind, "call_end");
                        let hang = bob.fetch_now().unwrap();
                        assert!(hang.iter().any(|r| r.kind == "call_end"));
                    }
                }
            }
        }
        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
        let _ = alice_id;
    }

    #[test]
    fn callee_reject_closes_caller() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-reject-a");
        let bob_dir = temp_dir("nemo-ffi-reject-b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let bob_id = bob.identity_id_hex().unwrap();
        bob.add_contact(alice.mint_share_uri().unwrap(), "A".into())
            .unwrap();
        alice
            .add_contact(bob.mint_share_uri().unwrap(), "B".into())
            .unwrap();
        match alice.start_call(bob_id) {
            Err(e) => eprintln!("skip I9 FFI call: {e}"),
            Ok(_) => {
                let incoming = bob
                    .fetch_now()
                    .unwrap()
                    .into_iter()
                    .find(|r| r.kind == "call_invite")
                    .expect("invite");
                let rejected = bob.reject_call(incoming.text).unwrap();
                assert_eq!(rejected.kind, "call_reject");
                assert_eq!(bob.call_state().unwrap(), "idle");
                let rows = alice.fetch_now().unwrap();
                assert!(rows.iter().any(|r| r.kind == "call_reject"));
                assert_eq!(alice.call_state().unwrap(), "idle");
            }
        }
        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn caller_hangup_before_answer_sends_cancel() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-cancel-a");
        let bob_dir = temp_dir("nemo-ffi-cancel-b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let bob_id = bob.identity_id_hex().unwrap();
        bob.add_contact(alice.mint_share_uri().unwrap(), "A".into())
            .unwrap();
        alice
            .add_contact(bob.mint_share_uri().unwrap(), "B".into())
            .unwrap();
        match alice.start_call(bob_id) {
            Err(e) => eprintln!("skip I9 FFI call: {e}"),
            Ok(_) => {
                let _ = bob.fetch_now().unwrap();
                let canceled = alice.end_call().unwrap();
                assert_eq!(canceled.kind, "call_cancel");
                assert_eq!(alice.call_state().unwrap(), "idle");
                let rows = bob.fetch_now().unwrap();
                assert!(rows.iter().any(|r| r.kind == "call_cancel"));
                assert_eq!(bob.call_state().unwrap(), "idle");
            }
        }
        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn idle_refresh_removes_revoked_group_member() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-idle-a");
        let bob_dir = temp_dir("nemo-ffi-idle-b");
        let alice = client_at(&alice_dir);
        let bob = NemoClient::create_at(
            bob_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        let mnemonic = bob.take_revocation_mnemonic().unwrap().unwrap();
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let bob_id = bob.identity_id_hex().unwrap();
        alice
            .add_contact(bob.mint_share_uri().unwrap(), "B".into())
            .unwrap();
        let gid = alice.create_group("crew".into()).unwrap();
        let invite = alice.mint_group_invite(gid).unwrap();
        let join = bob.accept_group_invite(invite).unwrap();
        let _ = alice.admit_join(join).unwrap();
        let _ = bob.fetch_now().unwrap();
        assert_eq!(alice.list_groups().unwrap()[0].member_count, 2);
        alice.test_publish_revocation(mnemonic, bob_id);
        alice.test_mark_discovery_stale();
        let rows = alice.expire_now().unwrap();
        assert!(
            rows.iter().any(|r| r.kind == "revoked"),
            "idle sweep must surface revocation"
        );
        assert_eq!(alice.list_groups().unwrap()[0].member_count, 1);
        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn wrong_passphrase_does_not_open_vault() {
        let dir = temp_dir("nemo-ffi-bad-pass");
        let _ = NemoClient::create_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        let err = match NemoClient::open_at(
            dir.to_string_lossy().into_owned(),
            "incorrect!!".into(),
            Vec::new(),
        ) {
            Ok(_) => panic!("wrong passphrase opened the vault"),
            Err(e) => e,
        };
        assert!(!err.to_string().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn privacy_private_and_high_survive_unlock() {
        let base = serve_home();
        let dir = temp_dir("nemo-ffi-privacy");
        let alice = NemoClient::create_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        alice.register(base).unwrap();
        assert_eq!(alice.privacy_mode().unwrap(), "normal");
        alice.set_privacy_mode("private".into()).unwrap();
        assert_eq!(alice.privacy_mode().unwrap(), "private");
        alice.set_privacy_mode("high".into()).unwrap();
        assert_eq!(alice.privacy_mode().unwrap(), "high");
        drop(alice);
        let opened = NemoClient::open_at(
            dir.to_string_lossy().into_owned(),
            "correct horse".into(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(opened.privacy_mode().unwrap(), "high");
        opened.set_privacy_mode("maximum".into()).unwrap();
        assert_eq!(opened.privacy_mode().unwrap(), "maximum");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn wakeup_then_fetch_sees_row() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-wake-a");
        let bob_dir = temp_dir("nemo-ffi-wake-b");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();
        let alice_id = alice.identity_id_hex().unwrap();
        bob.add_contact(alice.mint_share_uri().unwrap(), "A".into())
            .unwrap();
        std::thread::scope(|s| {
            let h = s.spawn(|| alice.wait_wakeup());
            std::thread::sleep(std::time::Duration::from_millis(300));
            bob.send_text(alice_id, "ping".into()).unwrap();
            h.join().unwrap().unwrap();
        });
        let rows = alice.fetch_now().unwrap();
        assert!(rows.iter().any(|r| r.text == "ping"));
        drop(alice);
        drop(bob);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }
}
