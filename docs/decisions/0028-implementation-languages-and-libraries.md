# ADR-0028: Implementation languages and libraries

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 0, 33, 49, 50, 51, 54, 63, 64; [LICENSE.md](../../LICENSE.md); ADR-0026 (media-engine consequence only)

## Context

The specification is protocol-first (ADR-0027) and until this record contained no binding implementation languages. An implementer still needs a single stack that matches the cryptographic libraries already named (ADR-0005), the Rust core and v1 platforms (ADR-0026), self-hosting with no vendor lock-in (ADR-0025), and the project's MIT specification licence.

`libsignal` (the audited PQXDH + Double Ratchet implementation) is AGPL-3.0-only. Linking it into a distributed client forces that client to be AGPL. The server never runs the Double Ratchet. MLS-only (1:1 as a two-leaf group) was re-evaluated as a way to drop `libsignal` and keep the client MIT; it remains rejected (ADR-0005).

This record binds languages and libraries for when code is written. It does not freeze HTTP routes, database DDL, or a public REST schema; those remain phase 8 (ADR-0027).

## Decision

### Languages and crate split

- **Protocol, cryptography, local vault, call signaling:** Rust, in one core crate. No second language on the security-critical path.
- **Blind server** (discovery, mailboxes, group streams, federation, push): Rust. It shares a wire crate with the client and **MUST NOT** depend on the client core or on `libsignal`.
- **v1 UI:** Kotlin, one [Compose Multiplatform](https://www.jetbrains.com/compose-multiplatform/) application for Android, Linux, and Windows. FFI is [UniFFI](https://mozilla.github.io/uniffi-rs/) generating Kotlin (JNA AAR on Android, JNA on desktop JVM). Swift bindings later with iOS (ADR-0026).
- **Web client:** out of scope for v1 (ADR-0026).
- Planned crate layout when code starts:

```text
nemo-wire      MIT     versioned encodings; the only crate both sides compile
nemo-server    MIT     discovery, delivery, federation, push
nemo-core      AGPL    identity, libsignal, OpenMLS, vault, privacy transport
nemo-ffi       AGPL    UniFFI surface
apps/compose   AGPL    Compose Multiplatform shell
deploy         MIT     container, Caddy, PostgreSQL, optional coturn
```

### Licences

- **Specification** (`README.md`, `docs/`) and **server / wire / deploy:** MIT, matching the existing `LICENSE`.
- **Client core, FFI, and apps:** AGPL-3.0, because they link `libsignal`.
- The repository root **MUST** carry a licence map ([LICENSE.md](../../LICENSE.md)) so a single GitHub `LICENSE` file cannot be read as “the whole product is MIT.” When a Cargo workspace exists, `cargo-deny` (or equivalent) **MUST** fail a server crate that depends on `libsignal` and a client crate that is not AGPL.

### Cryptographic libraries (client)

- **1:1:** `libsignal` for **PQXDH and Double Ratchet primitives only**. The client MUST NOT speak Signal's service protocol. Share tokens, mailboxes, sealed sender, and federation are this project's (ADR-0007, ADR-0009, ADR-0016).
- **Groups:** OpenMLS (RFC 9420). Prefer it over `mls-rs` unless OpenMLS cannot express a required extension; switching requires a new record.
- **1:1 stays Double Ratchet, not MLS.** MLS-only remains rejected (ADR-0005). Revisit only when MLS post-quantum ciphersuites are an RFC, OpenMLS ships them as stable, and the project accepts a sequencer on every 1:1 conversation.
- **Identity / revocation:** Ed25519 (`ed25519-dalek`). `identity_id = H(pubkey)` with `H` named in the phase-2 specification.
- **Federation HPKE:** RFC 9180 via the RustCrypto `hpke` crate, the same dependency in `nemo-wire` on client and server.
- **TLS:** `rustls` for outbound HTTPS (federation, FCM). Inbound TLS is the reverse proxy, not the Rust process.
- No custom primitives or tweaks to PQXDH, Double Ratchet, MLS, or HPKE (ADR-0025).

### Encoding (when those protocols are specified)

- MLS handshake and application bytes: as produced by OpenMLS.
- Contact cards / QR: compact CBOR (or a fixed versioned binary that fits a QR). JSON is not used for cards.
- Application and envelope types in `nemo-wire`: protobuf (`prost`) with a leading protocol-version field (ADR-0011). Serialize, **then** pad to a bucket (ADR-0010).

### Client shell, vault, calls, push, Tor

- Compose must not persist ratchet or MLS keys. The vault is SQLCipher via `rusqlite` **inside `nemo-core`**. The UI may hold decrypted display rows in memory.
- **Calls (v1 1:1):** signaling stays in the E2EE conversation in the Rust core (ADR-0024). ICE/DTLS-SRTP/TURN uses `webrtc` 0.20.x (webrtc-rs) with `iceTransportPolicy=relay` and no host or srflx candidates. Echo cancellation, AGC and NS use a WebRTC audio-processing module: on Android the official `org.webrtc` capture/APM path in the Kotlin shell; on desktop `webrtc-audio-processing` + `cpal` + Opus, or libwebrtc if APM quality is not enough. **This amends ADR-0026:** the media engine may live in the shell; signaling and DTLS fingerprint binding stay in the core. Group calls and SFrame remain deferred (ADR-0024).
- **Push:** FCM HTTP v1 is Android-with-Play-Services only; payload is an opaque wake token (ADR-0020). Desktop and Android without Play Services use a long-lived WebSocket or poll while the application runs. The protocol MUST NOT require Google.
- **Tor:** optional, via Arti behind the privacy-transport trait (ADR-0021). Not a system `tor` binary.

### Server (phase 8, after the protocols exist)

- **Runtime:** Tokio. **HTTP and WebSocket:** Axum 0.8 + Tower (`tower-http`). Listen on plain HTTP on localhost; **Caddy** terminates TLS in the reference deploy. QUIC is optional later; v1 is HTTPS + WebSocket.
- **Database:** PostgreSQL via `sqlx` (`query!`, offline mode, `sqlx-cli` migrations). No Redis. SQLite is not the only server database in v1.
- **Push (operator):** FCM HTTP v1 with `reqwest` + rustls and an operator-supplied service account. No unmaintained FCM wrappers.
- **TURN:** coturn as a sidecar. Do not rewrite TURN in Rust.
- **Packaging:** Docker Compose with `nemo-server`, PostgreSQL, Caddy, optional coturn. No cloud vendor API in the protocol.

## Pros

- One Rust core matches `libsignal` and OpenMLS with no FFI on the crypto path (ADR-0026).
- The server stays MIT and forkable by self-hosters who never take AGPL.
- Compose Multiplatform is one UI for the three v1 platforms; UniFFI already emits Swift for iOS later.
- Axum + Tower + sqlx are the boring Tokio default for a mailbox log and long-lived WebSockets; Caddy matches “container + database + reverse proxy” (ADR-0025).
- Using `libsignal` as primitives keeps Nemo's delivery protocol independent of Signal's servers.

## Cons

- The distributed client is AGPL; some contributors and distributors will refuse that.
- Two messaging protocols to maintain (ADR-0005).
- Two call media engines until desktop AEC quality is proven.
- Compose Desktop ships a JRE (`jpackage`); heavier than a pure-Rust GUI.
- Axum and sqlx are bound before phase 8 code exists; a later framework swap needs a new record, not a silent change. Routes and DDL remain unfrozen.

## Alternatives considered

### MLS for 1:1 as well (drop libsignal, keep the client MIT)

Rejected: every DM would need a hosted ordered stream (ADR-0017), the host would learn the two-member set (a metadata regression vs ADR-0016), PCS would be commit-driven, 1:1 would not be post-quantum until MLS PQ ciphersuites are an RFC, and attachments/share tokens would have to be redesigned. See ADR-0005.

### Go (or Python) server, Rust client

Rejected: duplicates envelope and HPKE types; the server would drift from `nemo-wire`.

### Electron, Tauri, Flutter, iced, egui, or Slint as the product UI

Rejected: web stack is out of v1 (ADR-0026); Flutter is a second runtime next to UniFFI; iced/egui fail accessibility and IME for a messenger; Slint is a second DSL and does not share the Android UI. gtk-rs/libadwaita remains a Linux-native escape hatch, not v1.

### Actix-web, Rocket, Warp, or gRPC as the public API

Rejected: Actix's RPS edge is irrelevant (bottleneck is Postgres and HPKE); Rocket and Warp do not compose with Tower as Axum does; gRPC is hostile to self-host reverse proxies and opaque padded envelopes. Federation is HTTP POST of HPKE bytes.

### SeaORM, Diesel, MongoDB, or Redis

Rejected: mailboxes are SQL-shaped opaque logs, not CRUD entities. Diesel is sync-first. Mongo and Redis add operator surface ADR-0025 does not require.

### webrtc-rs without an AEC/APM path

Rejected: the crate is ICE/DTLS-SRTP/TURN, not echo cancellation. Unusable 1:1 calls.

### Hand-written JNI or flutter_rust_bridge

Rejected: UniFFI is the Kotlin/Swift path; flutter_rust_bridge assumes Flutter.

## Consequences

- README sections 0, 33, 49, 50, 51, 54, 63 and 64 must agree with this record.
- ADR-0026's consequence “libwebrtc via the Rust core on all three platforms” is amended as stated above; the platforms and Rust-core decisions stand.
- First code, when leaving documentation-only work, is `nemo-wire` plus a `nemo-core` identity/PQXDH prototype — not a chat UI and not a REST server.
- Independent implementations may use other languages; this record binds *this* project's implementation.

## History

- 2026-09-19 — Accepted. Implementation languages and libraries.
