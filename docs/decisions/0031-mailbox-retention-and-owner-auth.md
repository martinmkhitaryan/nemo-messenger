# ADR-0031: Mailbox retention defaults and owner authentication

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 7, 9, 38, 53.8; [`docs/protocol/04-delivery-protocol.md`](../protocol/04-delivery-protocol.md)

## Context

[ADR-0015](0015-bounded-mailbox-log-and-acks.md) requires bounded logs and three ack kinds but not numbers. The Double Ratchet skip window must be sized against those numbers (README 53.8). Delivery also needs a way for the mailbox **owner** to fetch and ack without turning a delivery capability into a read token ([ADR-0008](0008-capability-based-delivery.md)).

## Decision

- **Mailbox retention (v1 defaults, advertised in server capabilities):** drop the oldest envelopes when the mailbox exceeds **500 MiB** or **14 days** since receipt, whichever comes first. Operators MAY set stricter limits; they MUST advertise the values they actually apply. They MUST NOT offer unbounded retention.
- **Group stream retention:** drop oldest stream objects (not counting group file blobs) when the stream exceeds **2 GiB** or **30 days**. Group file objects follow `ttl_bucket` and the 16 MiB cap; they count toward a separate **4 GiB** per-group file budget.
- **Skip window:** clients MUST retain Double Ratchet skipped-message keys for at least **2000** messages. MLS out-of-order application messages: follow OpenMLS defaults, not less than one epoch of buffering.
- **Owner fetch/ack:** authenticated to the **home server only**, with the identity key (context `mailbox-owner` in [phase 2](../protocol/02-cryptographic-protocol.md) style, specified in the delivery document). A delivery capability, share token, or member credential MUST NOT authorise fetch or ack of a mailbox.
- Unknown capabilities and unknown fetch tokens return a **constant-time** indistinguishable failure.

## Pros

- Numbers exist so skip windows and operator cost are testable.
- Read power stays with the identity, not with a leaked inbox write token.

## Cons

- 14 days / 500 MiB still loses mail for a long holiday or a few large 1:1 attachments. UX must show "messages lost".
- Operators who tighten retention create a heterogeneous federation; clients must read advertised limits.

## Alternatives considered

### Unbounded until ack

Rejected: ADR-0015.

### Identity as mailbox address

Rejected: ADR-0008.

### Delivery capability also fetches

Rejected: a leaked contact capability would become a read oracle for that conversation's ciphertext (still encrypted, but a traffic and ciphertext-theft handle).

## Consequences

- README 53.8 skip-window vs retention is resolved for v1 defaults.
- Phase 4 document is normative for append/fetch/ack and capability consume rules.

## History

- 2026-09-19 — Accepted. Retention defaults and owner-only fetch.
