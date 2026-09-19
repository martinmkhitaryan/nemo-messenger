# ADR-0010: Every envelope is padded to a fixed size bucket

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 2.3, 25, 26, 43.5, 47, 54 phase 3, 61

## Context

This is the padding rule. Message length leaks content class (one-word reply vs attachment vs call invite) even when the payload is encrypted. Padding cannot be added later without a flag day: any client shipped without it is distinguishable forever.

## Decision

Every envelope is padded to one of a **fixed set of size buckets** before the outer encryption layer. No envelope is ever transmitted at its natural length.

- Padding is an **envelope-layer** rule, always on, including Normal mode (ADR-0021). It is not a privacy-mode option.
- The bucket set is part of the protocol version (ADR-0011). Changing the set is a version bump, not a silent tweak.
- Dummy cover-traffic envelopes (ADR-0021) must land in the same buckets so they are indistinguishable from real ones.
- The concrete bucket sizes are **not** fixed in this record. Phase 3 of ADR-0027 must choose them. Prefer a small set of coarse buckets (maximal size-hiding) over a large set of tight buckets (less waste, more leakage).
- **Attachment envelopes** use a separate larger bucket set. The largest attachment bucket is 16 MiB (ADR-0019). No attachment is sent at natural length.
- For server-to-server (ADR-0016), **`InnerEnvelope` is padded to a versioned outer bucket before `HPKE_Seal`**. `hpke_ciphertext` length MUST NOT reveal the inner mailbox-envelope bucket to Server A or to observers on A → B.

## Pros

- Size leakage is closed from the first client.
- Later cover traffic and constant-rate modes can be added without a wire change.
- Early and late clients are not distinguishable by message length.

## Cons

- Bandwidth waste, especially for tiny messages that jump to the smallest bucket.
- Coarse buckets hide more and waste more; that is the intended security-first trade-off.

## Alternatives considered

### Add padding in the privacy-transport phase (original draft)

Rejected: any client shipped before then would be distinguishable forever.

### Pad to a single constant size

Stronger hiding, extreme waste for attachments. Rejected as the only size; attachments use a larger bucket. A future version may add a large constant-size mode for Maximum Privacy; that would be a new ADR.

### Per-message natural length plus optional padding

Rejected: optional padding is a distinguisher.

## Consequences

- README sections 2.3, 26 and 61 treat padding as always-on envelope behaviour, not as Private-mode transport.
- Phase 3 must publish the bucket set before Phase 4.
- Related: ADR-0011 (version owns the set), ADR-0021 (cover traffic uses the same buckets), ADR-0019 (attachment bucket set and 16 MiB cap).

## History

- 2026-09-19 — Accepted. Every envelope is padded to a fixed size bucket.
