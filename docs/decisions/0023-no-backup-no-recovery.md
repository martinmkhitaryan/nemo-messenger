# ADR-0023: No history backup and no identity recovery in v1

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 16, 37, 41, 42, 51, 52, 53.11

## Context

Three different things are commonly lumped together as "backup":

1. **Identity recovery** — regaining the ability to act as the same identity after losing the device.
2. **Cryptographic state recovery** — restoring ratchet or MLS state.
3. **Message history backup** — restoring readable past messages on a new installation.

Each weakens a security property the project holds as an invariant: identity recovery needs an escrow or a key hierarchy (rejected in ADR-0003); state recovery contradicts forward secrecy; history backup means plaintext-equivalent material exists outside the device.

## Decision

- **Identity recovery: none.** A lost identity is gone. The only external control over an identity is revocation (ADR-0004).
- **Cryptographic state recovery: none, ever.** Ratchet and MLS state are never exported, backed up or synchronised. This is a permanent invariant, not a v1 limitation.
- **Message history backup: none in v1.** No encrypted export, no server-side blob. Attachment envelopes (ADR-0019) are transient mailbox/stream ciphertext, not a backup. If history backup is added later it must be a separate ADR that states explicitly that it weakens historical confidentiality, and it must never include cryptographic state.

## Pros

- Forward secrecy and "server compromise does not reveal history" hold without exceptions or footnotes.
- No escrow, no recovery phrase for messages, no server-side encrypted blobs to protect or to be compelled to hand over.
- The threat model stays honest: the device is the only place plaintext exists.

## Cons

- Losing a device loses everything. Users accustomed to cloud restore will find this harsh.
- New installations start empty; the same person on two devices has two unrelated histories.
- No path to satisfy users who legitimately need archival.

## Alternatives considered

### Encrypted local export/import of history

User-initiated file export, passphrase-encrypted, re-imported on a new installation. Deferred; the most likely future addition.

### Server-side encrypted backup blob

Client-encrypted history stored at the home server. Deferred; creates a compellable artifact and a key the user must protect.

### Social recovery (Shamir shares among contacts) for identity

Rejected together with any identity hierarchy (ADR-0003).

## Consequences

- README 53.11 is resolved as a non-goal.
- Client UX must state at identity creation that the identity and its history cannot be recovered.
- The "Non-goals" section of the README lists all three items.

## History

- 2026-09-19 — Accepted. No history backup and no identity recovery in v1.
