# Temporary implementation plan

**Status:** working document, not a specification and not an ADR.
**Delete this file** when the v1 checkpoints below are done (or fold any leftover notes into `docs/`).
**Authoritative sources:** `README.md`, `docs/decisions/`, `docs/protocol/`. If this file disagrees with a record, the record wins.

How to use it:

- Finish one phase, hit its **checkpoint**, commit, then start the next.
- Do not start a later phase until the previous checkpoint is green.
- Stop only for a product choice that needs a new ADR (next free number is **0035**).
- Standing constraints: MIT spec + `nemo-wire` + `nemo-server`; AGPL `nemo-core` / `nemo-ffi` / Compose because of libsignal; no custom crypto; server MUST NOT link libsignal or depend on `nemo-core`; no Electron/Flutter; Tor / cover traffic / group calls are **nice-to-have** (Track N), not blockers.

---

## Snapshot (2026-09-20)

| Track | State |
| --- | --- |
| Protocol phases 1–8 (ADR-0027) | **Done** — documents under `docs/protocol/` |
| ADRs 0001–0034 | **Accepted** |
| Server HTTP + Postgres + S2S mTLS + wakeup WS | **Done** |
| Client identity, PQXDH, Double Ratchet, MLS (in RAM) | **Done** |
| SQLCipher vault for identity + libsignal store (ADR-0034) | **Done** (`701b992`) |
| MLS group state in the vault | **Done** (I1) |
| Durable home session | **Done** (I2) |
| UniFFI beyond create/open/save | **Not started** ← next
| Compose UI | Placeholder window only |
| 1:1 WebRTC media | Signaling types exist; media engine not wired |
| FCM | Not started (needs schema amendment) |
| Tor/Arti, cover traffic, group calls | Nice-to-have — Track N |

---

## Track P — Protocol (complete)

These are ADR-0027 phases. They are **closed**. Do not reopen them to add product UI.

| Phase | Document | Checkpoint (already met) |
| --- | --- | --- |
| P1 Security model | `docs/protocol/01-security-model.md` | Threat model + trust boundaries written |
| P2 Cryptographic protocol | `02-cryptographic-protocol.md` | Ed25519 identity, PQXDH, MLS suite `0x0003`, revocation |
| P3 Envelope protocol | `03-envelope-protocol.md` | Sealed sender, buckets, no sender field |
| P4 Delivery protocol | `04-delivery-protocol.md` | Share tokens, capabilities, mailbox, acks |
| P5 Federation | `05-federation.md` | HPKE nested hop, pins, ALPN `nemo-s2s/1` |
| P6 Privacy transport | `06-privacy-transport.md` | Normal/Private ship; High = Tor flag only |
| P7 Application protocol | `07-application-protocol.md` | Text, attachments, reactions, call signaling types |
| P8 API and persistence | `08-api-and-persistence.md` + ADR-0033 | `/v1` routes, frozen DDL, no JSON protocol objects |

---

## Track S — Server runtime (complete)

| Phase | What shipped | Checkpoint (already met) |
| --- | --- | --- |
| S1 Wire crate | `nemo-wire` CBOR encodings | `cargo test -p nemo-wire`; `no_libsignal` |
| S2 In-process home | `HomeServer` / `GroupHost` | Delivery + group join tests |
| S3 HTTP | Axum 0.8 localhost `/v1` | `crates/nemo-server/tests/http.rs` |
| S4 Postgres | sqlx + `0001_init.sql` | Optional `NEMO_TEST_DATABASE_URL`; in-memory default |
| S5 Federation listen | mTLS 9443, pin verifiers | `tests/s2s.rs` |
| S6 Wakeup | `GET /v1/wakeup` empty binary, 10 s coalesce | HTTP wakeup test |
| S7 CI | `.github/workflows/ci.yml` + `deny.toml` | `cargo test --workspace` on push |
| S8 Deploy | Caddy + Postgres compose; optional coturn `--profile calls` | Dockerfile builds `nemo-wire` + `nemo-server` only |

Do not add FCM tables here without a new ADR (frozen DDL, ADR-0033).

---

## Track I — Remaining product implementation

This is the live work. Phases are ordered so a restart never drops crypto, then a loop can talk to a home, then the shell can call that loop, then screens, then calls.

### I1 — Persist MLS groups in the vault

**Goal.** An installation that created or joined a group can unlock the vault and still encrypt/decrypt in that group.

**In scope.**

- Serialize `Group` / `PendingJoin` (OpenMLS `MlsGroup` + leaf signer + group signing key + `credential_id` map + `last_own_update`) into the existing SQLCipher `kv` table.
- Round-trip test: create two-member group, save, reopen vault, send another MLS app message.
- Still never store the revocation mnemonic.

**Out of scope.** Group UI, host credentials in the shell, nicknames.

**Checkpoint.**

- [x] `Group` encode/decode round-trip in `nemo-core` tests
- [x] Vault reopen keeps MLS epoch and decrypts a post-reload message
- [x] `cargo test --workspace`
- [x] Commit on `initial-release`

**Stop if.** OpenMLS 0.6 cannot dump `MlsGroup` without a storage-provider redesign — then write ADR-0035 for the store choice, do not invent a second MLS encoding.

---

### I2 — Durable home session (register / restock / fetch / send)

**Goal.** `nemo-core` can run a mailbox loop against a live `nemo-server` using only vault + HTTP, without the test `RouterTransport` helpers leaking into the product.

**In scope.**

- Persist beside the installation: home `ServerBundle`, mailbox `cursor`, unpublished prekey need, contact-capability map, member credentials (`HostCred` / `HostGroup`).
- On unlock: restock if `unused_one_time_prekeys < 25`, publish to `/prekeys`, fetch `/mailbox/fetch`, decrypt, ack.
- Discovery refresh timer (`DISCOVERY_REFRESH_SECS`) for 1:1 contacts; mark revoked and refuse send (ADR-0004).
- Optional: connect `GET /v1/wakeup` and still poll fetch (ADR-0020: poll remains required).

**Out of scope.** Compose screens, FCM, Tor.

**Checkpoint.**

- [x] Integration test: two `HomeSession`s, restart both from vault, 1:1 message still decrypts
- [x] Revoked contact: fetch discovery, send refused
- [x] `cargo test --workspace`
- [x] Commit

---

### I3 — Widen UniFFI (no keys across the boundary)

**Goal.** Compose can create/unlock, register, share a card, send/receive text, without holding ratchet or MLS secrets.

**In scope.** `NemoClient` methods roughly:

- `create_at` / `open_at` / `save` (already exist)
- `register(home_https_base)`
- `fingerprint` / `identity_id_hex` (exist)
- `take_revocation_mnemonic` (exist)
- `mint_share_uri() -> String` (`nemo:1:` card)
- `add_contact(card_or_uri, nickname)`
- `send_text(peer_id_hex, text)`
- `fetch_now()` / inbox callbacks or poll of display rows
- Display rows: `conv_id`, `conv_seq`, plaintext text, `sent_at` — **in memory or vault as display-only**, never ratchet keys in Kotlin

**Out of scope.** Generating UniFFI Kotlin in CI (I5), call media, group list.

**Checkpoint.**

- [ ] Rust tests on the FFI crate for register+send against in-process server
- [ ] No secret types in the UDL/UniFFI surface (ids, fingerprints, mnemonic once, ciphertext never)
- [ ] `cargo test -p nemo-ffi`
- [ ] Commit

---

### I4 — Compose: identity, unlock, fingerprint, 1:1 text

**Goal.** A person can install the desktop shell, choose a passphrase, write down the revocation phrase, unlock later, add a contact from a card/QR URI, and exchange text with another installation.

**In scope.**

- Screens: create, show mnemonic once, unlock, own fingerprint, paste/scan card, conversation list, 1:1 thread, send box.
- Wire generated Kotlin bindings (even if generated by hand in this phase).
- Local nicknames in the vault as display rows (ADR-0028: UI may hold decrypted display rows).
- “Cannot be recovered” copy at create (ADR-0023).
- Verify in the browser or by driving the Compose window: create → unlock → send → receive.

**Out of scope.** Android APK, groups, attachments picker, calls, FCM.

**Checkpoint.**

- [ ] Two desktop processes (or one process, two vault dirs) exchange a text message via local `nemo-server`
- [ ] Killing and relaunching both still decrypts new messages (vault I1+I2)
- [ ] Shell writes no `*.db` of its own besides what `nemo-core` creates
- [ ] Commit

---

### I5 — Bindings build and Android skeleton

**Goal.** One Gradle project produces desktop JVM and an Android debug APK that links `nemo-ffi` (JNA / AAR).

**In scope.** UniFFI Kotlin generation in Gradle; `cdylib` for desktop; Android NDK/JNI or JNA AAR as ADR-0028. Shared Compose screens from I4 on Android.

**Out of scope.** Play Store, iOS, push.

**Checkpoint.**

- [ ] `./gradlew :compose:run` (desktop) still does I4 flow
- [ ] Android debug install on emulator: create identity, show fingerprint
- [ ] CI builds the Gradle project or documents the exact command
- [ ] Commit

---

### I6 — Groups in the product

**Goal.** Create a hosted group, invite → accept → admit, send MLS text, RemoveBundle on revocation.

**In scope.** FFI + Compose: create group, show members as fingerprints/nicknames, invite URI, accept, admit, group thread. Core already has `Group` / `HomeSession` group routes. Persist host credentials (I2). Periodic MLS Update (7 d / 72 h before send, ADR-0029). Discovery refresh for every MLS member (ADR-0004).

**Out of scope.** Group calls, anonymous membership, group migration.

**Checkpoint.**

- [ ] Three clients: A creates, B joins via invite, C is refused without admit
- [ ] After vault restart, A still sends in the group (depends on I1)
- [ ] Revocation of B: A commits RemoveBundle; B cannot append
- [ ] `cargo test --workspace` + the desktop/Android path used in I4/I5
- [ ] Commit

---

### I7 — Attachments

**Goal.** 1:1 padded A* DR file envelopes; groups: `AttachmentReserve` + host blob + MLS caption with fetch token (ADR-0019).

**In scope.** File picker in Compose; size/bucket limits from ADR-0030; no public CDN.

**Checkpoint.**

- [ ] 1:1: send a small file, recipient opens after fetch
- [ ] Group: one upload, two members fetch with live credentials; dead credential 403
- [ ] Commit

---

### I8 — Reactions, delete-for-everyone request, disappearing, protocol acks

**Goal.** Remaining v1 `AppBody` types from phase 7 are usable in the thread UI.

**In scope.** Cooperating-client hide for delete; disappear timer display; `protocol_ack` / capability gossip as core behavior with minimal chrome.

**Checkpoint.**

- [ ] Tests already in `app.rs` stay green; UI shows reaction and deleted placeholder
- [ ] Disappear setting survives vault reopen (display + local hide, not a server flag)
- [ ] Commit

---

### I9 — 1:1 voice calls

**Goal.** Signaling already in `nemo-core` app types; media is WebRTC always-relay (ADR-0024, ADR-0028).

**In scope.**

- DTLS fingerprint + relay-only ICE in invite/answer (reject host/srflx — tests exist).
- Desktop: `webrtc` 0.20.x + APM/cpal/Opus **or** libwebrtc if APM quality fails (ADR-0028).
- Android: `org.webrtc` capture/APM in the shell; signaling stays in Rust.
- TURN: existing compose `--profile calls` coturn.

**Out of scope.** Group calls, video, calls over Tor.

**Checkpoint.**

- [ ] Two clients on the same machine complete a relayed audio call through local coturn
- [ ] Direct ICE candidates are refused
- [ ] Commit

**Stop if.** Desktop AEC is unusable — that is the ADR-0028 cons item; pick libwebrtc vs `webrtc-audio-processing` in a short note, not a new crypto ADR.

---

### I10 — Client wakeup: WebSocket (and later FCM)

**Goal.** Android-without-Play and desktop use the long-lived `/v1/wakeup` socket while the app runs. Fetch still required.

**In scope.** FFI subscribe; Compose foreground service / desktop keep-alive. Coalesce 10 s.

**FCM (optional sub-slice I10b).** Opaque 32-byte wake token only (ADR-0020). **Needs ADR-0035** (or an amendment to ADR-0033) because `0001_init.sql` has no push-endpoint table. Do not sneak a column in.

**Checkpoint (I10a).**

- [ ] Mailbox ingest wakes the client WS; client fetch sees the row
- [ ] Frame payload is empty binary
- [ ] Commit

---

### I11 — Packaging and self-host docs

**Goal.** An operator can `docker compose up` and a user can install a desktop package.

**In scope.** `jpackage` or documented JVM run; `deploy/README.md` with Caddy, Postgres password as reference; no cloud vendor API.

**Checkpoint.**

- [ ] Fresh clone: compose up, two clients register on `https://localhost:8443` (or documented port)
- [ ] LICENSE.md still maps MIT vs AGPL correctly
- [ ] Commit

---

### I12 — Hardening pass (v1 freeze)

**Goal.** Nothing in Track I is a prototype pretending to be the spec.

**In scope.** cargo-deny on the client too if needed; no secrets in logs; mailbox owner auth 120 s window; rate-limit notes; README revision bump.

**Checkpoint.**

- [ ] `cargo test --workspace`
- [ ] `cargo test -p nemo-wire -p nemo-server --test no_libsignal`
- [ ] Manual: revoke phrase kills discovery; stolen-vault-without-passphrase does not unlock
- [ ] Delete **this file** or replace it with a one-paragraph “v1 shipped” note in `docs/`
- [ ] Commit

---

## Track N — Nice-to-have (do not block v1)

User-marked deferred. Implement only after I12, or after an explicit “pull this forward”.

| ID | Item | Notes |
| --- | --- | --- |
| N1 | Tor / Arti | Privacy `High` already has a hop flag; plug Arti behind `EnvelopeSink`. No system `tor` binary (ADR-0028). |
| N2 | Cover traffic / Maximum / constant-rate | Wire-compatible dummies exist; do not ship a mode that lies. |
| N3 | Group calls | SFrame + call-specific MLS + SFU (ADR-0024). Video stays out. |
| N4 | FCM HTTP v1 | After I10b ADR. Operator service account; payload = opaque wake token. |

Also still out of v1 by README §52: iOS/macOS/web, history export, multi-device, key transparency, group migration.

---

## Decision gates (stop and write an ADR)

Do not silently decide these:

| Topic | Why it needs a record |
| --- | --- |
| FCM / push-endpoint table | Frozen DDL (ADR-0033) |
| OS keystore wrapping the SQLCipher key | ADR-0034 deferred this |
| Changing Argon2 parameters incompatibly | kdf version; maybe just a format bump in code if params stay in `kdf.cbor` |
| MLS storage-provider swap | I1 stop condition |
| P2P ICE for calls | ADR-0024 forbids it |
| JSON protocol objects | ADR-0033 |
| Putting libsignal on the server | ADR-0028; CI must keep failing |

Passphrase minimum (8) and in-memory-vs-sqlx server default are already decided. Do not stop for those.

---

## Suggested commit rhythm

One commit per checkpoint (existing `initial-release` style: one sentence, why not what). Do not batch I4+I6+I9 into a single commit.

Order if time is short before a demo: **I1 → I2 → I3 → I4**. That is a recoverable 1:1 messenger. Groups (I6), files (I7), calls (I9) can follow without rewriting the shell.

---

## Next action

**I2 is implemented.** Start **I3** (widen UniFFI: register, share card, send/receive text).
