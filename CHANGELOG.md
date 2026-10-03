# Changelog

All notable changes to Nemo Messenger are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> All code in this project was written by AI, using
> Fable 5.1, Sol 5.6, Muse Spark 1.3, Grok 4.6 / 4.7 and Composer 2.5.

## [0.4.3] - 2026-10-03

### Fixed

- Reply quote accent is stable per user: own quotes always share one
  color instead of changing per message (seq-qualified fallback keys
  removed in bubble and composer preview).
- Reply bubble timestamp pins to the bubble corner when the quote is
  wider than the reply: the width probe now includes the quote author
  plus header chrome, and pill rows fill the bubble width.

## [0.4.2] - 2026-10-03

### Fixed

- Reaction pills live inside the bubble on the timestamp line (emoji +
  count only, no avatar dots); same-meaning emoji variants aggregate.
- Bubble timestamp can no longer overlap text: measured and rendered
  widths match by construction.
- Unreachable home servers fail fast with bounded connect/request
  timeouts instead of freezing sends and fetches.

### Fixed

- Unreachable home servers fail fast instead of freezing sends/fetches:
  bounded connect/request timeouts, so the UI reports "couldn't reach
  the home server" within seconds.

## [0.4.1] - 2026-10-03

### Fixed

- Invite/join cards no longer resurrect Accept/Admit after restart:
  taken actions persist per vault.
- Copy buttons removed from invite/join cards.
- Soft keyboard keeps Enter for newlines; hardware Enter still sends.
- Bubble timestamp floats in the last line's tail instead of guttering
  the whole text block.
- Bubbles flatter (14dp), emoji-only messages render large with no
  bubble, group sender names colored, pills float below bubbles.
- Reply quotes have no filled box; attach moved inside the composer pill.
- Composer rebuilt as a floating dock with glass tint and docked reply
  card; menu cards more opaque with a proper expand chevron.

## [0.4.0] - 2026-10-02

### Added

- Message replies: quote any bubble, reply header with peer accent color
  and quoted preview, quotes hide when the original is deleted.
- Multi-emoji reactions with author list: tap a pill to toggle, long-press
  for who reacted, quick picker in the message menu.
- Fluid message menu anchored near the bubble (flips above/below on edges)
  with reaction bar, Reply / Copy / Save / Delete actions.
- Delete scope choice: delete for everyone or only for you.

### Changed

- Deletes are hard deletes: the message and its reactions are completely
  removed, never tombstoned, and no deleted bubble is fetched back.

## [0.3.6] - 2026-10-02

### Fixed

- Composer cursor no longer jumps onto the text after ~13 characters.
  The `decorationBox` wrapper sized to content and capped the paragraph
  at ~119px regardless of field width; the placeholder overlay now only
  wraps an empty field.

## [0.3.5] - 2026-10-02

### Fixed

- Background arrivals no longer mark messages seen: viewport read
  tracking is gated on foreground, so an open-but-backgrounded chat
  keeps messages unread until actually viewed.
- Notification taps no longer force a passphrase prompt: MainActivity is
  `singleTask`, so taps resume the live unlocked instance instead of
  recreating it. Genuine process-death taps still correctly lock.

## [0.3.4] - 2026-10-02

### Added

- Composer shows a progress ring around the send button filling toward
  the 8192-byte single-message cap (red at the limit, send disabled).
  Overlong drafts are rejected up front with a clear warning instead of
  a backend error.

## [0.3.3] - 2026-10-02

### Added

- Group display names sync to joiners: admitting auto-sends the name
  over E2EE 1:1 when the joiner is a contact; it applies silently
  (stashed if the Welcome lands later). Renames will sync the same way.
- Group messages show their author: incoming group bubbles carry a sender
  header (contact nickname, else short id), sourced from the MLS leaf.

## [0.3.2] - 2026-10-02

### Changed

- Your own sent join-request card no longer shows a Copy button — just
  the sent state. Incoming cards are unchanged.

## [0.3.1] - 2026-10-02

### Added

- Accepting a group invite card now automatically sends the join request
  back to the inviter — no contact picker step. Invite cards flip to
  `Request sent` and join cards to `Admitted ✓` once tapped, so the same
  invite or request can't be submitted twice. Group-thread/clipboard
  invites still fall back to the manual Join sheet.

### Fixed

- Group names survive app restart. They were kept only in memory and reset
  to `Group` on every unlock; now they persist in the vault display store
  (old vaults load unchanged).

## [0.3.0] - 2026-10-01

### Added

- Group invites and join requests can be sent directly to a 1:1 contact
  instead of copy-paste. Incoming invites render as a card with
  Accept/Copy; incoming join requests render as a card with Admit/Copy.
  Accept opens the Join sheet with the request ready to send.
  Manual copy-paste stays as fallback at every step. Consent is unchanged:
  joining still needs the invitee's Accept tap and the member's Admit tap,
  never silent auto-add.

### Fixed

- S2S dialer now drops 14-day-expired outbound rows and counts them as
  expired, like the in-process pump. Previously expired rows were skipped
  but left queued forever.

## [0.2.0] - 2026-10-01

### Added

- Human read receipts: new `read { upto }` message type (the documented
  `user_receipt`), sent only when a chat is open and in the foreground —
  never from background fetch. Tracked per conversation, persisted in the
  vault, exposed as `NemoClient.mark_read()` / `read_upto()`. Ticks are now
  three-tier and theme-adaptive: single ✓ on send, faded ✓✓ on delivery,
  full-emphasis ✓✓ on viewed. 1:1 chats only; group read receipts are
  future work (see README). Per-device setting, on by default; turning it
  off hides sending and display both ways.
- Real delivery receipts: inbound `protocol_ack` messages are now recorded
  per conversation, persisted in the vault across unlocks, and exposed to
  the shell as `NemoClient.acked_upto()`. Outgoing bubbles show single ✓ on
  network accept and ✓✓ only when the peer confirms decryption.

### Changed

- Binding gossip is sent only when `(binding_seq, server_id)` actually
  changed for a peer (cached, self-healing after restart) instead of on
  every message. Steady-state 1:1 traffic drops ~2 POSTs per message; the
  ADR-0006 anti-equivocation tripwire still fires immediately on rehome
  (covered by a federated rehome test with S2S pump).
- Prekey restock no longer blocks register/unlock: a small synchronous
  stock (5) publishes inline and the remainder tops up on a background
  thread.

### Fixed

- Delivery ticks refresh on every fetch cycle, not just on new rows:
  ack-only fetches previously updated FFI state invisibly, so double ticks
  appeared only after the peer replied.
- Initial viewport is reported once marking enables, so single-message
  chats mark viewed on open without scrolling.
- Floating date pill shows only for finger scrolling, never for
  programmatic pin-to-bottom animations on send/receive.

## [0.1.1] - 2026-09-30

### Fixed

- Home-server Postgres persistence is incremental instead of rewriting all
  tables on every request. Each mutating request previously ran a full
  `TRUNCATE` + row-by-row re-`INSERT` (~350 ms with a few thousand rows),
  and a chat message costs ~10 such requests, so receives took ~2 s against
  a live server. Endpoints now journal what they changed in memory and the
  write-through persist issues only the affected statements (single-row
  upserts/deletes, batched multi-row inserts), with a no-op fast path when
  nothing changed. Measured on the reference deploy: `fetch_now` ~2.1 s to
  ~85 ms, `send_text` ~750 ms to ~40 ms, `register` ~8.5 s to ~0.35 s.
  No wire or schema change; crash-reload semantics unchanged
  (`crates/nemo-server/tests/persist.rs` covers multi-round reloads).

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
- Notifications: foreground `SyncService` wake + `PollWorker`
  fallback, generic-text alerts. No FCM by design.

Manual testing status (tested, untested, and the must-test list before
1.0.0) now lives in [`docs/manual-testing.md`](docs/manual-testing.md).

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
- Notifications: foreground `SyncService` (default, manually tested) holding `/v1/wakeup` + `PollWorker` fallback (manually tested), generic-text alerts with per-conversation seen marks, Nemo logo icons. No FCM by design.
- Single `VaultStore` fetch owner (poll + wake under mutex); UI/notifier collect deltas; bounded wake waits with stale-wake drop on identity switch.
- Chat list chrome + empty-state CTA, animated phase/chat/settings transitions, attachment save dialogs, friendly backend errors + not-connected banner with Connect action.

### Deliberately out of scope (v1)

- No history backup/export, no crypto-state recovery, no multi-device linking, no iOS/macOS/web, no group calls/video, no FCM push, no key transparency, no group migration. See `docs/v1.1.md`, `docs/v1.2.md`, and ADR-0023.
