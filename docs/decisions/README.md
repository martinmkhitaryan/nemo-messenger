# Architecture Decision Records

This folder is the history of the key architectural choices behind the messenger: what was chosen, why, what it costs, and what was rejected.

Each decision lives in its own file and is never edited to say something different once accepted. If a decision changes, a new record is written and the old one is marked **Superseded** with a link to its replacement. The old file stays, so the reasoning trail is preserved.

## Status values

- **Proposed** — under discussion, not yet binding.
- **Accepted** — binding for the current design. The main specification (`../../README.md`) must agree with it.
- **Superseded** — replaced by a later record; kept for history.
- **Deprecated** — no longer applies and has no replacement.

## Decision log

IDs match reading order (foundations first). New records take the next free number (0036 is next) and are listed where a reader should encounter the decision.

### Foundations

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0001](0001-untrusted-server-blind-delivery.md) | 2026-09-19 | The server is untrusted infrastructure; all content is end-to-end encrypted | Accepted |
| [ADR-0002](0002-cryptographic-identity-no-registry.md) | 2026-09-19 | Cryptographic identity for users and servers; no central registry | Accepted |

### Identity

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0003](0003-single-key-identity.md) | 2026-09-19 | One identity per client installation, no device layer | Accepted |
| [ADR-0004](0004-revocation-key.md) | 2026-09-19 | Revocation key instead of successor or recovery mechanisms | Accepted |
| [ADR-0034](0034-local-vault-passphrase.md) | 2026-09-20 | Local vault passphrase, Argon2id, and SQLCipher | Accepted |

### Cryptography and discovery

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0005](0005-double-ratchet-and-mls.md) | 2026-09-19 | Double Ratchet for 1:1, MLS for groups | Accepted |
| [ADR-0006](0006-invite-first-discovery.md) | 2026-09-19 | Invite-first contact discovery, no global lookup | Accepted |
| [ADR-0007](0007-one-time-share-tokens.md) | 2026-09-19 | One-time short-TTL share tokens; QR is a fresh mint | Accepted |

### Delivery and federation

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0008](0008-capability-based-delivery.md) | 2026-09-19 | Capability-based delivery addressing | Accepted |
| [ADR-0009](0009-sealed-sender.md) | 2026-09-19 | Sealed sender: no sender field on any server-visible envelope | Accepted |
| [ADR-0010](0010-padding-buckets.md) | 2026-09-19 | Every envelope is padded to a fixed size bucket | Accepted |
| [ADR-0011](0011-protocol-version-field.md) | 2026-09-19 | Every protocol object begins with a version field | Accepted |
| [ADR-0012](0012-no-global-message-identifiers.md) | 2026-09-19 | No globally meaningful message identifiers | Accepted |
| [ADR-0013](0013-internal-server-timestamps.md) | 2026-09-19 | Server timestamps are internal and never returned to clients | Accepted |
| [ADR-0014](0014-sender-chosen-ttl-bucket.md) | 2026-09-19 | Sender-chosen TTL bucket on the inner envelope | Accepted |
| [ADR-0015](0015-bounded-mailbox-log-and-acks.md) | 2026-09-19 | Mailbox as a bounded opaque log; three kinds of acknowledgement | Accepted |
| [ADR-0016](0016-server-to-server-routing.md) | 2026-09-19 | Server-to-server routing with nested encrypted envelopes | Accepted |
| [ADR-0017](0017-server-hosted-group-streams.md) | 2026-09-19 | Server-hosted MLS group streams with server-side fan-out | Accepted |
| [ADR-0018](0018-group-join-and-member-credentials.md) | 2026-09-19 | Hybrid group join and split member credentials | Accepted |
| [ADR-0019](0019-attachment-as-padded-envelope.md) | 2026-09-19 | Attachments are padded envelopes, not a blob store | Accepted |
| [ADR-0020](0020-opaque-push-wakeup.md) | 2026-09-19 | Opaque push wake-up; the device fetches its own ciphertext | Accepted |

### Privacy and data handling

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0021](0021-privacy-layer-and-modes.md) | 2026-09-19 | Metadata privacy as a separate, tunable layer; Tor optional; High/Maximum cover ships | Accepted |
| [ADR-0022](0022-server-side-data-minimisation.md) | 2026-09-19 | Server-side data minimisation: no presence, receipts, typing, analytics; minimal logs | Accepted |
| [ADR-0023](0023-no-backup-no-recovery.md) | 2026-09-19 | No backup, no recovery, no history export | Accepted |

### Features

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0024](0024-voice-call-architecture.md) | 2026-09-19 | Voice call architecture: E2EE signaling, WebRTC media, SFrame keyed from MLS for groups | Accepted |

### Product and process

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0025](0025-self-hostable-open-source-no-custom-crypto.md) | 2026-09-19 | Self-hostable, open source, established cryptography only | Accepted |
| [ADR-0026](0026-target-platforms.md) | 2026-09-19 | Android, Linux and Windows first; iOS later; shared Rust core | Accepted |
| [ADR-0027](0027-protocol-first-development-order.md) | 2026-09-19 | Protocol-first development order; APIs and schemas come last | Accepted |
| [ADR-0028](0028-implementation-languages-and-libraries.md) | 2026-09-19 | Implementation languages and libraries | Accepted |
| [ADR-0029](0029-cryptographic-identifiers-and-encodings.md) | 2026-09-19 | Cryptographic identifiers, contact-card encoding, MLS suite, and PCS interval | Accepted |
| [ADR-0030](0030-envelope-buckets-and-layout.md) | 2026-09-19 | Envelope version 1 layout, padding buckets, and ttl_bucket set | Accepted |
| [ADR-0031](0031-mailbox-retention-and-owner-auth.md) | 2026-09-19 | Mailbox retention defaults and owner authentication | Accepted |
| [ADR-0032](0032-server-signing-key-and-federation-tls.md) | 2026-09-19 | Server signing key and TLS-pinned federation hop | Accepted |

### API and persistence

| ID | Date | Title | Status |
| --- | --- | --- | --- |
| [ADR-0033](0033-local-http-api-and-postgres-schema.md) | 2026-09-20 | Local HTTP API and PostgreSQL schema | Accepted |
| [ADR-0035](0035-ephemeral-turn-credentials.md) | 2026-09-20 | Ephemeral TURN credentials from the home server | Accepted |

## Adding a decision

1. Copy `TEMPLATE.md` to `NNNN-short-title.md` with the next free number (0036 is next).
2. Fill in every section. "Alternatives considered" and "Cons" are mandatory; a decision without a stated cost has not been thought through.
3. Add a row to the log above under the fitting heading.
4. Update the affected sections of `../../README.md` so the specification and the record agree, and add the record to the summary table in README section 0.
5. If the new record replaces an older one, change the old record's status to `Superseded by ADR-NNNN` and add an entry to its History section. Do not rewrite the old record's content.
6. Small clarifications that do not change a decision may be added to an existing record as a numbered amendment with a History entry. If several independent rules share one record, split them into separate ADRs rather than bundling.
