# Task: expiry/GC for contact-capability tokens

Status: planned (not implemented).

## Problem

`tokens` is the only unbounded server table. Every 1:1 message mints contact
capabilities (rotation on send + on every receive), and tokens are only ever
deleted when a mailbox is disabled (`disable_mailbox`). Mailbox rows have
14-day retention + acks, `seen_enc` is a 1024-entry window, prekeys are
consumed on use — tokens grow forever (~2 per message exchange, plus one
share token per add; 1,278 rows observed on a lightly-used test server).

Consequences as it grows: slower `load()` on restart, fatter backups, and
(before incremental persist) slower every request. Same "fine today,
incident later" shape as the per-request full rewrite was.

## Background

* `ADR-0008:24` — capabilities are "rotated in-band with a grace period".
* `ADR-0008:38` — the server stores "capability → mailbox, state, **expiry**".
* Only the grace half exists (`rotate_contact` confirms); the mint path
  (`mint_contact_capability`, used per message) never sets expiry.

## Design (agreed)

No schema change, no wire change. All signals already exist in the tables.

**Share tokens** — fully decidable, delete when:
* expired (`now > expires_at`; fetch already rejects these) + 1-day buffer
  for in-flight requests,
* burned without `reserved_prekey` + 1-day buffer,
* burned *with* reservation + 7-day retry window (a requester whose
  response was lost can retry the same token).

**Contact caps** — delete when **not the newest minted for their mailbox
AND last use (`window_start`, already bumped on every authorized append)
older than 30 days**. Rationale: mailbox retention already abandons
14-day-idle peers' history, so a 30-day fuse is consistent and generous.
The newest cap per mailbox is never deleted.

**Known edge (accepted):** a peer idle >30 days holding only a deleted old
cap bounces its next send (`Denied`) and cannot self-heal — only its peer's
next message (carrying a fresh rotation) or a fresh share re-arms it. Same
UX class as an expired QR invite. No silent data loss: undelivered mail is
never created, the sender gets an explicit error.

## Mechanism

* `HomeDirty` gains per-token drops (`DELETE WHERE token = ...`); today's
  `tokens_dropped` is per-owner (mailbox disable) and stays.
* **Opportunistic prune:** on `mint_contact_capability`, sweep that
  mailbox's dead tokens first (amortized, no new jobs).
* **Periodic sweeper:** hook the 1-second `pump_loop` with a 24h gate. First
  tick after boot sweeps, so deploying cleans legacy rows immediately.
* Constants next to `CONTACT_GRACE_SECS` in `home.rs`
  (`CONTACT_CAP_IDLE_SECS`, share buffers); amend ADR-0008 with the policy.

## Tests

* Unit (no DB): controlled `window_start`/`expires_at`, assert kept/dropped
  sets — newest-always-kept, burned-reserved retention, grace behavior.
* Live (scratch PG, existing `NEMO_TEST_DATABASE_URL` harness): multi-round
  persist/load asserting swept rows stay gone while newest caps,
  reservations, and sends keep working.
* Deploy check: row counts before/after first sweep on a real database.

## Acceptance

* Steady-state token count per mailbox bounded (newest + recently used).
* No behavior change for peers idle <30 days; no silent loss anywhere.
* `cargo test --workspace` + live PG suite green.

## Deliberately out

Per-peer "confirmed rotation" tracking (would need a new confirm message
type — no such channel exists; usage-anchored GC is equivalently safe),
client fallback protocol for bounced sends, group-cred GC (separate topic).
