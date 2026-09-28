# Changelog

All notable changes to Nemo Messenger are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> All code in this project was written by AI, using
> Fable 5.1, Sol 5.6, Muse Spark 1.3, Grok 4.6 / 4.7 and Composer 2.5.

## [0.1.0] - 2026-09-28

First release. Rust core + self-hostable home server + Compose clients
for Desktop (Linux, Windows via JVM) and Android (minSdk 35 / Android 15+).

### Manually tested in 0.1.0

Tested by hand on desktop two-pane (two vaults), Android emulator,
and a physical device:

- Identity / contacts: create, unlock, Home URL connect, contact-card
  add with fingerprint confirm, local nicknames.
- 1:1 text: send/receive both directions, persistence across unlock.
- 1:1 file attachments: send/receive and save to disk.
- Notifications (non-FCM): foreground `SyncService` wake + `PollWorker`
  fallback, generic-text alerts.

Not manually tested in 0.1.0 (implemented, covered by automated
tests where noted in `docs/testing.md`): groups, 1:1 voice calls
(no live-audio check passed), High/Maximum cover traffic and Tor.

Left to test (required before 1.0.0):

- Groups: New group -> copy invite -> Join group with `nemo-j:` request
  -> admit -> both sides send, on desktop and Android
  (`docs/testing.md` freeze checklist).
- 1:1 live call: audible speech with real mic (not silence frames),
  ringing/reject/cancel, mic permission, via coturn on desktop two-pane
  (headphones on one side), emulator, and physical device.
- Privacy modes: High cover traffic, Maximum constant-rate slots,
  Tor SOCKS for non-loopback homes.

### Identity and vault

- One installation = one identity (`id = H(pubkey)`): no accounts, no registry, no device linking.
- Passphrase-protected (min 8 chars) SQLCipher vault in `nemo-core`, device-bound secret (Android Keystore / OS credential store). Lost passphrase or device secret is a lost identity.
- Revocation keypair per identity, shown once as 24-word phrase + QR. Revocation kills the identity; no recovery, no successor.
- Invite-first discovery: contact cards (`nemo:1:` URI, QR, link, file) mint a fresh one-time short-TTL share token per share. Fingerprint verification (Verified / Unverified / Revoked states).
- Local editable contact nicknames, persisted across unlock.
- Identity portability: move same identity to a new home via signed `seq`-monotonic home-server binding; old server drops tokens/prekeys and disables mailbox.

### 1:1 messaging

- E2EE 1:1 via PQXDH + Double Ratchet (audited libsignal-class primitives, no custom crypto). One-time prekeys only; new sessions wait when stock is empty.
- Sealed sender, padded fixed-size buckets, per-container sequence numbers, bounded mailbox (14 d / 500 MiB default) with offline backlog, out-of-order/duplicate handling.
- Contact capabilities exchanged in-session, rotated in-band; first sender burns the share token.
- Cooperating-client reactions, delete, and disappearing messages.
- Inbox history and outgoing authorship persisted across vault unlock/restart.
- Periodic discovery refresh + MLS healing while online; gossip of home-server binding `seq` with conflict alert.

### Groups (MLS) — implemented, not manually tested

- Server-hosted MLS groups (suite 0x0003): ordered opaque stream, fan-out to member mailboxes, signed invite -> pending join -> signed admit flow.
- Bound invites (`H(identity_public_key || salt)`), in-band KeyPackage/proof delivery, member credentials bound to MLS leaf, `RemoveBundle` handling for revocation.
- MLS state persisted in vault; periodic Update commits for PCS/healing in quiet groups.
- Group file/attachment objects fetched with write-once token + member credential.

### Attachments

- 1:1 attachments as padded E2EE envelopes (up to 16 MiB), manually tested including save-to-disk from bubble menu on desktop and Android. No public CDN/blob store.
- Group attachments share the same envelope format; implemented but not manually tested.

### Voice calls (1:1) — implemented, no live-audio check passed

- In-conversation E2EE signaling (ringing / reject / cancel), WebRTC media always relayed through self-hosted TURN (`iceTransportPolicy=relay`); no host/srflx exposure.
- Trickle ICE via `call_ice`, Opus CBR with DTX/VAD off in Private mode, ephemeral per-call TURN credentials (`POST /v1/turn`).
- Optional coturn sidecar in `deploy/compose.yml --profile calls`.

### Privacy modes — High/Maximum not manually tested

- Normal / Private / High / Maximum. Padding buckets always on; calls always-relay.
- Private adds batching/jitter; High adds client-generated dummy cover + optional Tor SOCKS5 (non-loopback homes); Maximum uses ~2 s constant-rate slots, no calls.

### Server, federation, deploy

- Self-hostable Axum home: blind delivery (mailboxes + group streams), discovery, federation S2S over TLS 1.3 pinned Ed25519 mTLS (`nemo-s2s/1`) with HPKE inner envelopes, Caddy TLS-termination + Postgres (frozen DDL, write-through) or in-memory engine.
- Transports: localhost HTTP client-to-home (`0.0.0.0:8787` default for LAN), HTTPS via Caddy `:8443`, WebSocket `GET /v1/wakeup` push wakes (empty frame) plus poll fallback.
- Data minimisation: no plaintext/keys/sender/nicknames/receipts/typing/presence/analytics; capability- and peer-scoped rate limits; operator logs hardened against secret leak/replay.
- `deploy/compose.yml` reference stack (Caddy + Postgres + nemo-server + optional coturn). MIT-licensed server/wire/deploy.

### Apps (Compose Multiplatform + UniFFI)

- Single Gradle project: Desktop JVM + Android. UI never holds ratchet keys; `nemo-ffi` (UniFFI over `nemo-core`, AGPL) exposes create/unlock/register/share-card/send/fetch.
- Onboarding: branded create/unlock, revocation-phrase chips with copy, Home URL connect, QR scan/generate, New chat sheet with verify-contact card.
- Telegram-style thread: optimistic send with delivery ticks, bubble entrance + mobile composer-to-bubble morph, Enter-to-send, day separators + floating date pill with collision push, unread badge + in-thread marker + jump-to-unread / new-message button, pinned bottom-right timestamps, floating translucent header pills, top fade mask, screen-space outgoing gradients.
- Themes: System / Light / Dark / Mono Light / Mono Dark, persisted by name.
- Notifications: foreground `SyncService` (default, manually tested) holding `/v1/wakeup` + `PollWorker` fallback (manually tested), generic-text alerts with per-conversation seen marks, Nemo logo icons. No FCM in 0.1.0.
- Single `VaultStore` fetch owner (poll + wake under mutex); UI/notifier collect deltas; bounded wake waits with stale-wake drop on identity switch.
- Chat list chrome + empty-state CTA, animated phase/chat/settings transitions, attachment save dialogs, friendly backend errors + not-connected banner with Connect action.

### Deliberately out of scope (v1)

- No history backup/export, no crypto-state recovery, no multi-device linking, no iOS/macOS/web, no group calls/video, no FCM push, no key transparency, no group migration. See `docs/v1.1.md`, `docs/v1.2.md`, and ADR-0023.
