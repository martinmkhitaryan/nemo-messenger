# ADR-0027: Protocol-first development order; APIs and schemas come last

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 54, 63

## Context

Projects that start with REST endpoints and database tables tend to freeze accidental choices (identifiers, field names, what the server stores) into the protocol. For a system whose entire value is what the server does *not* know, the data the server stores must be derived from the protocol, not the other way round.

## Decision

Development proceeds in the order below. A phase does not start until the previous one has a written specification that the decision records agree with.

```text
1. Security model        threat model, invariants, trust boundaries, identity lifecycle, abuse model
2. Cryptographic protocol identity key and signed keys, contact card, PQXDH + Double Ratchet, MLS, revocation
3. Envelope protocol     sealed sender (0009), padding (0010), version (0011), seq (0012), timestamps (0013), HPKE nesting, ttl_bucket (0014); attachment bucket set (0019)
4. Delivery protocol     share tokens, contact capabilities, member credentials, mailbox log, acks, retries, deduplication; attachments as envelopes (0019)
5. Federation            server authentication, server-to-server append, group fan-out, expiry, rate limits
6. Privacy transport     batching, jitter, cover traffic, Tor (may proceed in parallel with 2–5)
7. Application protocol  text, attachments, reactions, group changes, disappearing messages, calls signaling
8. API and persistence   HTTP/WebSocket APIs, database schema, queues, caches, operations
```

- Phases 1–5 define what the server may know. Phase 8 may store nothing that phases 1–5 did not require.
- Prototyping is allowed and encouraged at any phase, but a prototype's API or schema is not a specification.
- Decision records are written when a phase makes a choice, not after implementation.

## Pros

- Server storage and APIs are provably derived from protocol needs, which is the only way to keep the "data that should not exist server-side" list honest.
- Protocol specifications exist before code, so independent implementations and audits have something to read.
- Expensive mistakes (identifier design, what an ack means, whether an envelope has a sender) are caught in documents rather than migrations.

## Cons

- Slower to a demo; nothing user-visible exists until phase 7.
- Requires discipline to keep prototypes from becoming the spec.
- Some decisions need implementation feedback (library ergonomics, performance); the order allows prototypes for that but they must not be shipped.

## Alternatives considered

### API-first / schema-first

Rejected: freezes server-visible data before the protocol says what is allowed to be visible.

### Build a working client-server MVP, then retrofit protocol documents

Rejected: retrofits rarely remove fields already in production.

## Consequences

- README section 54 is the normative phase list.
- Each phase ends with a specification document under `docs/` and, where choices were made, new or amended decision records.
- Phase 8 cannot introduce a server-stored field without pointing to the phase 1–5 requirement that needs it.

## History

- 2026-09-19 — Accepted. Protocol-first development order; APIs and schemas last.
