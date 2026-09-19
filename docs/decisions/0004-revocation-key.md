# ADR-0004: Revocation key instead of successor or recovery mechanisms

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 3, 6, 17, 35, 36, 43, 45, 53, 59

## Context

ADR-0003 makes a lost or stolen device a lost identity. Without any additional mechanism, an attacker holding a stolen device can keep acting as the victim indefinitely, and contacts have no authenticated way to learn that the identity is dead.

Two remedies were discussed: a pre-signed revocation certificate created at identity creation, and a successor certificate signed by the old key when the user still controls the old device. The project does not want successor certificates: a new device is a new identity, full stop. It does want a way to kill an identity from outside the device.

## Decision

Each identity has a second, independent **revocation keypair** (Ed25519), created at the same time as the identity key.

- The revocation public key is part of the contact card, next to the identity public key, so every contact can verify a revocation without needing anything else.
- The revocation private key is displayed once (24-word phrase and/or QR), is meant to be stored off the device, and is never stored by the client afterwards.
- Its only power is to sign a revocation statement: `{identity_id, "revoked", coarse_timestamp}`.
- The home server, on receiving a valid revocation statement: deletes the mailbox, key material and push endpoint; serves the statement from discovery to anyone asking about that identity; appends the statement into every group stream it hosts that contains the identity. That append is the **only** host-only stream object: a verified `RevocationStatement`. It is not an MLS handshake message. The host **MUST NOT** append Proposals, Commits, Welcome, or application messages without a member credential (review RS1). It does not know groups hosted elsewhere and cannot notify those hosts.
- Clients **MUST** refresh discovery on a coarse interval (hours) for existing 1:1 contacts **and** for every identity that appears in an MLS group they belong to. Members already see those identities inside MLS.
- Clients that receive a valid revocation statement mark the contact as revoked, stop sending to it, refuse new sessions from that key, and **MUST** commit MLS Remove in every group they share with that identity.
- The Remove is a `RemoveBundle` (ADR-0018). When the host admits it, it fans the object to the pre-revoke set (including the removed member), then drops the credential and prunes fan-out. The host does not parse the MLS Remove.
- The revocation key cannot authorise a new identity, add a device, recover messages or do anything other than revoke.

## Pros

- Gives the user a way to end a stolen identity from another machine without introducing a key hierarchy.
- Contacts can verify the revocation independently of the server.
- Group members learn of a revocation via a hosted-group stream injection, or by polling discovery for MLS identities and then Removing.

## Cons

- The user has to store a 24-word phrase somewhere safe; many will not.
- Anyone who obtains the phrase can kill the identity (denial of service, not impersonation).
- 1:1 contacts and foreign-hosted groups learn of a revocation only when they next fetch discovery, gossip, or see a hosted-stream injection. Hours lag stays accepted. The home server cannot push to unknown contacts or to group hosts it does not run. Phrase-as-DoS stays accepted: anyone with the phrase can kill the identity.
- Until some member commits Remove, a stolen client still holds member credentials and group signing keys on foreign hosts.
- Does nothing about messages already readable on the stolen device.

## Alternatives considered

### Pre-signed revocation certificate (PGP style)

A fixed signed statement created at identity creation and exported as a file. Rejected because it is a bearer token with a fixed timestamp and cannot be extended to anything else; a key is strictly more flexible at the same cost.

### Successor certificate

Old key signs "my successor is X" during a planned migration. Rejected by product decision: any new device is a new identity.

### Home server notifies every group host

Rejected: the home server has no group list (ADR-0009, ADR-0017). The machine holding the phrase has no MLS state. Other members are the only party that can Remove.

### No mechanism

Lost identity simply goes silent. Rejected because it leaves a stolen device fully functional forever.

## Consequences

- Contact card gains a field: `revocation_public_key`.
- Discovery gains a `RevocationStatement` object and a rule to serve it for revoked identities.
- Server data model gains `RevocationStatement`; the mailbox and key material of revoked identities are deleted.
- Client UX needs a one-time "save your revocation phrase" step and a "Identity revoked" contact state.
- Clients MUST periodically refresh discovery for existing 1:1 contacts and for every MLS-group identity, on a coarse interval (hours).
- Clients MUST commit MLS Remove after a verified revocation as a `RemoveBundle`; the host MUST NOT parse MLS (ADR-0018).

## History

- 2026-09-19 — Accepted. Off-device revocation key; no successor or recovery.
