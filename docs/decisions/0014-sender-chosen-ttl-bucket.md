# ADR-0014: Sender-chosen TTL bucket on the inner envelope

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 10, 34, 43.5, 44.1, 64.1

## Context

This is the TTL rule. Call invites and disappearing messages must not sit in a mailbox for the default retention (hours or days) if they are meaningless after a minute.

## Decision

The **inner** envelope may carry a `ttl_bucket` from a **small fixed set** of values, asking the recipient's server to drop the envelope if still undelivered.

- Only the recipient's server sees it (it lives inside the HPKE layer, ADR-0016).
- `expires_at = min(receipt + ttl_bucket, server retention)` and is never returned to clients (ADR-0013).
- The set is small (for example 1 minute, 1 hour, 1 day, default) so the value leaks little. Adding values is a version bump (ADR-0011).
- `call_invite` uses the smallest bucket (ADR-0024).
- Disappearing messages may set a matching bucket so undelivered ciphertext does not outlive the intent (README 44.1). Attachments of those messages use the same retention idea (README 34).
- This is the only piece of application intent visible at the envelope layer.

## Pros

- Stale call invites and expired disappearing ciphertext do not occupy mailboxes or wake devices late.
- The sender's home server does not see the TTL.

## Cons

- A small set still leaks a coarse "this was urgent / ephemeral / normal" bit to the recipient's server.
- A malicious sender can pick a short TTL to make offline delivery fail; that is a liveness issue, not a confidentiality one.

## Alternatives considered

### No sender TTL; only server retention

Rejected: call invites would wake devices hours later; disappearing-message ciphertext would outlive the setting.

### Free-form TTL in milliseconds

Rejected: high-resolution values leak the application feature and the user's setting.

### TTL on the outer envelope

Rejected: the sender's server would see the bucket.

## Consequences

- README 43.5 and 10 list `ttl_bucket` on the inner envelope.
- Related: ADR-0013 (expires_at internal), ADR-0024 (calls), README 44.1 (disappearing messages).

## History

- 2026-09-19 — Accepted. TTL-bucket rule, numbered in reading order.
