# Phase 3 — Envelope protocol

- **Status:** Complete
- **Date:** 2026-09-19
- **Phase:** 3 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** [02-cryptographic-protocol.md](02-cryptographic-protocol.md)
- **Decisions:** [ADR-0009](../decisions/0009-sealed-sender.md)–[0014](../decisions/0014-sender-chosen-ttl-bucket.md), [0016](../decisions/0016-server-to-server-routing.md), [0019](../decisions/0019-attachment-as-padded-envelope.md), [0030](../decisions/0030-envelope-buckets-and-layout.md)

All multi-byte integers are unsigned big-endian. `version` is the first byte and MUST be `0x01`. Unknown versions are rejected. Padding bytes MUST be zeros. There is **no sender field** on any of these objects.

Dummy cover envelopes (when phase 6 exists) MUST be valid objects in these buckets. No flag distinguishes dummy from real.

---

## 1. Bucket sets (version 1)

### 1.1 Inner `padded_message_envelope` (what Server B stores in a mailbox)

Text / control / MLS-fan-out application bytes after AEAD, including Double Ratchet or MLS ciphertext:

| Name | Size (bytes) |
| --- | --- |
| T1 | 1024 |
| T2 | 4096 |
| T3 | 16384 |

Attachments (1:1 mailbox copy of a file):

| Name | Size (bytes) |
| --- | --- |
| A1 | 262144 |
| A2 | 1048576 |
| A3 | 4194304 |
| A4 | 16777216 |

Pick the smallest bucket that holds `version || type || body`. Never transmit a shorter length.

### 1.2 Outer (InnerEnvelope padded before `HPKE_Seal`)

| Content | Outer size (bytes) |
| --- | --- |
| Any T1–T3 inner | 20480 |
| A1 | 266240 |
| A2 | 1052672 |
| A3 | 4202496 |
| A4 | 16781312 |

Server A sees only this outer class. T1 vs T2 vs T3 is not visible on A→B.

---

## 2. `ttl_bucket`

uint8 inside the inner envelope (visible to Server B only):

| Value | Meaning |
| --- | --- |
| 0 | No extra TTL; server retention only |
| 1 | 60 seconds from receipt |
| 2 | 1 hour |
| 3 | 1 day |

`expires_at = min(receipt + ttl, server_retention)`. Never returned to clients. `call_invite` MUST use `1`.

---

## 3. `padded_message_envelope`

```text
offset 0     version        u8     = 1
offset 1     type           u8     see §3.1
offset 2     body           nbytes  opaque ciphertext or host-framed object
offset 2+n   zero padding   to inner bucket
```

`type` is **not** a sender id. It tells the recipient which parser to use. Keep the set small so it is not a content oracle beyond what size already leaks.

### 3.1 `type` values

| type | Body |
| --- | --- |
| 0x01 | 1:1 Double Ratchet packet (libsignal message bytes) |
| 0x02 | MLS PrivateMessage / PublicMessage bytes (fan-out of a group stream object to a mailbox) |
| 0x03 | 1:1 attachment: same as 0x01 semantically (file key lives inside DR plaintext); body is DR wrapping the file ciphertext, padded to an A* bucket |
| 0x10 | Host-framed `RemoveBundle` (on the **group stream**; when fanned to a mailbox it is still 0x10) |
| 0x11 | Host-framed `SigningKeyReplace` |
| 0x12 | Host-framed `AttachmentReserve` |
| 0x13 | Host-framed `RevocationStatement` (the CBOR from phase 2, as delivered on a group stream) |
| 0x14 | Opaque MLS handshake bytes on the group stream (Proposal, Commit, Welcome) that are not a Remove |

Types 0x10–0x14 appear on group streams. Fan-out copies the same bytes into member mailboxes as type 0x02 **or** preserves 0x10–0x13 so clients can parse host framing. **v1 rule:** fan-out is a verbatim copy of the stream object, including type byte. Mailbox inner bucket is chosen for that object's size (T* or A*).

---

## 4. Host-framed bodies

Lengths below are of `body` only, before inner padding.

### 4.1 `RemoveBundle` (type 0x10)

```text
credential_id               32 bytes
sidecar_signature           64 bytes   context remove-sidecar over (credential_id || "revoke")
mls_commit_len              u32
mls_commit                  mls_commit_len bytes
```

Sidecar semantic `{credential_id, "revoke"}` ([ADR-0018](../decisions/0018-group-join-and-member-credentials.md)). The host verifies the signature against the **appending** credential's registered group signing key, fans out, then revokes. It MUST NOT parse `mls_commit`.

### 4.2 `SigningKeyReplace` (type 0x11)

```text
new_public_key              32 bytes
signature                   64 bytes   context signing-key-replace
```

### 4.3 `AttachmentReserve` (type 0x12)

```text
fetch_token                 32 bytes
size_bucket                 u8         1=A1 … 4=A4
ttl_bucket                  u8
```

No file key, no filename. File bytes are a separate host object uploaded after reserve, length equal to that A* bucket, AEAD ciphertext from the uploader's MLS application key (phase 7).

### 4.4 `RevocationStatement` (type 0x13)

Canonical CBOR from [phase 2 §5](02-cryptographic-protocol.md).

---

## 5. InnerEnvelope (HPKE plaintext, then padded to an outer bucket)

```text
version                     u8     = 1
delivery_capability         32 bytes
ttl_bucket                  u8
idempotency_token           32 bytes   CSPRNG, unique per append attempt; not a message id
padded_message_envelope     inner-bucket bytes
zero padding                to outer bucket
```

No sender field. `idempotency_token` is per-append and opaque ([ADR-0012](../decisions/0012-no-global-message-identifiers.md), [ADR-0015](../decisions/0015-bounded-mailbox-log-and-acks.md)). Same token + same capability at Server B is the same append.

---

## 6. OuterEnvelope (what Alice posts to Server A)

```text
version                     u8     = 1
destination_server_id       32 bytes
hpke_ciphertext_len         u32
hpke_ciphertext             RFC 9180 HPKE seal of the padded InnerEnvelope
                            to server_hpke_public_key of destination_server_id
```

HPKE: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305. Info string: `nemo-v1/s2s-inner`. AAD: `version || destination_server_id`.

Same-server (A = B): Alice still builds this object; A opens HPKE with its own key.

Server A forwards `hpke_ciphertext` (and the destination id it already knows) on the server-authenticated channel. Phase 5 specifies that channel. A MUST NOT be given `delivery_capability` in the clear.

---

## 7. Replay and HPKE

- Each InnerEnvelope MUST use a fresh HPKE ephemeral. Reuse of the HPKE encapsulated key with a different inner is rejected by Server B.
- Server B SHOULD keep a short window of seen HPKE encapsulations per peer to drop duplicates; this is not a global message id.
- Inner `idempotency_token` is the append dedup key (phase 4).

---

## 8. What each party sees

| Party | Sees |
| --- | --- |
| Alice | Everything she built |
| Server A | `destination_server_id`, outer length (text vs attachment class), Alice's client session |
| Network A→B | HPKE ciphertext length (same classes), authenticated peer ids (phase 5) |
| Server B | `delivery_capability`, `ttl_bucket`, inner envelope length (T* or A* after opening), no sender |
| Bob | Inner plaintext after DR/MLS |

---

## 9. Inputs to phase 4

Phase 4 MUST specify: mailbox and group-stream append/fetch/ack; capability classes and rotation; share-token consume-on-first-envelope; bounded retention numbers; idempotent append; group fan-out using this OuterEnvelope; attachment upload/fetch for A* objects; unknown-capability constant-time responses.

---

## 10. Phase completion

Phase 3 is complete. Phase 4 (delivery protocol) may start.
