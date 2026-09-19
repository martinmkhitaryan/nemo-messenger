# ADR-0009: Sealed sender — no sender field on any server-visible envelope

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 7, 10, 22, 43.5, 45, 46.4

## Context

This is the sealed-sender rule. The other envelope rules are ADR-0010–0014, so this can change without reopening padding, versioning, identifiers, timestamps or TTL.

A delivery service that stores `from` and `to` can reconstruct the social graph from envelopes alone. Abuse handling is the usual reason people add a sender field "temporarily".

## Decision

The envelope that any server stores or forwards has **no sender field of any kind**.

- Sender authentication lives inside the ciphertext (Double Ratchet header, MLS sender data).
- Server-side abuse control operates on delivery capabilities (ADR-0008) and authenticated peer servers (ADR-0016), never on senders.
- Group-stream appends are authenticated as a current **member credential** (ADR-0018), not as an identity or a mailbox delivery capability (ADR-0017).
- There is no debug, logging, or "temporary" exception.

## Pros

- An envelope in transit, together with a capability (ADR-0008), reveals neither sender identity nor recipient identity.
- Prevents a sender field added for debugging from becoming permanent.
- Matches ADR-0001: the server is a carrier, not an observer of who talks to whom.

## Cons

- The server cannot rate-limit or ban a sender. Flooding is handled per capability and per peer server.
- Debugging delivery problems is harder: operators see "capability T received a blob", not "Alice sent to Bob".

## Alternatives considered

### Sender field for abuse handling

Rejected: exposes the social graph to every server on the path.

### Encrypted sender that the recipient's server can open

Rejected: Server B would learn the sender of every delivery, which is the graph on that server.

## Consequences

- README section 43.5 and the sealed-sender invariant in section 45 remain normative.
- Related: ADR-0008 (capabilities), ADR-0012 (no global IDs), ADR-0022 (no behavioural logs).

## History

- 2026-09-19 — Accepted. No sender field on any server-visible envelope.
