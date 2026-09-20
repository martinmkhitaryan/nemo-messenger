# Phase 2 — Cryptographic protocol

- **Status:** Complete
- **Date:** 2026-09-19
- **Phase:** 2 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** [01-security-model.md](01-security-model.md)
- **Decisions:** [ADR-0003](../decisions/0003-single-key-identity.md), [0004](../decisions/0004-revocation-key.md), [0005](../decisions/0005-double-ratchet-and-mls.md), [0006](../decisions/0006-invite-first-discovery.md), [0007](../decisions/0007-one-time-share-tokens.md), [0018](../decisions/0018-group-join-and-member-credentials.md), [0028](../decisions/0028-implementation-languages-and-libraries.md), [0029](../decisions/0029-cryptographic-identifiers-and-encodings.md)

This document binds algorithms, signed-object encodings, 1:1 session policy, and MLS application policy. It does not define mailbox envelopes, padding bucket sizes, or HTTP. Canonical CBOR means RFC 8949 definite lengths, no indefinite encodings, map keys sorted by encoded byte order.

Protocol version for every object in this document is **1**. Unknown versions are rejected; there is no downgrade ([ADR-0011](../decisions/0011-protocol-version-field.md)).

---

## 1. Primitive algorithms

| Role | Algorithm |
| --- | --- |
| Identity and revocation signatures | Ed25519 (RFC 8032), 32-byte public key, 64-byte signature |
| `H` | SHA-256 (FIPS 180-4) |
| `identity_id` | `SHA-256(identity_public_key)` (32 bytes) |
| `server_id` | `SHA-256(server_hpke_public_key)` (32 bytes) |
| PQXDH / Double Ratchet | libsignal's current PQXDH (ML-KEM-768 hybrid as shipped) then Double Ratchet, one-time prekeys only ([ADR-0005](../decisions/0005-double-ratchet-and-mls.md), [ADR-0028](../decisions/0028-implementation-languages-and-libraries.md)) |
| MLS | RFC 9420 ciphersuite `0x0003` (`MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519`) via OpenMLS |
| Federation / nested HPKE (used in phase 3) | RFC 9180: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305 |

Do not invent primitives. Groups are not post-quantum.

---

## 2. Domain-separated signatures

Every Ed25519 signature over a Nemo protocol object is:

```text
signature = Ed25519_Sign(sk, "nemo-v1/" || context || 0x00 || enc(payload))
```

`context` is ASCII without slashes. `enc(payload)` is canonical CBOR of the payload **without** the signature field. Verify with the corresponding public key before trusting any field.

| context | Signed by | Payload |
| --- | --- | --- |
| `home-server-binding` | identity key | binding fields in §4 |
| `prekey` | identity key | §6.1 |
| `revocation` | revocation key | §5 |
| `group-invite` | group member signing key | [ADR-0018](../decisions/0018-group-join-and-member-credentials.md) |
| `group-admit` | group member signing key | ADR-0018 |
| `remove-sidecar` | group member signing key | ADR-0018 sidecar |
| `signing-key-replace` | currently registered group member signing key | ADR-0018 |
| `server-bundle` | server Ed25519 signing key | [phase 5](05-federation.md) |
| `server-sign-rotate` | current server Ed25519 | phase 5 |
| `mailbox-owner` | user identity key | [phase 4](04-delivery-protocol.md) |

A signature that verifies under the wrong context MUST be rejected.

---

## 3. Key hierarchy on an installation

```text
identity_private_key          Ed25519, never leaves the installation
revocation_private_key        Ed25519, shown once, not stored (ADR-0004)
pqxdh_identity_dh             X25519, signed via prekey objects
one-time PQXDH prekeys        finite stock, signed
mls_hpke_init                 per KeyPackage, signed by identity inside MLS credential
per-group signing key         Ed25519, created at admit, published in MLS (ADR-0018)
```

The identity key signs all long-term and prekey material the identity uses. MLS application messages and handshake authentication follow RFC 9420 under suite `0x0003`. Double Ratchet message authentication is whatever libsignal specifies for PQXDH (deniable for 1:1).

Fingerprint (UX): hex of `identity_id`, eight groups of eight lowercase characters, separated by spaces.

---

## 4. Contact card (version 1)

CBOR map, version field first as required by [ADR-0011](../decisions/0011-protocol-version-field.md). Integer keys:

| Key | Field | Type |
| --- | --- | --- |
| 0 | `version` | uint, MUST be 1 |
| 1 | `identity_public_key` | bstr, 32 |
| 2 | `revocation_public_key` | bstr, 32 |
| 3 | `share_token` | bstr, 32 (CSPRNG) |
| 4 | `binding` | map, §4.1 |

Encoded size MUST be ≤ 400 bytes. QR: binary mode of the CBOR. Link: `nemo:1:` concatenated with unpadded base64url of the CBOR. QR and link are the same object ([ADR-0007](../decisions/0007-one-time-share-tokens.md)).

### 4.1 `binding` (home-server binding)

| Key | Field | Type |
| --- | --- | --- |
| 0 | `server_id` | bstr, 32 |
| 1 | `server_hpke_public_key` | bstr, 32 (X25519) |
| 2 | `host` | tstr, 1–64 bytes, DNS or `.onion`, no scheme, no path |
| 3 | `seq` | uint, monotonic per identity |
| 4 | `expires_at` | uint, Unix seconds |
| 5 | `signature` | bstr, 64 |

Signed context `home-server-binding` over keys 0–4 (no signature). Client MUST reject if `server_id ≠ SHA-256(server_hpke_public_key)`. Client MUST reject if `expires_at` is in the past by more than 300 seconds of local clock. Pin the highest accepted `seq`; a later discovery result with a lower `seq` is an alert, not a downgrade.

Showing a QR, copying a `nemo:1:` URI, or exporting a file MUST mint a new `share_token` and a new card. Two people need two cards.

Default `share_token` TTL at the home server: **30 minutes**. Allowed values: 5 minutes, 30 minutes, 1 hour. Unused tokens die at TTL. First envelope to the token consumes it. Prekey fetch with the token does not consume it; at most one prekey is reserved per live token.

---

## 5. Revocation statement

CBOR map:

| Key | Field | Type |
| --- | --- | --- |
| 0 | `version` | uint, 1 |
| 1 | `identity_id` | bstr, 32 |
| 2 | `status` | tstr, MUST be `revoked` |
| 3 | `coarse_timestamp` | uint, Unix seconds, 1-hour granularity recommended |
| 4 | `signature` | bstr, 64 |

Signed context `revocation` over keys 0–3 with the **revocation** private key. Verify against `revocation_public_key` from the contact card or a previously pinned card. Handling is [phase 1 §5.4](01-security-model.md). The statement does not name a successor.

---

## 6. 1:1 — PQXDH and Double Ratchet

Use libsignal as a primitive library, not Signal's network protocol ([ADR-0028](../decisions/0028-implementation-languages-and-libraries.md)).

### 6.1 Signed one-time prekey blob (discovery)

Discovery stores opaque rows. Each row is CBOR:

| Key | Field | Type |
| --- | --- | --- |
| 0 | `version` | uint, 1 |
| 1 | `identity_public_key` | bstr, 32 |
| 2 | `prekey_id` | uint |
| 3 | `libsignal_prekey` | bstr (libsignal serialisation, including PQXDH one-time material) |
| 4 | `signature` | bstr, 64 |

Signed context `prekey` over keys 0–3. The server MUST NOT parse `libsignal_prekey`. The client MUST verify the signature under `identity_public_key` and MUST verify that this public key matches the pinned contact.

Rules already decided, restated:

- No reusable last-resort prekey. Empty stock → refuse a **new** 1:1.
- Fetch requires a live unused share token. Fetch does not consume the token.
- At most one prekey reserved per live token; repeat fetch returns that same blob.
- Clients SHOULD upload 100 prekeys; MUST restock when unused count &lt; 25.
- Existing sessions do not use discovery prekeys.

### 6.2 Session policy

- A 1:1 session is exactly one pair of identity keys.
- After PQXDH, Double Ratchet as implemented by libsignal, including its post-quantum ratchet if present in that library version.
- Skipped-message keys: keep at least 1000; delete after the corresponding plaintext is processed and the skip window no longer needs them. Size the window against mailbox retention in phase 4; if they disagree, raise the skip window, not the retention, in a later record.
- Do not fall back to a weaker handshake if PQXDH material is missing.
- A 1:1 that becomes a group starts a **new** MLS group; no history, no in-place upgrade.

---

## 7. Groups — MLS application profile

Ciphersuite `0x0003` only. Clients MUST reject other suites in v1.

### 7.1 Credential and leaf extension

- MLS credential type: BasicCredential.
- `credential.identity` = 32-byte Ed25519 identity public key (not `identity_id`).
- Private v1 leaf extension with extension type `0xF001` (Nemo v1; replace if IANA assigns). Payload: 32-byte `credential_id` reserved at accept ([ADR-0018](../decisions/0018-group-join-and-member-credentials.md)).
- That extension is immutable. Clients MUST reject a Commit that changes or removes it.
- Group member signing public key MUST appear in MLS (application-specific proposal or in the leaf as specified by OpenMLS custom extensions in implementation notes) **before** that member invites or admits. v1: publish it as an MLS Application-specific **leaf extension** `0xF002`, 32-byte Ed25519 public key, replaceable only together with `SigningKeyReplace` on the host.

### 7.2 Required / forbidden RFC 9420 features

MUST reject (do not apply, even if the host delivered the bytes):

- External Commits
- External joins / Add via external proposal
- ReInit
- A Commit that processes a Remove unless it arrived as a `RemoveBundle`
- A `RemoveBundle` whose Commit removes zero, two, or more leaves, or a leaf whose `0xF001` value ≠ sidecar `credential_id`

Join is hybrid invite → accept → admit only. KeyPackages are produced at accept and travel in-band to the admitter, not via public discovery. The group host MUST NOT store KeyPackage plaintext.

### 7.3 PCS

- MUST commit an MLS Update at least every **7 days** of membership.
- MUST NOT send an application message if this member's last own Update is older than **72 hours**; Update first.
- SHOULD Update on coming online if the last own Update is older than 24 hours.
- A quiet group must not wait for someone to speak.

### 7.4 `RemoveBundle` and `SigningKeyReplace`

These are **host-framed** stream objects, not MLS-only. Byte layout is phase 3/4 (they sit on the group stream). Cryptographic requirements:

- Sidecar `{credential_id, "revoke"}` signed context `remove-sidecar` with the **appending** member's current group signing key.
- `SigningKeyReplace {new_public_key}` signed context `signing-key-replace` with the currently registered key; members publish the same new key in MLS extension `0xF002`.

### 7.5 Call exporter (group calls, deferred)

Group call media keys, when specified, MUST use the MLS exporter with label `SFrame 1.0 Base Key` as in [ADR-0024](../decisions/0024-voice-call-architecture.md), from a **call-specific** MLS group. Not used in v1 1:1 calls (DTLS-SRTP).

---

## 8. Discovery as untrusted input

Clients MUST verify, not trust:

| Object | Check |
| --- | --- |
| Prekey blob | context `prekey`, identity key, matches pinned contact |
| Home-server binding | context `home-server-binding`, `seq` not lower than pinned, `server_id` matches HPKE key |
| Revocation | context `revocation`, revocation public key from card |
| Group invite / admit | MLS-published signing key wins over host copy |

Refresh discovery at least every **12 hours** for 1:1 contacts and every MLS-group identity; SHOULD every 4 hours.

Gossip the highest observed binding `seq` inside existing encrypted sessions. On conflict, alert; do not silently take the server's value.

---

## 9. Key lifecycle (delete)

- One-time prekeys: delete locally when discovery reports them consumed or when replaced by a newer stock after migration.
- Double Ratchet skipped keys: delete when the skip window no longer includes them.
- MLS epoch secrets: delete according to OpenMLS / RFC 9420 after the epoch is no longer needed for out-of-order application messages in that epoch.
- Never export ratchet or MLS state ([ADR-0023](../decisions/0023-no-backup-no-recovery.md)).
- After displaying a disappearing message, delete the message key (application protocol, phase 7).

---

## 10. Inputs to phase 3

Phase 3 MUST specify padded mailbox envelopes with no sender field, version byte, inner/outer bucket sizes, `InnerEnvelope` / `OuterEnvelope` layouts, HPKE seal of the inner (suite in §1), `ttl_bucket` numeric set, and host-framed bytes for `RemoveBundle`, `SigningKeyReplace`, `AttachmentReserve`. Dummy cover envelopes MUST use the same buckets.

---

## 11. Phase completion

Phase 2 is complete. Phase 3 (envelope protocol) may start.
