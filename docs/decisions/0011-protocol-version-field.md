# ADR-0011: Every protocol object begins with a version field

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 3.4, 10, 43.5, 54 phase 3

## Context

This is the versioning rule. Without a version on the wire, a ciphersuite or padding-bucket change requires a parallel protocol or a flag day.

## Decision

Every envelope, contact card, key package wrapper and server-to-server frame **begins with a protocol version**.

- Ciphersuite agility is expressed through that version, not through per-message negotiation.
- The padding bucket set (ADR-0010) is defined by the version.
- Unknown versions are rejected. There is no silent fallback to an older parser that might accept a weaker object.
- Contact cards include `version` as the first field (ADR-0006).

## Pros

- Post-quantum or ciphersuite changes do not need a second protocol.
- Clients can refuse objects they cannot authenticate under a known version.

## Cons

- Old clients cannot read new-version objects until they upgrade. That is accepted; security-first means no downgrade.

## Alternatives considered

### Per-message ciphersuite identifiers without a document version

Rejected: mixes agility with framing and makes it easier to offer a weak suite to one peer.

### Implicit version (first release has none)

Rejected: the first client without a version field cannot be extended safely.

## Consequences

- Phase 3 must fix the version encoding before Phase 4.
- Related: ADR-0006 (contact card), ADR-0010 (bucket set is versioned).

## History

- 2026-09-19 — Accepted. Version-field rule, numbered in reading order.
