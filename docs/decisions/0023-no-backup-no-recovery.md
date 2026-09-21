# ADR-0023: No backup, no recovery, no history export

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 16, 37, 41, 42, 51, 52, 53.11

## Context

Three different things are commonly lumped together as "backup":

1. **Identity recovery** — regaining the ability to act as the same identity after losing the device.
2. **Cryptographic state recovery** — restoring ratchet or MLS state.
3. **Message history backup or export** — copying readable past messages off the live installation (file, blob, import on another device, or any other dump).

Each weakens a security property the project holds as an invariant: identity recovery needs an escrow or a key hierarchy (rejected in ADR-0003); state recovery contradicts forward secrecy; history backup or export means plaintext-equivalent material exists outside the device.

## Decision

- **Identity recovery: prohibited.** A lost identity is gone. The only external control over an identity is revocation (ADR-0004).
- **Cryptographic state recovery: prohibited, ever.** Ratchet and MLS state are never exported, backed up or synchronised. This is a permanent invariant, not a v1 limitation.
- **Message history backup: prohibited.** No encrypted export, no server-side blob, no archival copy of the vault.
- **History export: prohibited.** There is no client API, file format, or UI for dumping or restoring messages, chats, attachments, or searchable history. The one-time revocation mnemonic (ADR-0004) is not a history export. Contact-card QR/URI share (ADR-0006) is not a history export.

This is a permanent prohibition, not a deferral. Attachment envelopes (ADR-0019) are transient mailbox/stream ciphertext, not a backup. The local SQLCipher vault (ADR-0034) is at-rest encryption on that device, not a backup. Do not add an export, backup, or restore path.

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

Rejected. A user-initiated file is still plaintext-equivalent material off the device.

### Server-side encrypted backup blob

Rejected. Creates a compellable artifact and a key the user must protect.

### Social recovery (Shamir shares among contacts) for identity

Rejected together with any identity hierarchy (ADR-0003).

## Consequences

- README 53.11 is resolved as prohibited.
- Client UX must state at identity creation that the identity and its history cannot be recovered or exported.
- The product-prohibitions section of the README lists all three items. Do not add an export or backup path without a new ADR that explicitly supersedes this one.

## History

- 2026-09-19 — Accepted. No history backup and no identity recovery in v1.
- 2026-09-21 — Amendment 1: local history export/import is a non-goal, not a deferred leftover.
- 2026-09-21 — Amendment 2: no backup and no history export at all; not a v1 limitation. Alternatives that were deferred are rejected.
- 2026-09-21 — Amendment 3: backup, recovery, and history export are prohibited architecture, not a leftover or a later feature.
