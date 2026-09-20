# ADR-0032: Server signing key and TLS-pinned federation hop

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 4, 10, 53.5; [`docs/protocol/05-federation.md`](../protocol/05-federation.md)

## Context

[ADR-0016](0016-server-to-server-routing.md) requires a server-authenticated channel for HPKE ciphertexts but does not pick the handshake. The contact card carries an X25519 HPKE key (`server_id = SHA-256` of it, [ADR-0029](0029-cryptographic-identifiers-and-encodings.md)). X25519 is not a TLS signature algorithm, so the federation hop needs a separate server signing key.

## Decision

- Every server has an **Ed25519 signing key** in addition to its X25519 HPKE key. `server_id` remains `SHA-256(server_hpke_public_key)`.
- A **ServerBundle** (canonical CBOR, signed context `server-bundle`) binds `{server_id, server_hpke_public_key, server_sign_public_key, host}`. Clients and peers MUST reject a bundle whose `server_id` or HPKE key disagrees with the pinned card/binding.
- The A→B hop is **TLS 1.3** with **pinned** Ed25519 server certificates (self-signed; Web PKI MUST NOT be the source of identity). Mutual TLS: each peer presents its signing key. After TLS, frames are length-prefixed OuterEnvelope `hpke_ciphertext` values (or a thin envelope around them specified in the phase-5 document).
- Replay: per-direction monotonic `uint64` frame counters; drop `≤ last`. Retry: exponential backoff, cap 5 minutes, until the 14-day outbound expiry ([ADR-0031](0031-mailbox-retention-and-owner-auth.md)).
- Per-peer rate limit: default 100 accepted inners / second; a server MUST be able to refuse a peer.

## Pros

- Matches ADR-0002 (server key is identity) without forcing Web PKI.
- HPKE key on the card still lets Alice seal before a bundle fetch.

## Cons

- Two server keys to rotate. Rotation: new bundle with same `server_id` only if HPKE key is unchanged; a new HPKE key is a new `server_id` and a new home-server binding `seq` from every user on that server.

## Alternatives considered

### Web PKI as federation identity

Rejected: ADR-0002.

### HPKE Auth mode only, no TLS

Possible. Rejected for v1: TLS 1.3 + pin is the well-trodden operator path behind Caddy if Caddy is skipped on the S2S port; the phase-5 document allows the S2S port to be internal mTLS not through Caddy.

## Consequences

- Phase 5 document is normative for the hop.
- README 53.5 (HPKE suite, handshake) is resolved.

## History

- 2026-09-19 — Accepted. Ed25519 server signing key; TLS 1.3 pin for S2S.
