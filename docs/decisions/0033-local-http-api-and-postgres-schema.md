# ADR-0033: Local HTTP API and PostgreSQL schema

- **Status:** Accepted
- **Date:** 2026-09-20
- **Affects:** README.md sections 0, 49, 54; [`docs/protocol/08-api-and-persistence.md`](../protocol/08-api-and-persistence.md); ADR-0028 (routes and DDL were unfrozen there)

## Context

Phases 1–7 define what the untrusted server may know and how envelopes move. [ADR-0027](0027-protocol-first-development-order.md) kept HTTP paths and database tables last so they could not invent fields. [ADR-0028](0028-implementation-languages-and-libraries.md) already bound Axum 0.8, Tokio, PostgreSQL/sqlx, and Caddy as TLS terminator, but explicitly did not freeze routes or DDL.

This record freezes the v1 client-to-home HTTP surface and the Postgres schema. Both must be derivable from phases 1–5: no JSON protocol objects, no sender columns, no plaintext.

## Decision

### Listen and TLS

- `nemo-server` listens on **plain HTTP** at `127.0.0.1:8787` (override `NEMO_LISTEN`).
- **Caddy** (or an operator equivalent) terminates TLS in front of that port. The Rust process MUST NOT present a Web PKI certificate for client HTTPS.
- Federation **MUST NOT** share this port. S2S remains mTLS on `s2s_port` ([ADR-0032](0032-server-signing-key-and-federation-tls.md)). The in-process pump is valid until that listener exists.

### Encoding

- Request and response bodies for protocol objects are **canonical CBOR** (`application/cbor`) or raw padded/HPKE bytes (`application/octet-stream`).
- JSON MUST NOT be used for cards, envelopes, discovery, owner auth, or group frames.
- Path parameters and the headers below are lowercase hex of raw 32-byte values (or of a CBOR blob, for owner auth).

### Headers

| Header | Value |
| --- | --- |
| `nemo-owner` | hex(`MailboxOwnerAuth` CBOR) |
| `nemo-token` | hex(32-byte share token) |
| `nemo-cred-id` | hex(32-byte member `credential_id`) |
| `nemo-cred-secret` | hex(32-byte member `credential_secret`) |

`POST /v1/mailbox/fetch` and `/ack` carry owner auth in the **body** (that is the whole request). Other owner-authenticated routes use `nemo-owner`.

### Routes (`/v1`)

| Method | Path | Body in | Body out | Auth |
| --- | --- | --- | --- | --- |
| GET | `/bundle` | — | ServerBundle CBOR | none |
| POST | `/register` | ContactCard CBOR | empty 204 | none (card signature + binding) |
| GET | `/discovery/{identity_id}` | — | DiscoveryRecord CBOR | none |
| POST | `/prekeys` | opaque prekey blob | empty 204 | `nemo-owner` |
| GET | `/prekeys` | — | opaque blob | `nemo-token` |
| POST | `/envelopes` | OuterEnvelope | CBOR `{0: "ok", 1: seq}` or `{0: "queued"}` | `nemo-owner` |
| POST | `/mailbox/fetch` | MailboxOwnerAuth | CBOR array of `{0: seq, 1: inner_padded}` | body |
| POST | `/mailbox/ack` | MailboxOwnerAuth | empty 204 | body |
| POST | `/tokens/share` | empty | CBOR `{0: token}` | `nemo-owner` |
| POST | `/tokens/contact` | empty | CBOR `{0: token}` | `nemo-owner` |
| POST | `/revocation` | RevocationStatement | empty 204 | statement signature |
| POST | `/groups` | CBOR `{0: signing_pk, 1: cap, 2: hpke}` | `{0: group_id, 1: cred_id, 2: secret}` | none (creator holds the returned credential) |
| POST | `/groups/{group_id}/append` | CBOR `{0: cred_id, 1: secret, 2: type, 3: body}` | `{0: seq}` | member credential |
| POST | `/groups/{group_id}/invites` | CBOR `{0: cred_id, 1: secret, 2: GroupInvite}` | empty 204 | member credential |
| POST | `/groups/{group_id}/accept` | CBOR `{0: nonce}` | `{0: pending_id, 1: cred_id, 2: secret, 3?: binding}` | invite nonce |
| POST | `/groups/{group_id}/admit` | CBOR `{0–1: admitter cred, 2: GroupAdmit, 3: joiner_pk, 4: cap, 5: hpke}` | `{0: credential_id}` | member credential |
| POST | `/groups/{group_id}/fanout` | CBOR `{0–1: cred, 2: cap, 3: hpke}` | empty 204 | member credential |
| POST | `/groups/{group_id}/files/{token}` | opaque padded file | empty 204 | `nemo-cred-id` / `nemo-cred-secret` |
| GET | `/groups/{group_id}/files/{token}` | — | opaque file | `nemo-cred-id` / `nemo-cred-secret` |
| GET | `/wakeup` | — | WebSocket; empty binary wakes | `nemo-owner` |

Unknown discovery identities, unknown tokens, and generic delivery failure return **403 with an empty body** (same shape as Denied). Owner-auth failure is **401**. Duplicate register is **409**. Decode errors are **400**.

Body size limit: 18 MiB (A4 outer plus headroom).

### WebSocket

`GET /v1/wakeup` upgrades to WebSocket after `nemo-owner` auth. The server sends empty binary frames when that mailbox's head advances, coalesced to at most one frame per 10 s. **Poll of `/mailbox/fetch` is required** for v1. Waking MUST NOT carry size or type ([ADR-0020](0020-opaque-push-wakeup.md)).

### Persistence

- DDL lives in `crates/nemo-server/migrations/` and is the v1 schema. A later sqlx adapter MUST use these tables; it MUST NOT add columns that phases 1–5 did not require.
- The process MAY serve the routes above from an in-memory `HomeServer` / `GroupHost` that stores the same fields. That is a prototype runtime, not a second schema.
- No Redis. No JSONB protocol blobs that duplicate CBOR encodings already specified. `BYTEA` holds canonical CBOR or padded bytes.

## Pros

- HTTP and SQL cannot grow a shadow protocol: every stored column traces to a phase 1–5 object.
- Binary bodies keep padding and HPKE intact; JSON would force a second encoding and leak structure.
- Localhost + Caddy matches the self-host default (ADR-0025, ADR-0028).

## Cons

- Operators who want the app to bind `:443` itself are unsupported in v1.
- In-memory runtime and SQL schema can drift until sqlx is wired; tests must keep the HTTP handlers honest against `HomeServer`.
- Hex headers are larger than raw binary headers; chosen so ordinary reverse proxies and logs can carry them without binary-header bugs.

## Alternatives considered

### JSON REST with hex fields in objects

Rejected: a second encoding of cards and envelopes, and a temptation to add sender/display fields.

### sqlx in the same slice as the route freeze

Rejected as a crate dependency until a live database is required. The DDL is frozen without compiling `sqlx` unused.

### Serve S2S on the same Axum port

Rejected: [ADR-0032](0032-server-signing-key-and-federation-tls.md) pins mTLS to the peer signing key; Caddy/Web PKI must not be that identity.

### gRPC

Rejected in ADR-0028; restated: padded envelopes and self-host reverse proxies.

## Consequences

- Phase 8 document [`docs/protocol/08-api-and-persistence.md`](../protocol/08-api-and-persistence.md) is normative for paths and tables.
- ADR-0028's "routes and DDL remain unfrozen" is resolved by this record; the library choices there stand.
- Adding a client-visible column or JSON route needs a new record and a phase-1–5 citation.

## History

- 2026-09-20 — Accepted. Local HTTP `/v1` routes; PostgreSQL DDL; Caddy TLS; no JSON protocol objects.
- 2026-09-20 — Amendment 1: group invite / accept / admit / fan-out / file routes; `nemo-cred-*` headers for opaque file bodies. No new stored fields.
- 2026-09-20 — Amendment 3: `GET /wakeup` WebSocket implemented (empty binary frames, 10 s coalesce). No new stored fields.
