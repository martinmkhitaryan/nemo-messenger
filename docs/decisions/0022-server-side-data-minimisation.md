# ADR-0022: Server-side data minimisation — no presence, receipts, typing, analytics; minimal logs

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 6, 8, 24, 39, 40, 41

## Context

Most messengers implement presence ("online", "last seen"), typing indicators, read receipts and usage analytics as server-side features. Each is a stream of fine-grained behavioural metadata that the server accumulates and that a database compromise or a legal demand exposes. Operational logs are another such stream.

## Decision

The server implements **no feature whose purpose is to observe or report user behaviour**, and keeps operational data to the minimum needed to deliver ciphertext.

Not implemented server-side, in any form:

- presence / online status / last-seen;
- typing indicators;
- read or delivery receipts visible to the server (see ADR-0015: only transport acks exist server-side);
- contact lists (the only membership the server holds is group member capability sets, ADR-0017);
- per-user message counts, conversation statistics, behavioural profiles;
- server-side analytics of any kind as a dependency of normal operation.

If a client wants to show typing or read state, it sends it inside the encrypted channel as an application message, off by default, invisible to servers.

Logging:

- no message identifiers tied to identities, no content, no relationships, no request histories;
- IP addresses only where needed for abuse control, with short retention;
- receipt timestamps kept only for expiry and never returned to clients (ADR-0013);
- every retained log has a stated purpose, a retention limit and access control.

Optional diagnostics, if ever added, must be opt-in, minimal, privacy-preserving and separated from message infrastructure by a new decision record.

## Pros

- A server database compromise yields no behavioural profile of anyone.
- Nothing to hand over: the operator cannot be compelled to produce presence or read history.
- Operators, including hobbyist self-hosters, carry less sensitive data and less liability.
- Consistent with ADR-0001: the server is a carrier, not an observer.

## Cons

- Features users expect (online status, typing, blue ticks) are either absent or implemented end-to-end at extra message cost and with weaker guarantees (a client can lie).
- Debugging delivery problems is harder without request logs; operators must rely on aggregate counters.
- Abuse investigation is limited to capability- and peer-level signals.

## Alternatives considered

### Server-side presence and receipts with short retention

Rejected: retention limits do not change what an active compromise or a live demand can extract.

### Privacy-preserving aggregate analytics (differential privacy)

Deferred; would require its own record and must never be a dependency of normal operation.

## Consequences

- README section 8 "Data that should not exist server-side" is normative.
- Read receipts and typing indicators, if implemented, are application-layer messages inside the ciphertext (README 38.2 `user_receipt`).
- Discovery serves only cryptographic material and bindings, never social information (README section 6).

## History

- 2026-09-19 — Accepted.
