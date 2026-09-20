# Phase 4 — Delivery protocol

- **Status:** Complete
- **Date:** 2026-09-19
- **Phase:** 4 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** [03-envelope-protocol.md](03-envelope-protocol.md)
- **Decisions:** [ADR-0007](../decisions/0007-one-time-share-tokens.md), [0008](../decisions/0008-capability-based-delivery.md), [0015](../decisions/0015-bounded-mailbox-log-and-acks.md), [0017](../decisions/0017-server-hosted-group-streams.md), [0018](../decisions/0018-group-join-and-member-credentials.md), [0019](../decisions/0019-attachment-as-padded-envelope.md), [0031](../decisions/0031-mailbox-retention-and-owner-auth.md)

This document is the delivery state machine. HTTP paths are phase 8. "The client sends X to its home server" is a semantic, not a URL.

---

## 1. Containers

| Container | Who assigns `seq` | Who may append | Who may fetch |
| --- | --- | --- | --- |
| Mailbox | Home server of the recipient | Anyone presenting a live **delivery** capability or unused **share token** (via S2S InnerEnvelope) | Owner only (identity-authenticated to home server) |
| Group stream | Hosting server | Live **member credential**, or host-only verified `RevocationStatement` | Fan-out to member mailboxes; members do not subscribe to the host |

`seq` is `uint64`, starts at 1, monotonic, never reused, not global ([ADR-0012](../decisions/0012-no-global-message-identifiers.md)).

---

## 2. Capability classes

All tokens are 32-byte CSPRNG values. The server stores `token -> mailbox, class, expiry, state` and nothing about who holds the token. Unknown tokens: constant-time failure, same shape as expired.

### 2.1 Share token (`connect`)

- Minted with a contact card ([phase 2](02-cryptographic-protocol.md)).
- Authorises **one** InnerEnvelope append to the owner's mailbox.
- TTL: 5 min / 30 min / 1 hour (default 30 min).
- Prekey fetch with the token does not consume it; reserves at most one prekey.
- First accepted append **burns** it. Later appends: unknown-token.
- Race: first envelope wins.

### 2.2 Contact capability

- Created by the recipient, sent **inside** an established encrypted session (phase 7).
- One per `(conversation, recipient)`.
- Authorises append to that mailbox until rotated.
- Rotation: recipient sends a new capability in-band; old one remains valid for **72 hours** (grace), then the server MUST drop it if the owner has confirmed rotation via owner-auth.
- Rate-limit: recommended 30 appends / minute / capability; excess returns a generic failure (not "rate limited for Alice"). Exact numbers may be advertised; MUST be per-capability, never per-sender.

### 2.3 Member credential

- Reserved on group accept (`credential_id` 32 bytes, `credential_secret` 32 bytes).
- Activated on admit. Until then: no append, no fetch, no fan-out, no invites.
- Live credential authorises: group-stream append, `AttachmentReserve` / file upload, group-file fetch (with `fetch_token`), fan-out refresh `{delivery_capability, home_server}`.
- Does **not** mint invites (invites are client-signed).
- Revoked with no grace when the host admits a `RemoveBundle` naming that id, **after** fan-out of that bundle to the pre-revoke set.

### 2.4 Mailbox owner

- Fetch and transport-ack require a signature with the identity key, context `mailbox-owner`, payload canonical CBOR `{mailbox_hint, cursor, limit, ts}` with `ts` Unix seconds, rejected if older than 120 seconds. `mailbox_hint` is a server-local handle, not `identity_id` on the wire if it can be avoided; v1 MAY use `identity_id` on the owner channel only (the home server already knows the registration).
- Owner channel is **only** to the home server.

---

## 3. Mailbox operations

### 3.1 append (Server B, after opening HPKE)

Inputs: `delivery_capability` or share token, `ttl_bucket`, `idempotency_token`, `padded_message_envelope`.

1. If token unknown/expired/burned: constant-time fail.
2. If `idempotency_token` already stored for this mailbox: return the original `seq` (no second row).
3. Assign next `seq`. Store envelope. Set `expires_at` from `ttl_bucket` ([phase 3](03-envelope-protocol.md)).
4. If share token: burn it.
5. If mailbox exceeds 500 MiB or contains receipts older than 14 days (or the advertised stricter limit): delete lowest `seq` still present until within bounds. Attachments count.
6. Optionally enqueue an opaque push wake (phase 6/8). Wake payload MUST NOT include size or type.

Return: `seq` to the **forwarding peer** (Server A), not to Alice as a global id. Alice's transport ack is with **her** server about **her** outbound queue (below).

### 3.2 fetch (owner)

`fetch(cursor, limit)` → envelopes with `seq > cursor`, in order, `limit` ≤ 256. Does not delete.

### 3.3 ack (owner)

`ack(up_to_seq)` deletes stored envelopes with `seq ≤ up_to_seq`. Means: ciphertext is durable on the client. MUST NOT be treated as read.

---

## 4. Outbound queue (Server A)

Alice posts an OuterEnvelope to A.

1. A verifies Alice's owner/session (same identity that registered).
2. A enqueues `(destination_server_id, hpke_ciphertext, alice_idempotency)`.
3. A forwards to B on the phase-5 channel. Retries are phase 5.
4. When B accepts, A may drop the outbound row.
5. If B is unreachable, A keeps the row until the **outer** expires (default 14 days) then drops. Alice's client MUST be able to retry a new OuterEnvelope (new HPKE, same inner idempotency) if she still has the InnerEnvelope.

Alice–A "transport ack" is: A accepted the OuterEnvelope into the outbound queue. That is not delivery to Bob.

---

## 5. Group stream operations

### 5.1 append

Authorised by `credential_id` + `credential_secret` (constant-time compare). Host-only `RevocationStatement` is the exception: verified signature, no member credential.

Assign `seq`. Persist object. Fan out a verbatim copy ([phase 3](03-envelope-protocol.md) type preserved) to each fan-out entry `{delivery_capability, home_server}` using OuterEnvelope as if the host were a sending client of that member's home server. The host is Server A in that hop; it MUST NOT put a sender field.

If the object is `RemoveBundle`: verify sidecar, fan out to **pre-revoke set including the named credential**, then revoke that live credential and delete its fan-out entry.

If `SigningKeyReplace`: replace that credential's filter key only.

If `AttachmentReserve`: reserve `fetch_token → (credential_id, seq)` atomically; reject duplicate tokens.

Retention: 2 GiB or 30 days for stream objects.

### 5.2 Fan-out refresh

Live member replaces `{delivery_capability, home_server}` under their credential. Not group migration.

### 5.3 Join (semantic; bytes of invite objects may be CBOR in phase 7)

1. Invite: member-signed, one-time, same TTL set as share tokens, stored on host, host cannot mint.
2. Accept: pending join, reserved `credential_id`, no fan-out. Invite consumed.
3. Admit: member-signed; bound invites need in-band identity proof; discovery revoke check; activate **same** reserved id; register signing key; add fan-out **before** fanning admit-associated stream objects (or Welcome in-band).

Pending joins expire with the invite TTL.

### 5.4 Group files

Upload: after reserve, body length MUST equal the reserved A* bucket. First successful upload immutable. Fetch: S2S from member home server with `fetch_token` **and** live member credential. Token alone fails (constant-time). Removed members cannot fetch.

---

## 6. Three acknowledgements

| Kind | Path | Meaning |
| --- | --- | --- |
| `transport_ack` | Owner → home server `ack(up_to_seq)` | Ciphertext stored on client |
| `protocol_ack` | Inside E2EE (phase 7) | Decrypted and processed; optional; batch |
| `user_receipt` | Inside E2EE, off by default | Read; never on a server |

---

## 7. Loss

If `fetch` sees a `seq` gap versus the last acked `seq + 1`, the client MUST show "messages lost". DR/MLS continue. Do not pretend the gap will be filled.

---

## 8. Inputs to phase 5

Phase 5 MUST specify: how Server A authenticates to Server B; HPKE/replay on that hop; retry, backoff, and expiry of the outbound queue; per-peer rate limits and refuse-peer; how B's HPKE key is refreshed after rotation (binding `seq`).

---

## 9. Phase completion

Phase 4 is complete. Phase 5 (federation) may start.
