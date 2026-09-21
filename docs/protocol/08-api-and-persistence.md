# Phase 8 — API and persistence

- **Status:** Complete (HTTP routes, frozen DDL, sqlx write-through when `DATABASE_URL` is set)
- **Date:** 2026-09-20
- **Phase:** 8 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** [04-delivery-protocol.md](04-delivery-protocol.md), [05-federation.md](05-federation.md)
- **Decisions:** [ADR-0028](../decisions/0028-implementation-languages-and-libraries.md), [0031](../decisions/0031-mailbox-retention-and-owner-auth.md), [0032](../decisions/0032-server-signing-key-and-federation-tls.md), [0033](../decisions/0033-local-http-api-and-postgres-schema.md)

This document is the HTTP and storage mapping of phases 1–5. It MUST NOT add a server-stored field that those phases did not require. Tor/Arti, cover traffic, Maximum mode, and group calls remain nice-to-have and have no routes here.

---

## 1. Process

```text
client  --HTTPS-->  Caddy (TLS)  --HTTP-->  127.0.0.1:8787  nemo-server
peer    --mTLS-->   s2s_port (not this listener; ADR-0032)
```

- Default bind: `127.0.0.1:8787`. Override: `NEMO_LISTEN`.
- Reference deploy: container + PostgreSQL + Caddy + optional coturn ([ADR-0028](../decisions/0028-implementation-languages-and-libraries.md)).
- This implementation serves from in-memory `HomeServer` / `GroupHost`. When `DATABASE_URL` is set, that state is loaded from the SQL below at start and flushed after successful mutating HTTP requests. Unset `DATABASE_URL` keeps the prototype in-memory runtime.

Body limit: **18 000 000** bytes.

---

## 2. Encoding and errors

| Rule | Detail |
| --- | --- |
| Protocol objects | Canonical CBOR, `Content-Type: application/cbor` |
| Opaque blobs | Prekeys, OuterEnvelope, padded inners: `application/octet-stream` when not a CBOR map |
| JSON | Forbidden for protocol objects |
| Path ids | 64 lowercase hex chars = 32 raw bytes |
| `nemo-owner` | hex of `MailboxOwnerAuth` CBOR |
| `nemo-token` | hex of 32-byte share token |
| `nemo-cred-id` / `nemo-cred-secret` | hex of live member credential; group-file GET/POST only |

| Status | When |
| --- | --- |
| 204 | Success, no body |
| 200 | Success with CBOR or octet-stream |
| 400 | Decode / wire / fetch limit |
| 401 | Mailbox owner authentication failed |
| 403 empty | Denied, unknown identity, unknown token, generic delivery failure |
| 409 | Identity already registered |

403 MUST NOT distinguish unknown vs expired vs burned vs empty prekey stock ([ADR-0031](../decisions/0031-mailbox-retention-and-owner-auth.md)).

`seq` in HTTP responses is a **per-container** mailbox or stream sequence ([ADR-0012](../decisions/0012-no-global-message-identifiers.md)). Receipt times are not returned ([ADR-0013](../decisions/0013-internal-server-timestamps.md)).

---

## 3. Routes

Base path: `/v1`.

### 3.1 `GET /bundle`

Returns the host `ServerBundle` (phase 5). Clients check `server_id` / HPKE against the contact-card binding; Web PKI on Caddy is not identity.

### 3.2 `POST /register`

Body: `ContactCard`. Server verifies the card, that `binding.server_id` is this host, creates the mailbox, stores discovery keys, and registers the card's share token. 204.

`POST /binding`: same body. Observes a `HomeServerBinding` for an identity this host already knows ([phase 1](01-security-model.md) §5.3). Higher `seq` is stored; if `binding.server_id` is not this host, the mailbox is disabled (README §11). Unknown identity: 403 empty. No extra table.

### 3.3 `GET /discovery/{identity_id}`

Returns `DiscoveryRecord` CBOR:

| Key | Field |
| --- | --- |
| 0 | version = 1 |
| 1 | identity_public_key (32) |
| 2 | revocation_public_key (32) |
| 3 | HomeServerBinding map |
| 4 | optional RevocationStatement bytes |

Unknown id: 403 empty. Not a search directory ([ADR-0006](../decisions/0006-invite-first-discovery.md)).

### 3.4 Prekeys

- `POST /prekeys` — owner header; body is an opaque one-time prekey blob. 204.
- `GET /prekeys` — `nemo-token` share token. Does not burn the token. Reserves at most one blob per token. Empty stock: 403.

### 3.5 `POST /envelopes`

Owner header. Body: `OuterEnvelope` (HPKE to a destination `server_id`). Same-server ingest returns `{0: "ok", 1: seq}`. Other destination: `{0: "queued"}` onto the outbound federation queue (phase 5).

The owner signature is required so a client cannot use this host as an open relay ([phase 4](04-delivery-protocol.md) Alice → her home).

### 3.6 Mailbox fetch and ack

`POST /mailbox/fetch` and `POST /mailbox/ack`: body is `MailboxOwnerAuth` (`ts` Unix seconds, rejected if older than 120 s).

Fetch response: CBOR **array** of maps `{0: seq, 1: inner_padded}`. `seq > cursor`, at most `limit` (1..=256). Does not delete. Ack with `cursor = N` drops rows `seq ≤ N`.

### 3.7 Tokens

`POST /tokens/share` and `POST /tokens/contact`: owner header, empty body. Response `{0: token}` (32 bytes). Share TTL default 30 minutes ([ADR-0029](../decisions/0029-cryptographic-identifiers-and-encodings.md)). Contact capabilities persist until rotation + 72 h grace.

### 3.8 `POST /revocation`

Body: `RevocationStatement`. Home wipes mailbox material and returns hosted group ids so the group host can append a host-only `0x13` ([phase 1](01-security-model.md) §5.4). 204 even when no groups are hosted here.

### 3.9 Groups

`POST /groups` body `{0: signing_pk, 1: delivery_capability, 2: home_hpke_public}`. Returns `{0: group_id, 1: credential_id, 2: credential_secret}` for the creator's live member credential.

`POST /groups/{group_id}/append` body `{0: credential_id, 1: credential_secret, 2: type uint, 3: body}`. `type` is the envelope `MessageType`. The host fans out OuterEnvelopes to member mailboxes; response `{0: seq}` is the **stream** sequence.

`POST /groups/{group_id}/invites` body `{0: cred_id, 1: secret, 2: GroupInvite bytes}`. The host stores a client-signed invite; it does not mint one. 204.

`POST /groups/{group_id}/accept` body `{0: nonce}`. Consumes the invite. Response `{0: pending_id, 1: reserved credential_id, 2: secret}` and optional `{3: invitee_binding}`. The reserved credential is inert until admit.

`POST /groups/{group_id}/admit` body `{0: admitter cred_id, 1: secret, 2: GroupAdmit bytes, 3: joiner signing_pk, 4: joiner delivery_capability, 5: joiner home HPKE}`. Activates the reserved id. Response `{0: credential_id}`. Bound-invite identity proof stays in-band; the host never sees `InviteeProof`.

`POST /groups/{group_id}/fanout` body `{0: cred_id, 1: secret, 2: delivery_capability, 3: home_hpke_public}`. 204.

`POST /groups/{group_id}/files/{token}` and `GET` of the same path: opaque padded object. Auth is headers `nemo-cred-id` and `nemo-cred-secret` (the body is the file). Upload requires a prior `AttachmentReserve` append naming that token; first upload wins; fetch needs a **live** member credential and the token. Reserved and live file bytes count toward a **4 GiB** per-group budget, separate from the 2 GiB stream cap ([ADR-0031](../decisions/0031-mailbox-retention-and-owner-auth.md)).

### 3.10 Wakeup (optional)

`GET /v1/wakeup` is a WebSocket. After `nemo-owner` auth, the server sends **empty binary frames** when that mailbox's head advances, coalesced to at most one frame per 10 s. The frame MUST NOT carry size or type ([ADR-0020](../decisions/0020-opaque-push-wakeup.md)). Poll of `/mailbox/fetch` remains required.

### 3.11 TURN credentials

`POST /v1/turn`: empty body, `nemo-owner` auth. Response `{0: url, 1: username, 2: credential, 3: ttl_secs}`. Username is `{expiry}:{random hex}` and MUST NOT contain `identity_id`. Credential is coturn REST HMAC-SHA1 ([ADR-0035](../decisions/0035-ephemeral-turn-credentials.md)). No table.

---

## 4. PostgreSQL

Normative DDL: [`crates/nemo-server/migrations/0001_init.sql`](../../crates/nemo-server/migrations/0001_init.sql).

| Table | Why it exists |
| --- | --- |
| `server_identity` | This host's HPKE + signing keys (phase 5) |
| `identities` | Discovery public keys, binding, optional revocation, mailbox cursor (phases 1–2, 4) |
| `mailbox_rows` | Padded inners, seq, internal expiry, idempotency (phases 3–4) |
| `tokens` | Share and contact capabilities (phase 4) |
| `prekeys` | Finite public prekey stock (phase 2) |
| `groups`, `group_creds`, `group_invites`, `group_pending`, `group_stream`, `group_files` | Hosted MLS stream (phases 4–5, ADR-0017/0018/0019) |
| `memberships` | Which identities this home has seen in groups it hosts (revocation fan-out) |
| `peers`, `outbound` | Federation pins and 14-day queue (phase 5, ADR-0031) |
| `hpke_seen` | HPKE encapsulation replay window (phase 5) |

MUST NOT exist: plaintext, nicknames, contact lists beyond group credentials, sender columns, presence, receipts, analytics ([phase 1](01-security-model.md) §6.3).

`received_at` / `expires_at` are operator-internal. They MUST NOT appear on `/mailbox/fetch`.

Retention defaults remain 14 days / 500 MiB for mailboxes ([ADR-0031](../decisions/0031-mailbox-retention-and-owner-auth.md)). Group stream bound: 30 days / 2 GiB (phase 4).

---

## 5. Queues, caches, operations

- **Outbound queue:** `outbound` rows; backoff 1 s … 300 s; drop after 14 days. The S2S mTLS listener on `s2s_port` (ALPN `nemo-s2s/1`, pinned Ed25519) forwards due rows; in-process `pump` remains for tests. Operator pin files: `NEMO_PEERS_DIR` of ServerBundle CBOR.
- **Caches:** none required. No Redis ([ADR-0028](../decisions/0028-implementation-languages-and-libraries.md)).
- **Push:** opaque FCM wake is operator-optional ([ADR-0020](../decisions/0020-opaque-push-wakeup.md)); not a table of message metadata.
- **TURN:** coturn sidecar; this API does not issue long-lived credentials in v1.

---

## 6. Phase completion

v1 client-to-home HTTP, error shapes, and Postgres DDL are specified and implemented. Group join and files are on `/v1`. sqlx write-through uses the frozen tables when `DATABASE_URL` is set. The client talks to those routes through `nemo-core::HomeSession` (transport trait; no `nemo-server` link at runtime). Remaining operational work that does **not** unfreeze this document: FCM. Those MUST keep the tables and routes above.
