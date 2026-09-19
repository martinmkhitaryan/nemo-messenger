# ADR-0005: Double Ratchet for 1:1, MLS for groups

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 6, 12, 13, 17, 18, 20, 21, 45, 51

## Context

The original draft chose a Double Ratchet family for 1:1 and MLS (RFC 9420) for groups. During review the alternative of using MLS for everything, with a 1:1 conversation being a two-member group, was raised: one protocol to audit, one state machine, trivial upgrade from 1:1 to group, and a single key-derivation path (the MLS exporter) for calls.

With ADR-0003 in place there is no multi-device fan-out, so both options are simple. The decision therefore rests on protocol properties and implementation maturity rather than on complexity.

## Decision

- **1:1 conversations** use the Signal protocol family: an asynchronous post-quantum session setup (PQXDH) followed by the Double Ratchet, as implemented in libsignal or an equivalent audited library.
- **Group conversations** use MLS (RFC 9420) as implemented in OpenMLS, mls-rs or an equivalent audited library.
- Both protocols authenticate with the same identity key (ADR-0003). The identity key is an Ed25519 signing key; the X25519 key for PQXDH and the HPKE init key for MLS key packages are separate keys signed by it. One fingerprint covers all of them.
- A 1:1 conversation that becomes a group starts a new MLS group; there is no in-place upgrade.
- **Prekeys are one-time only.** Discovery serves a finite stock of PQXDH one-time prekeys. There is no reusable last-resort prekey. If the stock is empty, a **new** 1:1 cannot start until the recipient comes online and restocks. Existing conversations are unaffected.
- **Fetching a one-time prekey requires a live unused share token** (ADR-0007). Knowledge of `identity_id` or an old contact card is not enough. The fetch does not consume the token; the first envelope to it does.
- **At most one one-time prekey is reserved per live share token.** Repeat fetch with the same token returns that same reserved key so the legitimate sender can retry. A second key is not issued. The token still burns on the first envelope. Consume-on-fetch is rejected: it would break offline Bob.
- **Groups are not post-quantum.** Do not claim PQ for MLS. Standardised MLS post-quantum ciphersuites are still drafts.
- **Clients MUST periodically commit MLS Updates** so a quiet group still heals. The interval is specified with the group state machine; this record does not pick a number.
- **Clients MUST reject External Commits, external joins, and MLS ReInit** (RFC 9420). Join is hybrid admit (ADR-0018). Group re-form after a dead host is deferred. The host still delivers those bytes if appended (RFC 9750 §5.3); clients do not apply them.

## Pros

- **Maturity.** X3DH/Double Ratchet has a decade of deployment at Signal and WhatsApp scale and several formal analyses. libsignal is audited Rust with Java, Swift and TypeScript bindings. MLS libraries are solid but younger.
- **Post-quantum today.** libsignal ships PQXDH for session setup and a PQ ratchet for ongoing forward secrecy. MLS post-quantum ciphersuites are still drafts.
- **No sequencer for 1:1.** The Double Ratchet tolerates arbitrary reordering and loss; the server is a dumb mailbox. MLS needs a total order on commits even for two members.
- **Continuous post-compromise security.** The DH ratchet heals on every reply. MLS heals only when a member commits an Update, which needs a policy and a round-trip through the sequencer.
- **Lower per-message overhead for 1:1.** No epoch, group context or PrivateMessage framing.
- **Deniability** of 1:1 authentication is preserved; MLS handshake messages are signed.
- **Independent failure.** A bug in one stack does not take down the other.

## Cons

- Two protocols to audit, test and maintain.
- Two verification stories that must be presented as one to the user.
- Converting a 1:1 conversation to a group starts fresh; no history carries over.
- Calls need two key-derivation paths: a shared secret carried in the encrypted call offer for 1:1, the MLS exporter for groups.

## Alternatives considered

### MLS-only

1:1 as a two-leaf MLS group. Rejected for now because of the sequencer requirement on every 1:1 conversation, commit-driven PCS, and the absence of standardised post-quantum ciphersuites. Remains a valid future direction if MLS PQ ciphersuites and libraries mature; revisiting requires a new ADR.

### Double Ratchet with sender keys for groups (WhatsApp / Signal legacy groups)

Rejected: sender keys give weaker forward secrecy and post-compromise security in groups, and require O(n) pairwise sessions per group.

## Consequences

- Discovery serves a finite stock of one-time PQXDH prekeys, and only to a requester who presents a live unused share token. At most one prekey is reserved per token. MLS KeyPackages are issued on group accept (ADR-0018), not left infinitely fetchable.
- The server must implement per-group ordered streams for MLS (ADR-0017) but only plain mailboxes for 1:1.
- The specification's fingerprint definition must cover the identity key and state that all other keys are signed by it.
- Clients must refuse to start a new 1:1 on an empty prekey bundle rather than fall back to a reusable key.
- Clients MUST periodically commit MLS Updates; a silent group must not wait for someone to happen to speak before PCS applies.
- Marketing, docs and threat-model text must not claim post-quantum security for groups.

## History

- 2026-09-19 — Accepted. Double Ratchet for 1:1; MLS for groups.
