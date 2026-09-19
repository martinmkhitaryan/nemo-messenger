# ADR-0020: Opaque push wake-up; the device fetches its own ciphertext

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 33, 43, 64.6

## Context

Mobile platforms suspend applications; a message or call cannot reach a suspended client without the platform's push service (FCM on Android, APNs on iOS). Those services are operated by Google and Apple and see everything placed in the push payload. Many messengers put encrypted message content, or at least conversation identifiers, into the push.

## Decision

Push notifications are **opaque wake-ups only**.

- The push payload contains a wake token and nothing else: no ciphertext, no conversation or sender identifier, no counts, no preview, **no size and no type hint** (a 16 MiB attachment wake must be indistinguishable from a one-word text wake).
- On wake, the client opens its own authenticated connection to its home server and fetches pending envelopes from its mailbox (ADR-0015).
- The home server holds one push endpoint per identity, registered by the client, and deletes it on revocation (ADR-0004).
- The server coalesces pushes: many envelopes arriving in a short window produce one wake.
- Every wake on a platform uses the **same priority class** and the same opaque payload. Call invites (ADR-0024) are not marked high-priority relative to messages. The client fetches and decrypts before ringing.
- Desktop platforms without a push service keep a persistent connection or poll while the application runs (ADR-0026).

## Pros

- The push provider learns only that this installation received *something*, and when. It learns no content, no counterparties, no message counts beyond coalesced wakes.
- The push endpoint is the only link between the identity and a platform account, and it is deleted on revocation.
- No key material is ever near the push path.

## Cons

- Extra round trip on wake (push, then fetch) adds latency and battery cost versus payload-carrying pushes.
- Notification content (sender name, preview) cannot be shown until the fetch and decrypt complete; on constrained platforms this may fail.
- Timing of pushes still leaks activity patterns to the push provider; coalescing and, in higher privacy modes, batching (ADR-0021) reduce but do not remove this.
- iOS requirements for call pushes (PushKit must report a call promptly) constrain how long the fetch may take; specified when iOS is added.

## Alternatives considered

### Encrypted payload in the push

The push carries the ciphertext, decrypted on device. Rejected: the payload size, arrival pattern and per-conversation routing keys leak more to the provider, and it places ciphertext in a third party's queue.

### Self-hosted push (UnifiedPush / persistent connection)

Attractive for privacy and consistent with self-hosting. Not chosen as the only path because platform power management makes it unreliable on stock Android. Kept as an optional transport for the wake-up; the protocol does not depend on which provider wakes the device.

## Consequences

- Data model has `PushEndpoint {identity_id, provider, opaque_endpoint, created_at}`.
- Server-side push coalescing rules and the wake token format are open protocol details (README 53.6).

## History

- 2026-09-19 — Accepted. Opaque push wake-up; the device fetches its own ciphertext.
