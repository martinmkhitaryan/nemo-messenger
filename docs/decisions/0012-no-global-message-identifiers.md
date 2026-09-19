# ADR-0012: No globally meaningful message identifiers

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 24, 38, 43.5

## Context

This is the identifier rule. A UUID or hash that is the same on Server A, Server B and in client logs lets an observer join those views into one message and, over time, a conversation.

## Decision

Servers assign **per-mailbox and per-group-stream sequence numbers** that are opaque outside that container.

- A `seq` on Alice's outbound federation queue is not the `seq` on Bob's mailbox. There is no shared message id.
- Deduplication identifiers inside the ciphertext are the client's business (ADR-0015 idempotency tokens are opaque and per-append, not global names).
- Application-level references (replies, reactions, delete-for-everyone) live inside the ciphertext, never on the envelope.

## Pros

- An observer who sees two servers cannot join their logs by message id.
- Ordering and acks still work (ADR-0015) because `seq` is local to one container.

## Cons

- Cross-server debugging cannot ask "where is message X?".
- Clients must maintain their own conversation-scoped ids inside the ciphertext.

## Alternatives considered

### Global UUID per envelope

Rejected: correlates traffic across mailboxes and servers.

### Hash of ciphertext as identifier

Rejected: the same ciphertext uploaded twice (retry) would be joinable across hops; also leaks equality of content.

## Consequences

- README section 38 and ADR-0015 remain the acknowledgement model.
- Related: ADR-0009 (sealed sender), ADR-0022 (logs must not invent a global id).

## History

- 2026-09-19 — Accepted. Identifier rule, numbered in reading order.
