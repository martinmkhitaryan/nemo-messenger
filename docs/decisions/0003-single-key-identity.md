# ADR-0003: One identity per client installation, no device layer

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 3, 12, 13, 21, 35–37, 43, 51, 53, 55, 59, 62

## Context

The original draft modelled a user as a long-term identity key that owns several device keys (phone, laptop, tablet). That model requires a device enrollment protocol, a signed device list, per-device revocation, cross-device state synchronisation that does not weaken forward secrecy, and custody of a root key that is more valuable than any single device.

Every one of those pieces is a source of protocol complexity and a place where a server or an attacker can try to insert a device. The multi-device layer is also the most complicated part of existing messengers (Signal's Sesame, Matrix cross-signing).

## Decision

There is no separate user identity above the device. **One client installation is one identity.**

- Each installation generates its own long-term identity keypair. `identity_id = H(identity_public_key)`.
- A person using a phone and a laptop has two identities. Contacts see two contacts. Groups must include both.
- There is no key hierarchy, no device list, no enrollment protocol, no linking of installations, and no key that is more privileged than the installation's own key.
- Moving to a new device means creating a new identity and being re-added by every contact. There is no successor certificate. The only off-device control over an identity is revocation (ADR-0004), which kills the identity and cannot authorise a replacement.
- A lost, wiped or stolen device is a lost identity.
- This is the product model, not a v1 limitation. Installations are never linked. A second phone, a laptop, or a reinstall is a new identity.

## Pros

- Smallest possible trust model: nothing to enroll, sync or escrow.
- Contact verification is a single fingerprint that covers every protocol the identity uses.
- An MLS leaf is exactly an identity; a Double Ratchet session is exactly a pair of identities. No fan-out across devices.
- The server can never add a "device" to an account, because the concept does not exist.
- Removes roughly a third of the open questions in the original draft (device enrollment, device revocation, cross-device state sync, key hierarchy custody).

## Cons

- No multi-device experience. Phone and laptop are strangers to each other; history is not shared; every group must add each installation separately.
- No identity recovery. A lost device ends the identity and every contact must re-verify the replacement.
- A stolen device holds a fully functional identity until each contact learns otherwise (mitigated only by ADR-0004).
- Onboarding a second device is socially expensive: the user must be re-invited by every contact.

## Alternatives considered

### Root identity key signing device keys (original draft)

Cold identity key authorises per-device keys; discovery serves a signed, versioned device list. Rejected because it reintroduces enrollment, device revocation, state sync, root-key custody and recovery, all of which the project wants to avoid in v1.

### Peer-linked identities

Two full identities sign a mutual "same person" attestation and clients display them as one contact. No hierarchy. **Rejected.** A new device is a new identity. Do not add linking, pairing, or a "same person" overlay without a record that supersedes this one.

## Consequences

- README sections on device model, device addition, device revocation and compromised-device recovery are removed or rewritten.
- Delivery capabilities are scoped per recipient identity and conversation, not per device.
- Identity portability between servers means moving the mailbox and re-publishing the home-server binding under the same key; it never means moving the key to another device.
- Client implementations must make identity loss explicit in the UX ("this identity cannot be recovered").

## History

- 2026-09-19 — Accepted. One identity per installation; no device layer.
- 2026-09-21 — Amendment 1: one installation = one identity is the product rule, not a deferral. Peer-linked identities are rejected, not a later UX.
