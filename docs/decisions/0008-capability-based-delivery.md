# ADR-0008: Capability-based delivery addressing

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 7, 20, 22, 23, 43.4, 46.4

## Context

A delivery service must be able to route an envelope to a recipient. The obvious address is the recipient's identity (or a hash of it). That makes every envelope carry "to Bob" in the clear for the server, turns the mailbox into an identity-keyed record, and lets anyone who knows Bob's identity flood his mailbox.

## Decision

Envelopes are addressed by **delivery capabilities**: random 256-bit tokens that the recipient's server maps to a mailbox. The token, not the identity, is the address.

Classes:

```text
Share token (ADR-0007)
  minted per share; one-time; short TTL; connect purpose; required to fetch a prekey;
  consume-on-first-envelope

Contact capability
  one per (conversation, recipient); exchanged only inside an established encrypted session;
  never published; rotated in-band with a grace period

Member credential (ADR-0018)
  issued by a group hosting server on admit; authorises stream append and group-file fetch;
  cannot mint invites; not a mailbox delivery capability;
  revoked with no grace when the host admits a RemoveBundle for that member (ADR-0018)
```

Rules:

- A sender never addresses an envelope to an identity. It addresses a capability it was given.
- After first contact, the share token is burned and a private contact capability is used.
- Group fan-out (ADR-0017) delivers to members' contact capabilities. Stream append uses the member credential (ADR-0018).
- A **delivery** capability grants the right to append to a mailbox. It grants nothing else: no read, no identity, no group membership change.
- The server stores `capability -> mailbox, state, expiry` and nothing about who holds it.
- Unknown capabilities are rejected with a constant-time response to prevent enumeration.
- Capabilities are not transferred between servers; migration issues fresh ones and distributes them in-band.

## Pros

- The server cannot join two conversations of the same recipient by address; each conversation has its own token.
- Leaked or spammed tokens are rotated without affecting other conversations or the identity.
- Spam control without knowing senders: share tokens are one-shot; contact capabilities are rate-limited per token.
- Together with sealed sender (ADR-0009), an envelope in transit reveals neither sender nor recipient identity to servers.

## Cons

- More tokens to manage: one per conversation per recipient, plus rotation logic and grace periods.
- The server can still correlate successive generations of the same conversation's token (the replacement arrives in the same mailbox); this is accepted.
- Timing analysis across capabilities of the same mailbox remains possible; capabilities address identity leakage, not traffic analysis (ADR-0021).
- A capability is a bearer token; whoever holds it can append. Theft means spam into one conversation until rotation, not impersonation.

## Alternatives considered

### Address by identity id

Rejected: exposes recipient identity to every server on the path and to the recipient's own server for every message, and allows flooding by anyone who knows the id.

### Single identity-wide capability

Rejected: equivalent to an identity id with a different name; no cross-conversation separation.

### Per-message one-time capabilities

Considered for maximum unlinkability. Rejected for v1: requires a large pre-shared pool and complicates offline delivery. Could be layered on later.

## Consequences

- Contact card (ADR-0006) contains a just-minted share token (ADR-0007), not a durable introduction capability.
- Data model has `DeliveryCapability` and `MemberCredential` (ADR-0018).
- Abuse controls in README 46.4 are specified per capability class and per peer server, never per sender.

## History

- 2026-09-19 — Accepted. Capability-based delivery addressing.
