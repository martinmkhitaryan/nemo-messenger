# ADR-0025: Self-hostable, open source, established cryptography only

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 1, 12, 49, 50

## Context

A messenger that claims the server is untrusted (ADR-0001) must let anyone run the server and let anyone inspect the client, or the claim cannot be checked. Cryptographic protocol design is also the place where well-meaning projects fail most often, by inventing primitives or protocols instead of reusing analysed ones.

## Decision

- **Self-hostable.** The complete server (discovery, delivery, federation peer, push integration, optional TURN/SFU) runs from a container plus a database plus a reverse proxy, with no dependency on a central service operated by the project. Nothing in the protocol depends on a specific cloud provider or on infrastructure the project controls.
- **Open source.** All security-critical components (client core, protocol implementations, server) are published under an open-source licence. The protocol specification is maintained separately from the implementation so that independent implementations are possible.
- **Established cryptography only.** The project uses standardised or formally analysed protocols (PQXDH, Double Ratchet, MLS, HPKE, SFrame) through audited libraries (libsignal, OpenMLS/mls-rs or equivalents). No custom primitives, no custom protocols, no "tweaks" to standard ones. Any deviation requires a decision record and external review.
- **Auditability.** Reproducible builds where practical; explicit threat model documentation (README section 46); independent security review before claiming security properties publicly.

## Pros

- Users and reviewers can verify that the server is blind and the client does what it says.
- Anyone can run a server for themselves or their community; the project is not a single point of trust or failure.
- Reusing analysed protocols means the security argument leans on existing literature and audits, not on this project's own claims.
- Independent implementations are possible, which is the real test of a specification.

## Cons

- No control over server quality: badly run servers exist and users may pick them.
- Library choices constrain the implementation language and platform support (see ADR-0026).
- Standard protocols sometimes lack a desired property (e.g. MLS post-quantum ciphersuites today); the project waits rather than invents.
- Open source does not stop malicious forks; verification UX must make key changes visible so that a modified client cannot silently impersonate.

## Alternatives considered

### Hosted service with optional self-hosting later

Rejected: "later" tends not to arrive, and the protocol would grow dependencies on the hosted environment.

### Custom protocol optimised for this design

Rejected: unanalysed cryptography is the most common way messengers fail.

### Source-available but not open-source licence

Rejected: restricts independent implementation and audit.

## Consequences

- README sections 49 and 50 are normative.
- Library selection (ADR-0005, ADR-0026) must prefer audited implementations even at a feature cost.
- Any proposal to alter a standard protocol requires a new decision record with an explicit review plan.

## History

- 2026-09-19 — Accepted. Product principle of the original draft, recorded as a decision.
