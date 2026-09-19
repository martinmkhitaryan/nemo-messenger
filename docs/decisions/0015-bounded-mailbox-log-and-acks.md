# ADR-0015: Mailbox as a bounded opaque log; three kinds of acknowledgement

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 7, 9, 38, 43.5, 48

## Context

Offline delivery needs server-side storage of ciphertext. How that storage behaves (ordering, bounds, deletion, what "delivered" means) determines how clients handle reordering, loss and duplicates, and what a server can infer from acknowledgements. The original draft asked for "enough identifiers and acknowledgement semantics" without fixing them.

## Decision

Every mailbox and every group stream (ADR-0017) is a **bounded, append-only, opaque log**.

```text
append(capability, envelope, idempotency_token)   -> seq
fetch(mailbox_capability, cursor, limit)          -> envelopes with seq
ack(mailbox_capability, up_to_seq)                -> deletes acknowledged envelopes
```

- The server assigns a monotonically increasing `seq` per container. It is meaningful only inside that container (ADR-0012).
- Appends are idempotent by a client-supplied opaque token; retransmissions do not duplicate.
- Retention is bounded by size and age. When exceeded, the **oldest envelopes are dropped**. Attachment envelopes (ADR-0019) count toward the same bound; a few large files can evict older text. The client detects the gap from `seq` and shows "messages lost". The cryptographic protocols survive dropped messages (Double Ratchet skips; MLS tolerates within policy), so loss degrades the conversation rather than breaking it.
- A sender may request earlier expiry with `ttl_bucket` (ADR-0014).
- Acknowledgements are three distinct things and are never conflated:

```text
transport_ack   client -> its own server: "delete up to seq N". Means: ciphertext stored durably on the client.
protocol_ack    inside the encrypted channel, recipient -> sender. Means: decrypted and processed. Optional.
user_receipt    application feature (read receipt). Off by default. Never visible to any server.
```

- Servers know only transport acks. A transport ack must never be interpreted or displayed as "read".

## Pros

- Clean reordering, duplicate and loss semantics without global identifiers.
- Bounded storage gives operators a predictable cost and removes "the server as archive" by construction (ADR-0001).
- Delivery indicators that mean something can be built on protocol acks without trusting the server.
- Sequence gaps make server-side dropping (malicious or not) visible to the user.

## Cons

- Long absences lose messages. There is no "fetch older" because the server never had them for long.
- Protocol acks cost a message per message if used naively; clients should batch them.
- Retention defaults are a policy the operator sets; users on different servers experience different loss thresholds.

## Alternatives considered

### Unbounded retention until acknowledged

Rejected: turns the server into an archive and an attack target; contradicts ADR-0001.

### Server-side "delivered" and "read" status

Rejected: read state is metadata the server must not have (ADR-0022). Delivery state beyond "deleted after transport ack" is not kept.

### Global message identifiers

Rejected: correlate traffic across mailboxes and servers (ADR-0012).

## Consequences

- Client UX has a "Messages lost" state (README section 48).
- Retention limits are part of server capabilities served by discovery, so clients can warn users.
- The Double Ratchet skipped-key window and the MLS out-of-order policy must be sized against the retention defaults (open item in README 53.8).

## History

- 2026-09-19 — Accepted. Mailbox as a bounded opaque log; three kinds of acknowledgement.
