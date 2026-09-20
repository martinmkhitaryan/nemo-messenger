# Phase 6 — Privacy transport

- **Status:** Complete (v1 subset)
- **Date:** 2026-09-19
- **Phase:** 6 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** envelope bytes from [03](03-envelope-protocol.md); MUST NOT change them
- **Decisions:** [ADR-0021](../decisions/0021-privacy-layer-and-modes.md)

Cover traffic and constant-rate modes remain **deferred**. This document freezes the v1 transport interface and mode behaviour so those modes can be added without a wire change.

---

## 1. Interface

```text
send(outer_envelope_bytes)     // already padded and HPKE-sealed as required
recv() -> outer or inner results from home server
```

Implementations:

| Name | v1 |
| --- | --- |
| `direct` | HTTPS client → home (Caddy), then S2S as phase 5 |
| `tor` | Same protocol over Arti ([ADR-0028](../decisions/0028-implementation-languages-and-libraries.md)); client → home only |
| `relay` | Reserved; not shipped |

S2S stays direct mTLS. Tor-on-S2S is out of v1.

---

## 2. Modes ([ADR-0021](../decisions/0021-privacy-layer-and-modes.md))

| Mode | v1 behaviour |
| --- | --- |
| Normal | Real traffic; padding always on; calls always-relay |
| Private | + batch send window 0–5 s jitter; + 20–200 ms extra delay; calls always-relay; Opus CBR, DTX/VAD off |
| High | Private + optional Tor for client→home; cover traffic **not** shipped; document as partial |
| Maximum | Not shipped; calls unavailable; when shipped: near constant-rate + cover + Tor |

v1 MUST ship Normal and Private. High with Tor SHOULD. Maximum MUST NOT be claimed.

Dummy envelopes, when added, are type-valid phase-3 objects in the same buckets, indistinguishable to every server.

---

## 3. Push

Unaffected: opaque wake, no size/type ([ADR-0020](../decisions/0020-opaque-push-wakeup.md)). Coalescing: at most one wake per 10 s per device. Desktop: no FCM; long-lived HTTPS/WSS to home while running.

---

## 4. Phase completion

Phase 6 v1 subset is complete. Phase 7 (application protocol) may start.
