# ADR-0035: Ephemeral TURN credentials from the home server

- **Status:** Accepted
- **Date:** 2026-09-20
- **Affects:** README.md sections 0, 64; [`docs/protocol/08-api-and-persistence.md`](../protocol/08-api-and-persistence.md); [ADR-0024](0024-voice-call-architecture.md); [ADR-0033](0033-local-http-api-and-postgres-schema.md) (adds one owner-auth route, no table)

## Context

[ADR-0024](0024-voice-call-architecture.md) requires TURN credentials that are issued per call by the caller's home server and are not tied to identity. Discovery may advertise TURN. [ADR-0033](0033-local-http-api-and-postgres-schema.md) froze `/v1` without a TURN route or a credentials table. Static sidecar passwords (`nemo:nemo`) were a test default, not the product path.

Coturn's REST/HMAC time-limited credentials already exist: username `{unix_expiry}:{random}`, password `HMAC-SHA1(shared_secret, username)` as standard Base64. That needs a shared operator secret with coturn, not a row per call.

## Decision

- **Route:** `POST /v1/turn`. Empty body. Auth is `nemo-owner` (same 120 s window as other owner routes). Response CBOR `{0: url text, 1: username text, 2: credential text, 3: ttl_secs uint}`.
- **Username** is `{expiry_unix}:{32 hex random}`. It MUST NOT contain `identity_id` or any stable mailbox identifier.
- **Credential** is coturn REST HMAC-SHA1 over the username with `NEMO_TURN_SECRET`. If that env is unset, the process uses a random secret for this run so issued creds are still ephemeral and not identity-tied; they will only work against a coturn that shares the same secret.
- **TTL** is 3600 seconds. Clients MUST fetch new credentials for each call, not reuse a previous response as a long-lived account.
- **URL** is `NEMO_TURN_URL` (default `turn:127.0.0.1:3478`). This record does not add TURN fields to `ServerBundle`; advertising TURN in discovery can come later without a table.
- **No Postgres table.** Credentials are stateless HMAC. DDL from ADR-0033 is unchanged.
- **Static `NEMO_TURN_USER` / `NEMO_TURN_PASS`** remain a local fallback when the home route is unreachable (tests, broken network). They are not the product issuance path.

## Pros

- Matches ADR-0024 without storing call state on the untrusted home.
- Coturn operators already know `--use-auth-secret`.
- Owner auth rate-limits minting the same way as share tokens.

## Cons

- HMAC-SHA1 is what coturn REST specifies; we do not invent a stronger TURN MAC.
- A process-local secret when `NEMO_TURN_SECRET` is unset will not match a separately started coturn until the operator sets the env on both.
- TURN URL is env, not a signed bundle field, so a lying home can point the client at a TURN the contact card never mentioned. ICE is still relay-only; that is the same operator-trust as hosting TURN at all (ADR-0024).

## Alternatives considered

### Store username/password rows in Postgres

Rejected: a table would retain who called and when. HMAC is enough for coturn and keeps the home blind of call records.

### Put TURN URL on ServerBundle

Deferred: would change a signed object and force every home to re-sign. Env is enough for v1 same-operator TURN.

### Keep only static `nemo:nemo`

Rejected by ADR-0024: credentials must be per-call and not identity.

## Consequences

- `nemo-server` grows `POST /v1/turn`. Clients ask for credentials at call start.
- Operators who enable the compose `calls` profile SHOULD set the same `NEMO_TURN_SECRET` on `nemo-server` and coturn `--use-auth-secret`.
- Cross-server TURN (callee's home vs caller's home) remains the open item in README 53.12.

## History

- 2026-09-20 — Proposed.
- 2026-09-20 — Accepted.
