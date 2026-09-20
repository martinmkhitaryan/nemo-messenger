# ADR-0030: Envelope version 1 layout, padding buckets, and ttl_bucket set

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 10, 25, 43.5, 53.12, 54; [`docs/protocol/03-envelope-protocol.md`](../protocol/03-envelope-protocol.md)

## Context

[ADR-0010](0010-padding-buckets.md) requires a versioned bucket set before delivery can be specified. [ADR-0011](0011-protocol-version-field.md) requires a version at the start of every envelope. [ADR-0014](0014-sender-chosen-ttl-bucket.md) requires a small `ttl_bucket` set. README 53.12 still listed concrete sizes as open. Nested HPKE ([ADR-0016](0016-server-to-server-routing.md)) must not leak the inner mailbox bucket to the sender's server.

## Decision

Protocol version **1** envelope encodings are those in [`docs/protocol/03-envelope-protocol.md`](../protocol/03-envelope-protocol.md). Summary:

**Mailbox (inner) padded_message_envelope buckets, bytes:** 1024, 4096, 16384. Pick the smallest that fits. Never send natural length.

**Attachment inner buckets, bytes:** 262144 (256 KiB), 1048576 (1 MiB), 4194304 (4 MiB), 16777216 (16 MiB). 16 MiB is the cap ([ADR-0019](0019-attachment-as-padded-envelope.md)).

**Outer InnerEnvelope-before-HPKE buckets, bytes:** 20480 for every non-attachment mailbox envelope (all three inner text buckets map to one outer size). Attachment outers: 266240, 1052672, 4202496, 16781312 (inner attachment bucket plus 4096 bytes of InnerEnvelope overhead, then pad). `hpke_ciphertext` length therefore does not distinguish which *text* inner bucket was used. It does distinguish text-class vs attachment-class, which is accepted (attachments already use a separate set).

**`ttl_bucket` values (uint8):** 0 default (server retention only), 1 = 60 seconds, 2 = 1 hour, 3 = 1 day. `call_invite` MUST use 1.

**HPKE** for the nested inner: RFC 9180 DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305, as in [ADR-0029](0029-cryptographic-identifiers-and-encodings.md).

No sender field ([ADR-0009](0009-sealed-sender.md)). No global message id ([ADR-0012](0012-no-global-message-identifiers.md)). Receipt times never leave the server ([ADR-0013](0013-internal-server-timestamps.md)).

## Pros

- One outer text size meets the ADR-0010 leak rule for S2S text.
- Three coarse inner sizes hide one-word vs paragraph vs short-note without a dozen buckets.
- 16 MiB cap and four attachment buckets match ADR-0019.

## Cons

- Every S2S text envelope costs ~20 KiB on the A→B hop, including a 1 KiB inner. That is the bandwidth cost of hiding the inner bucket from A.
- Attachment-class vs text-class remains visible to Server A by outer length.

## Alternatives considered

### Four outer text buckets matching the three inner plus slack

Rejected: would reveal the inner bucket to Server A, contradicting ADR-0010.

### Single 16 MiB size for everything

Rejected: unusable on mobile for chat.

### Inner set {256, 512, 1024, 4096, …}

Rejected: too many sizes, more leakage. Prefer a small coarse set (ADR-0010).

## Consequences

- README 53.12 concrete bucket sizes are resolved for version 1.
- Phase 4 may specify mailbox append/fetch/ack using these encodings.
- Changing any size is a protocol version bump, not a silent tweak.

## History

- 2026-09-19 — Accepted. Version-1 envelope buckets and layout.
