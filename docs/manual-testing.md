# Manual testing

What has been verified by hand, what hasn't, and what must be verified
before 1.0.0. Automated coverage is tracked separately in
[`testing.md`](testing.md).

## Manually tested in 0.1.0

Tested by hand on desktop two-pane (two vaults), Android emulator,
and a physical device:

- Identity / contacts: create, unlock, Home URL connect, contact-card
  add with fingerprint confirm, local nicknames.
- 1:1 text: send/receive both directions, persistence across unlock.
- 1:1 file attachments: send/receive and save to disk.
- Notifications: foreground `SyncService` wake + `PollWorker`
  fallback, generic-text alerts. No FCM by design.

## Manually verified in 0.2.0

Verified live during development on desktop two-pane against the
reference deploy (Caddy + TLS + Postgres):

- Server timings after incremental persist: register, send, and fetch
  latencies back to interactive levels.
- 1:1 delivery/read ticks: single tick on send, faded double tick on
  peer fetch, full-emphasis double tick only after the peer views the
  chat; no false double ticks from background fetch.
- Viewport gating: single-message chats mark viewed on open without
  scrolling; date pill stays hidden while writing.

## Manually verified in 0.4.5

Verified live during development on desktop two-pane against the
reference deploy:

- Groups: New group -> copy invite -> Join group with `nemo-j:` request
  -> admit.
- Message replies and reactions.
- Group message seen logic is not implemented.

## Not manually tested

Implemented, covered by automated tests where noted in `testing.md`:
group messaging and group seen ticks, 1:1 voice calls (no live-audio
check passed), High/Maximum cover traffic and Tor, server federation.
Group read receipts are future work: seen logic is not implemented
for groups.

## Left to test (required before 1.0.0)

- Groups: group seen ticks once seen logic is implemented
  (`testing.md` freeze checklist).
- 1:1 live call: audible speech with real mic (not silence frames),
  ringing/reject/cancel, mic permission, via coturn on desktop two-pane
  (headphones on one side), emulator, and physical device.
- Privacy modes: High cover traffic, Maximum constant-rate slots,
  Tor SOCKS for non-loopback homes.
- Server federation: two homes with pinned peer bundles
  (`NEMO_PEERS_DIR`, S2S mTLS), one identity per home, cross-server
  1:1 text both directions.
