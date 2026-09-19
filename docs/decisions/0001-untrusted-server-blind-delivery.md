# ADR-0001: The server is untrusted infrastructure; all content is end-to-end encrypted

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 1, 2.2, 7, 8, 41, 45, 46.1

## Context

Every other decision in this project depends on where trust is placed. A messenger can treat its server as a trusted party (it stores plaintext, manages accounts, decides who is who) or as a blind carrier that moves ciphertext it cannot read between parties it cannot fully identify. The choice determines what a server compromise, a malicious administrator or a legal demand can yield.

## Decision

The server is **infrastructure, not authority**. It is assumed to be potentially malicious at all times.

- All message content, attachments and group application data are end-to-end encrypted on the client. There is no unencrypted mode and no server-side decryption path for any purpose (search, moderation, backup, compliance).
- The server stores ciphertext only temporarily, for offline delivery, and never as a message history.
- The server holds no user private keys and no conversation keys, ever.
- The server is not the authority for identity (ADR-0002, ADR-0003): it distributes signed material it cannot forge or alter.
- Server-side persistent state is minimised to what routing requires (ADR-0022).
- A server may drop, delay, replay or reorder traffic and serve stale discovery data; the protocols must tolerate this and detect what can be detected. A server must never be able to read content, forge an identity, silently replace a verified key, or recover historical keys from its own state.

## Pros

- A server database compromise yields ciphertext, public keys and routing state, not messages or identities.
- Operators (including self-hosters) cannot be compelled to produce what they do not have.
- Self-hosting by strangers is safe for users: they need not trust the operator with content.
- The security argument for the whole system reduces to the client and the cryptographic protocols, both of which are open source and auditable (ADR-0025).

## Cons

- No server-side features that need plaintext: search across devices, content moderation, spam filtering by content, link previews generated server-side, message recovery.
- Abuse handling must work on capabilities and peers, never on content or sender (ADR-0009).
- Availability attacks by the server itself (dropping, delaying) cannot be prevented, only detected and routed around by migration.
- Users must understand that losing their device loses their data (ADR-0023).

## Alternatives considered

### Trusted server with transport encryption only

Simplest to build, enables every server-side feature. Rejected: contradicts the product's purpose.

### E2EE by default with an optional server-readable mode (e.g. for "cloud chats")

Rejected: a plaintext path, however optional, becomes the path of least resistance and the target of every demand.

### Server as key authority (server signs user keys)

Rejected: the server could issue a substitute key at will; ADR-0002 and ADR-0006 place that authority with the identity itself and with out-of-band verification.

## Consequences

- Every protocol layer is designed so that the server sees opaque bytes (ADR-0009, ADR-0010) addressed by capabilities (ADR-0008).
- Threat model section 46.1 lists server capabilities and non-capabilities explicitly.
- The "Data that should not exist server-side" list in README section 8 is normative.

## History

- 2026-09-19 — Accepted. Foundational principle of the original draft, recorded as a decision so later proposals must argue against it explicitly.
