# ADR-0007: One-time short-TTL share tokens; QR is a fresh mint

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 3.4, 20, 21, 22, 23, 53.12

## Context

ADR-0006 published a long-lived introduction capability on the contact card. Anyone who saw the card (forwarded link, screenshot, scrape) could write to that mailbox until the owner rotated and republished. Security review treated that as a reusable inbox password.

The user chose the same machine for in-person QR and for links: every share is a newly minted, single-use, short-lived token. A QR is that token drawn as a picture, not a durable badge.

## Decision

- **Every share mints a new token.** Showing a QR, copying a link, or exporting a card file creates a fresh one-time capability with a short TTL (ADR-0014 buckets, typically minutes). Two people require two codes.
- **QR and link are encodings of the same object.** The payload is still a contact card (identity key, revocation key, home-server binding, version) plus **one** introduction token. There is no long-lived introduction capability.
- **Consume-on-use.** A connect token is burned when the recipient's server accepts the first envelope addressed to it. A group-invite is burned when the invitee redeems it into a pending join (ADR-0018). Fetching a prekey requires a live unused connect token and does **not** consume it. **At most one one-time prekey is reserved per live token**; a repeat fetch returns that same reserved key. A second key is not issued. Further uses after consume are rejected with the same constant-time unknown-token response as a never-valid token (ADR-0008).
- **Race after scan is accepted.** Whoever sends the first envelope to a connect token wins. In-person QR windows are short. A forwarded link is treated like a password. Bob does not have to be online to confirm.
- **Expiry.** Unused tokens die at TTL. Screenshots become useless.
- **After first contact,** parties exchange private per-conversation contact capabilities inside the encrypted session, as before (ADR-0008).
- **Group invites** are a different object (client-signed, ADR-0018) that reuses the same consume and TTL rules. A connect token cannot join a group.
- The server that stores the token must not learn who redeemed it beyond "this token was consumed". Redeem is sealed / capability-only.

## Pros

- A leaked share is one shot and time-bounded.
- In-person and remote introduction are one protocol.
- Rotation is the default, not an emergency procedure.

## Cons

- Two people cannot scan the same QR. The screen must mint again for the next person.
- If Alice is offline and her one-time prekeys are also exhausted, a new chat cannot start (ADR-0005); the token may expire first. One leaked token can reserve only one prekey, not the whole stock.
- Whoever sees a connect token before the intended person can send first and burn it (accepted; review T1). Shares must travel on a channel the user already trusts, or be shown in person.

## Alternatives considered

### Long-lived introduction capability on the card (ADR-0006 original)

Rejected: reusable write token after any leak.

### On-screen QR that auto-rotates every few seconds

Compatible, not required. Fresh-each-display is enough.

### Consume the share token on prekey fetch

Rejected: the token must still be valid for the first envelope. Offline Bob would lose the introduction if fetch consumed it.

### Multi-use short window (N scans / 10 minutes)

Rejected for v1: a screenshot during the window is still reusable.

## Consequences

- ADR-0006 contact card no longer carries a durable introduction capability; it carries a just-minted token.
- README section 20 is rewritten around mint / scan / consume.
- Related: ADR-0018 (group-invite tokens and member credentials).

## History

- 2026-09-19 — Accepted. One-time short-TTL share tokens; QR is a fresh mint.
