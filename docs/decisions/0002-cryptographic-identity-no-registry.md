# ADR-0002: Cryptographic identity for users and servers; no central registry

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 2.1, 3.1, 4, 11, 45, 62

## Context

An identity can be a server-issued account (username, phone number, e-mail) or a cryptographic key the user generates. The same question applies to servers: a server can be identified by a DNS name plus Web PKI certificate, or by a key it generates. Account-based identity gives the server the power to reassign, suspend or impersonate; registry-based server identity makes the system depend on external authorities.

## Decision

- A **user identity is a keypair**. `identity_id = H(identity_public_key)`. The key is generated on the client and is the only thing that defines the identity. Servers keep internal database ids for operations but those ids are never the identity. (ADR-0003 further restricts this to one key per installation.)
- A **server identity is a keypair**. `server_id = H(server_public_key)`. Clients and peer servers verify a server by its key. TLS with Web PKI may protect the transport but is not the source of server identity, and a server may be reached at any address as long as it proves its key.
- **No central registry** of users or servers exists. Users learn each other's keys out of band (ADR-0006). Users learn their home server's key at registration and other servers' keys through contact cards.
- The binding between a user identity and its current home server is a **statement signed by the identity key** (`HomeServerBinding` with a monotonic `seq`), not a server-side record. The user can move that binding to another server at any time; the old server cannot forge a newer one.

## Pros

- Neither a server nor any third party can create, reassign or take over a user identity.
- A user can leave a server and keep every contact, because contacts hold the key, not the server's name.
- Servers are portable across hosting, DNS and certificate authorities; a self-hoster can move without changing identity.
- The design needs no phone number, e-mail or username, so no database of those exists.

## Cons

- Keys are unreadable to humans; local nicknames and out-of-band verification carry the whole UX burden.
- Without a registry there is no way to look up a stranger (accepted in ADR-0006).
- Key loss is identity loss; there is no "forgot password" (ADR-0023).
- Server key distribution and rotation must be specified; a server that loses its key must be re-trusted by every client and peer.

## Alternatives considered

### Server accounts with usernames

Rejected: gives the server ownership of identity and creates a registry.

### Phone-number identity (Signal, WhatsApp model)

Rejected: requires a central verification service and leaks a real-world identifier.

### DNS + Web PKI as the server identity

Rejected as the *source* of identity: it ties server identity to domain ownership and to certificate authorities outside the user's trust. Kept as an optional transport protection.

### DHT or blockchain-based registry

Rejected for v1: heavy infrastructure, and the invite-first model (ADR-0006) removes the need.

## Consequences

- Contact cards carry the server public key so that clients can build the HPKE layer (ADR-0016) and verify federation peers.
- Server migration (README section 11) is a client-signed binding update, not a server-to-server account transfer. **Whenever** a server observes a `HomeServerBinding` for an identity with `seq` higher than the one it stores, it **MUST** drop unused share tokens and the old one-time prekey stock, revoke contact capabilities, and disable that mailbox. First contact **MUST** resolve the current `HomeServerBinding` via discovery, not the card snapshot alone. Group fan-out is refreshed under the member credential (ADR-0017); moving the group itself stays deferred.
- The fingerprint shown to users is over the identity public key only; all other keys are signed by it (ADR-0005).

## History

- 2026-09-19 — Accepted. Foundational principle of the original draft, recorded as a decision.
