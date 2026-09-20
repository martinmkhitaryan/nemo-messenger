# ADR-0029: Cryptographic identifiers, contact-card encoding, MLS suite, and PCS interval

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 3, 4, 6, 12, 20, 21, 53.12; [`docs/protocol/02-cryptographic-protocol.md`](../protocol/02-cryptographic-protocol.md)

## Context

Phase 1 left named holes for phase 2: the hash `H`, how the identity key signs protocol keys, a contact card that fits a QR, the MLS ciphersuite, and the MLS Update interval that ADR-0005 required but did not number. Share-token TTL was also still open (README 53.12). These are protocol choices, not library preferences.

## Decision

- **`H` is SHA-256.** `identity_id = SHA-256(identity_public_key)` and `server_id = SHA-256(server_hpke_public_key)`. Public keys are 32-byte raw values (Ed25519 or X25519), not PEM or SPKI.
- **Fingerprint** shown to users is `identity_id` encoded as 64 lowercase hex characters in eight groups of eight. One fingerprint covers every protocol key because those keys are signed by the identity key (ADR-0005).
- **Contact card** is canonical CBOR (RFC 8949 definite-length, integer keys, sorted) beginning with protocol version `1`. QR is that CBOR in QR binary mode. URI form is `nemo:1:` plus unpadded base64url of the same bytes. Maximum CBOR size is **400 bytes**. `host` is a single UTF-8 DNS name or `.onion`, max 64 bytes. `server_id` and `server_hpke_public_key` are both in the signed binding (ADR-0006, ADR-0016); the client MUST reject the card if `server_id ≠ SHA-256(server_hpke_public_key)`.
- **Share-token and group-invite TTL** default is **30 minutes**. The set of allowed introduction TTLs is {5 min, 30 min, 1 hour}. Consume and race rules stay ADR-0007.
- **MLS ciphersuite** is RFC 9420 `0x0003`: `MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519`. No PQ claim. Leaf credential is MLS BasicCredential whose `identity` is the 32-byte Ed25519 identity public key. Leaf extension `nemo.credential_id` (TBD IANA; v1 uses the private range as specified in the phase-2 document) holds the 32-byte reserved host credential id and is immutable for the leaf.
- **MLS Update:** a member MUST commit an Update at least every **7 days** of membership, and MUST NOT send an application message if their last own Update is older than **72 hours**. External Commits, external joins, and ReInit remain rejected (ADR-0005).
- **PQXDH + Double Ratchet** follow libsignal's current PQXDH (ML-KEM-768 hybrid as shipped by that library) with **one-time prekeys only**. Discovery stores an opaque signed blob; the server does not parse libsignal. Clients SHOULD upload 100 one-time prekeys and MUST restock when fewer than 25 remain unused.
- **Signature domain separation:** Ed25519 signatures over protocol objects use UTF-8 prefix `nemo-v1/` plus a context string, then the canonical CBOR payload, as listed in the phase-2 document.

## Pros

- SHA-256 and RFC 9420 suite `0x0003` are boring, widely implemented, and match an Ed25519 identity without AES.
- A 400-byte CBOR card plus 30-minute tokens closes the QR-size and TTL holes without a directory.
- Numbered Update policy makes group PCS testable.

## Cons

- SHA-256(Ed25519 pk) is not a hash of a self-certifying SPKI document; rotation of the identity key is a new identity (already true under ADR-0003).
- ChaCha20 MLS suite is not the MLS mandatory-to-implement AES suite; interop with a foreign MLS stack may need a mapping. Independent Nemo implementations MUST use `0x0003`.
- 30-minute share TTL is harsh for a link sent to someone who is asleep; the user mints again. That is the cost of ADR-0007.

## Alternatives considered

### BLAKE2b-256 for `H`

Slightly faster. Rejected: SHA-256 is the default assumption of reviewers and of libsignal-adjacent stacks.

### MLS MTI suite `0x0001` (AES-128-GCM)

Rejected: mobile clients without AES-NI pay for AES; ChaCha20 matches the rest of the v1 AEAD preference.

### PQ MLS ciphersuites from draft-ietf-mls-pq-ciphersuites

Rejected: still a draft (ADR-0005, ADR-0025).

### Share-token TTL of 24 hours

Easier UX for links. Rejected: ADR-0007 said typically minutes; a day-long token is a reusable inbox for that day.

### Put only `server_id` on the card; fetch HPKE keys from the host

Smaller QR. Rejected: ADR-0016 requires the recipient server public key on the card so Alice can HPKE-seal without a round trip to Bob's operator before first contact.

## Consequences

- [`docs/protocol/02-cryptographic-protocol.md`](../protocol/02-cryptographic-protocol.md) is the normative encoding.
- README 53.12 contact-card encoding and share-token TTL defaults are resolved.
- ADR-0005's Update interval is this record's 7-day / 72-hour rule.

## History

- 2026-09-19 — Accepted. Phase-2 cryptographic identifiers and encodings.
