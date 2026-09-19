# ADR-0006: Invite-first contact discovery, no global lookup

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 3, 6, 20, 21, 22, 43, 53

## Context

The original draft left contact discovery open (section 20, 53.1). Any lookup service that maps a human-readable name to a key creates a namespace to administer and lets the server observe who searches for whom. The draft's principle is that the server is infrastructure, not authority, and that social graph information should not be exposed unnecessarily.

## Decision

Contacts are established **only** by exchanging a **contact card** out of band (QR code, link, file, or any channel the users already trust).

The contact card contains:

```text
ContactCard
├── version                         required; every card begins with a protocol version (ADR-0011)
├── identity_public_key
├── revocation_public_key           (ADR-0004)
├── home_server_binding             signed by identity key: {server_id, server_public_key, endpoints, seq, expires_at}
└── share_token                     one-time, short-TTL introduction token minted for this share (ADR-0007)
```

- There is no username, no phone-number lookup, no directory, no `identity@domain` address in v1.
- Discovery serves cryptographic material for an identity you already know (current home-server binding, revocation statement). A one-time prekey is served **only** when the requester presents a live unused share token (ADR-0005, ADR-0007). At most one prekey is reserved per token. It does not answer "who is X". KeyPackages are issued on group accept and travel in-band to the admitting member (ADR-0018), not left infinitely fetchable.
- First contact consumes the share token (ADR-0007). Once a session exists, the parties exchange private per-conversation contact capabilities (ADR-0008).
- Adding someone to a group requires they accept a group-invite token (ADR-0018). Holding a contact card is not enough.
- Clients **gossip** the latest observed home-server binding `seq` inside existing encrypted sessions and alert on conflict, so a discovery service cannot indefinitely show different bindings to different peers.

## Pros

- No global namespace, no registry, nothing for a server to be authoritative over.
- The server never sees a search. The social graph is not revealed by discovery.
- Spam control is built in: each share is one-shot and short-lived (ADR-0007).
- Verification happens at exchange time: scanning a freshly minted QR in person is the verification.

## Cons

- No cold contact. You cannot reach someone you have never met or been introduced to.
- Onboarding friction: every relationship starts with an out-of-band exchange.
- Two people cannot use the same QR; the next person needs a newly minted token.
- Contact cards expire with the home-server binding and the share token TTL.

## Alternatives considered

### `identity_id@domain` addressing

Familiar and supports cold contact. Rejected for v1 because it creates a namespace and exposes lookups to the server. Could be added later as an optional, clearly labelled mode without changing the contact-card model.

### Privacy-preserving lookup (PIR / private set intersection over phone numbers)

Rejected for v1 as heavy infrastructure with weak guarantees against the operator; deferred indefinitely.

## Consequences

- README section 20 becomes normative rather than a list of options; 53.1 is resolved.
- The contact card format must be versioned and specified in the envelope/protocol document.
- Share tokens follow ADR-0007. Group join follows ADR-0018.

## History

- 2026-09-19 — Accepted. Invite-first discovery; no global lookup.
