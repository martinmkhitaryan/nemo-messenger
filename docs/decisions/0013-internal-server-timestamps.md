# ADR-0013: Server timestamps are internal and never returned to clients

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 8, 39, 43.5

## Context

This is the timestamp rule. Fine-grained receipt times let a server or a compelled log reconstruct conversation timing. Returning them to clients also trains clients to trust the server's clock.

## Decision

Servers keep receipt times **only as long as needed for expiry** and **never return them to clients**.

- Clients carry their own timestamps inside the ciphertext. Displayed "sent at" is the sender's claim.
- Retention of receipt times is the minimum needed to compute `expires_at = min(receipt + ttl_bucket, retention)` (ADR-0014). Prefer coarse clocks (seconds or coarser), not millisecond traces.
- Operational logs follow ADR-0022: no message timing retained longer than operationally required.

## Pros

- Timing metadata is not a client-visible API and is not a durable record.
- Clients cannot be tricked into displaying a server-chosen send time as fact.

## Cons

- Debugging "when did this arrive?" requires operator-only, short-lived data.
- "Sent at" can be forged by a malicious sender; that is accepted (the sender already chose the plaintext).

## Alternatives considered

### Return `created_at` to clients for UX

Rejected: trains users to trust the server and retains a timing channel.

### Store millisecond receipt times indefinitely for abuse forensics

Rejected: contradicts ADR-0022 and creates a compelled timing archive.

## Consequences

- Envelope data model has no client-visible `created_at` (README 43.5).
- Related: ADR-0014 (`ttl_bucket` / `expires_at`), ADR-0022 (logs).

## History

- 2026-09-19 — Accepted. Timestamp rule, numbered in reading order. Coarse-clock guidance for maximal security.
