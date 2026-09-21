# Phase 1 — Security model

- **Status:** Complete
- **Date:** 2026-09-19
- **Phase:** 1 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Decisions:** [ADR-0001](../decisions/0001-untrusted-server-blind-delivery.md), [0002](../decisions/0002-cryptographic-identity-no-registry.md), [0003](../decisions/0003-single-key-identity.md), [0004](../decisions/0004-revocation-key.md), [0009](../decisions/0009-sealed-sender.md), [0021](../decisions/0021-privacy-layer-and-modes.md), [0022](../decisions/0022-server-side-data-minimisation.md), [0023](../decisions/0023-no-backup-no-recovery.md), [0025](../decisions/0025-self-hostable-open-source-no-custom-crypto.md)

This document is the written security model required before the cryptographic protocol (phase 2). It does not define wire encodings, ciphersuites, HTTP routes, or database tables. Where a later phase must name an algorithm (for example the hash `H` in `identity_id = H(pubkey)`), that choice is phase 2.

The decision records are authoritative if this file and a record disagree.

---

## 1. Purpose

Nemo is a self-hostable messenger whose value is what the server does **not** know. The security model states:

- who the actors are and where trust stops;
- which properties must remain true in every later phase;
- what each class of attacker can do, and what the product claims against them;
- how an identity is created, used, moved, and killed;
- how abuse is handled when the server cannot see senders.

---

## 2. Actors and trust boundaries

```text
Installation (client)
  owns identity_private_key, ratchet/MLS state, local plaintext
  |  untrusted network, optional Tor
Home server (discovery + blind delivery + optional TURN)
  |  server-authenticated channel
Peer servers (federation)
Push provider (FCM or none)
TURN operator (may be the home server)
User (holds revocation phrase off-device)
```

| Actor | Trusted for | Not trusted for |
| --- | --- | --- |
| Client installation | Generating keys, encrypting, verifying fingerprints, storing vault | Other installations of the same person (they are other identities) |
| User | Storing the revocation phrase; verifying contact cards out of band | Recovering a lost device (impossible) |
| Home server | Availability of *its* mailboxes and discovery, if the operator is competent | Content, identity, social graph, historical keys |
| Peer server | Same as home server for traffic it handles | Same |
| Push provider | Delivering an opaque wake | Anything in the payload beyond a token with no size or type hint |
| TURN operator | Relaying encrypted RTP | Content; it sees IPs and call duration |
| Certificate authorities / DNS | Optional transport protection | Server identity (`server_id = H(server_public_key)`) |

The server is assumed potentially malicious at all times ([ADR-0001](../decisions/0001-untrusted-server-blind-delivery.md)). It may drop, delay, replay, reorder, serve stale discovery, or lie about routing. Protocols must tolerate that and detect what can be detected. A server must never be able to decrypt content, forge an identity, silently replace a verified key, or recover historical keys from its own state.

Self-hosting by a stranger is in scope: the operator is not a trusted party.

---

## 3. Security properties (invariants)

These must remain true through every later phase and in any implementation of this project.

### 3.1 Identity

- A user identity is a keypair. `identity_id = H(identity_public_key)`. Servers may keep internal row ids; those ids are never the identity ([ADR-0002](../decisions/0002-cryptographic-identity-no-registry.md)).
- One client installation is one identity. There is no device layer, enrollment, linking, or key hierarchy ([ADR-0003](../decisions/0003-single-key-identity.md)).
- The identity private key never leaves the installation.
- A server identity is a keypair. `server_id = H(server_public_key)`. TLS/Web PKI may protect transport; it is not the source of server identity.

### 3.2 Confidentiality and keys

- All message content, attachments, and group application data are end-to-end encrypted on the client. There is no unencrypted mode and no server-side decryption path ([ADR-0001](../decisions/0001-untrusted-server-blind-delivery.md)).
- The server holds no user private keys and no conversation keys, ever.
- Ratchet and MLS state never leave the installation, are never exported, and are never recovered ([ADR-0023](../decisions/0023-no-backup-no-recovery.md)).
- Compromise of current keys must not automatically decrypt history (forward secrecy). After a successful key update, future messages must not be readable under the old keys (post-compromise security). Groups are not post-quantum; 1:1 session setup is ([ADR-0005](../decisions/0005-double-ratchet-and-mls.md)).

### 3.3 Sender hiding and storage

- No envelope stored or forwarded by any server names its sender ([ADR-0009](../decisions/0009-sealed-sender.md)).
- The server stores ciphertext only temporarily for offline delivery, never as a message history ([ADR-0001](../decisions/0001-untrusted-server-blind-delivery.md), [ADR-0015](../decisions/0015-bounded-mailbox-log-and-acks.md)).
- Metadata privacy is a separate layer from E2EE ([ADR-0021](../decisions/0021-privacy-layer-and-modes.md)). Padding buckets are always on; stronger modes are optional. The product MUST NOT claim protection against a global passive observer except in a mode that actually implements the required mechanisms.

### 3.4 Recovery

- Identity recovery: none. A lost, wiped, or stolen device is a lost identity. The only off-device control is revocation ([ADR-0004](../decisions/0004-revocation-key.md), [ADR-0023](../decisions/0023-no-backup-no-recovery.md)).
- Cryptographic-state recovery: none, ever.
- Message-history backup or export: none. No local dump, no import, no server blob ([ADR-0023](../decisions/0023-no-backup-no-recovery.md)).

### 3.5 Portability

- The same identity key can move its mailbox and published material to another server. The key never moves to another device ([ADR-0002](../decisions/0002-cryptographic-identity-no-registry.md), [ADR-0003](../decisions/0003-single-key-identity.md)).

---

## 4. Threat model

### 4.1 Malicious or compelled home/peer server

**Can:** read its database; inspect traffic visible to it; modify discovery; drop, delay, replay; stall a group it hosts by withholding Commits; attempt identity substitution via stale or forged unsigned material.

**Must not be able to:** decrypt content; forge a verified identity; produce a `HomeServerBinding` with a higher `seq` than the client; recover historical message keys from ordinary server state; learn senders from envelopes.

A database seizure yields ciphertext, public keys, capabilities, group member-capability sets, and routing state — not plaintext, private keys, or contact lists ([ADR-0022](../decisions/0022-server-side-data-minimisation.md)).

Same-server delivery (Alice and Bob on one operator) reveals Alice → Bob's capability to that operator. That residual is accepted ([ADR-0016](../decisions/0016-server-to-server-routing.md)).

### 4.2 External network attacker

**Can:** intercept and modify packets; replay; MITM transports that are not authenticated by server keys; steal a server database.

**Protection (later phases must supply):** authenticated cryptographic protocols; fingerprint verification at contact-card exchange; encrypted transport; server-key authentication between federation peers; forward secrecy and post-compromise security.

### 4.3 Global passive observer

**Can:** see IPs, timing, sizes, connection patterns, and correlate ingress and egress.

**Protection:** the optional privacy layer ([ADR-0021](../decisions/0021-privacy-layer-and-modes.md)). Not claimed in Normal mode. Cover traffic and constant-rate modes are deferred; the envelope format must not need a wire change to add them (phase 3).

### 4.4 Stolen or compromised client

A live stolen installation *is* the identity until revocation propagates. The attacker can read local plaintext, use conversation keys, and send as that identity. No server design prevents that.

The revocation key can kill the identity from off-device. It cannot unread messages already on the stolen device, and it cannot authorise a replacement identity. Phrase-as-DoS is accepted: anyone with the phrase can revoke.

The other party (screenshots, a modified client) is outside the threat model for disappearing messages.

### 4.5 Push provider and TURN operator

Push sees only an opaque wake token: no size, no type, no conversation, no sender ([ADR-0020](../decisions/0020-opaque-push-wakeup.md)). TURN/SFU sees IPs and duration, not media plaintext, because media is always relayed and 1:1 uses DTLS-SRTP with fingerprints bound in the E2EE channel ([ADR-0024](../decisions/0024-voice-call-architecture.md)).

---

## 5. Identity lifecycle

Wire encodings of these objects are phase 2. The lifecycle itself is binding now.

### 5.1 Create

1. The installation generates an Ed25519 identity keypair and an independent Ed25519 revocation keypair.
2. `identity_id = H(identity_public_key)`.
3. The client displays the revocation private key once (24-word phrase and/or QR) and then MUST NOT store it.
4. UX MUST state that the identity and its history cannot be recovered or exported.
5. The client registers with a home server: publishes identity public key, revocation public key, a signed `HomeServerBinding` (`server_id`, `server_public_key`, endpoints, monotonic `seq`, `expires_at`), and a finite one-time PQXDH prekey stock. The server never sees private keys.

### 5.2 Use

- Contacts are established only by exchanging a contact card out of band ([ADR-0006](../decisions/0006-invite-first-discovery.md)). There is no directory lookup.
- Each share (QR, link, file) mints a new one-time short-TTL share token ([ADR-0007](../decisions/0007-one-time-share-tokens.md)).
- Local nicknames exist only on the client.
- Clients MUST periodically refresh discovery (hours) for existing 1:1 contacts and for every identity in every MLS group they belong to ([ADR-0004](../decisions/0004-revocation-key.md)).
- First contact MUST resolve the current `HomeServerBinding` via discovery, not a card snapshot alone.

### 5.3 Migrate home server

The key stays on the installation. Portability moves mailbox and published material.

1. Register on server B: new mailbox capability, fresh one-time prekey stock.
2. Sign a `HomeServerBinding` with `seq` higher than the current one; publish on B (and on A if reachable).
3. **Whenever** a server observes a binding for that identity with `seq` higher than the one it stores, it MUST drop unused share tokens and the old prekey stock, revoke contact capabilities, and disable that mailbox — even if it learns this late ([ADR-0002](../decisions/0002-cryptographic-identity-no-registry.md)).
4. The client sends the new binding and new contact capabilities over existing encrypted sessions.
5. For each group, refresh fan-out `{delivery_capability, home_server}` under the member credential. This is not group migration (deferred).

The old server cannot impersonate the identity: it never held the key and cannot produce a higher `seq`.

### 5.4 Revoke

A `RevocationStatement` is `{identity_id, "revoked", coarse_timestamp}` signed by the revocation private key.

On a valid statement the home server: deletes mailbox, key material, and push endpoint; serves the statement from discovery; appends the statement to every group stream **it hosts** that contains the identity. That append is the only host-only stream object. The host MUST NOT append MLS objects without a member credential. It does not know groups hosted elsewhere.

Clients that verify the statement: mark the contact revoked, stop sending, refuse new sessions from that key, and MUST commit MLS Remove as a `RemoveBundle` in every shared group. The host fans the bundle to the pre-revoke set, then drops the credential. It does not parse MLS.

Hours of lag for 1:1 contacts and foreign-hosted groups is accepted.

### 5.5 End of life without revocation

A lost device that is never revoked remains a live identity until contacts notice silence or eventually fetch a statement. There is no automatic expiry of the identity key.

---

## 6. What the server may know

Phases 1–5 define this list. Phase 8 may store nothing that is not required here or in phases 2–5.

### 6.1 May exist (persistent)

- Server identity key material (the server's own).
- Identity public keys, revocation public keys, signed home-server bindings, revocation statements.
- Finite one-time prekey stock (public).
- Share tokens and contact capabilities mapped to mailboxes — not to identities on the envelope.
- Member credentials and, per hosted group: opaque group id, member capability set, member credentials.
- Peer server ids, public keys, endpoints; advertised TURN/SFU capabilities.

### 6.2 May exist (temporary)

- Padded opaque envelopes in bounded mailboxes and group streams.
- Per-container sequence cursors.
- Internal expiry timestamps, never returned to clients ([ADR-0013](../decisions/0013-internal-server-timestamps.md)).
- Outbound federation queue; push wake state; ephemeral TURN credentials.
- IP addresses only where needed for abuse control, short retention, purpose-bound.

### 6.3 Must not exist

- Message plaintext; permanent history; any user private key; conversation keys.
- Sender identifiers on envelopes.
- Contact nicknames; contact lists (beyond group member capability sets).
- Presence, last-seen, typing, read receipts visible to the server.
- Analytics or request histories tied to identities.
- A field whose purpose is to observe behaviour ([ADR-0022](../decisions/0022-server-side-data-minimisation.md)).

If a client wants typing or read state, it sends it inside the encrypted channel, off by default.

---

## 7. Abuse and availability

A blind server with sealed sender cannot rate-limit by sender. All abuse control is by **capability** and by **peer server** ([README](../../README.md) §46.4).

| Threat | Control (normative direction; numbers in later phases) |
| --- | --- |
| Mailbox flood via leaked contact capability or raced share token | One-shot short-TTL share tokens; rate-limit per contact capability; capability rotation |
| Group-stream flood by stolen member credential | Member credential required to append; mailbox tokens cannot; invites/admits client-signed |
| Prekey exhaustion | Fetch requires a live unused share token; at most one prekey reserved per token; empty stock refuses a **new** 1:1 |
| Push amplification | Opaque wake, one priority class, coalescing; no size/type in the push |
| Call spam | Client-side throttle; signaling is sealed so the server cannot classify it as a call |
| Storage exhaustion | 16 MiB cap; quotas per capability / member credential / peer; bounded mailbox and stream retention |
| Malicious federation peer | Per-peer rate limits; ability to refuse a peer |
| Malicious host stalling a group | Denial of service, not confidentiality loss; remedy is migrate or re-create (group migration deferred) |

Denial of service by a user's own home server is out of scope for confidentiality; the remedy is migration.

---

## 8. Honest claims

The product **does** claim: E2EE of content; server-blind delivery of ciphertext; cryptographic identity not issued by the server; forward secrecy and post-compromise security for 1:1 as provided by PQXDH + Double Ratchet; MLS group confidentiality with commit-driven PCS; sealed sender on envelopes; no identity or state recovery.

The product **does not** claim: multi-device; identity recovery after device loss; protection against a global passive observer in Normal mode; post-quantum groups; anonymous group membership; that disappearing messages bind a malicious recipient; that a stolen live client is harmless before revocation; that TURN hides participation in a call.

---

## 9. Inputs to phase 2

Phase 2 (cryptographic protocol) MUST specify, without reopening this model:

- the hash `H` used in `identity_id` and `server_id`;
- how the identity key signs PQXDH prekeys, MLS key-package init keys, and the home-server binding;
- contact-card encoding (must fit a QR together with a freshly minted share token);
- PQXDH + Double Ratchet policy (one-time prekeys only, as already decided);
- MLS ciphersuite, in-band KeyPackage, periodic Update interval, `RemoveBundle`, `SigningKeyReplace`;
- `RevocationStatement` encoding and verification;
- that 1:1 remains Double Ratchet, not a two-leaf MLS group ([ADR-0005](../decisions/0005-double-ratchet-and-mls.md)).

Phase 3 MUST keep envelopes free of sender fields and distinguishable dummy bits so later privacy modes need no wire change.

---

## 10. Phase completion

Phase 1 is complete: threat model, invariants, trust boundaries, identity lifecycle, and abuse model are written and agree with the cited ADRs. Phase 2 may start.
