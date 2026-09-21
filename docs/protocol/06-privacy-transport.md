# Phase 6 — Privacy transport

- **Status:** Complete
- **Date:** 2026-09-19
- **Phase:** 6 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** envelope bytes from [03](03-envelope-protocol.md); MUST NOT change them
- **Decisions:** [ADR-0021](../decisions/0021-privacy-layer-and-modes.md)

Cover traffic and constant-rate modes are implemented in the client. This document freezes the transport interface so those modes do not change envelope bytes.

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
| High | Private + client-generated cover to a contact; Tor SOCKS for client→home when the origin is not loopback |
| Maximum | ~2 s cover slots; Tor SOCKS when not loopback; calls unavailable |

v1 MUST ship Normal and Private. High with cover ships. Tor SOCKS is used for non-loopback homes. Dummy envelopes are type-valid phase-3 objects in the same buckets, indistinguishable to every server.

---

## 3. Push

Unaffected: opaque wake, no size/type ([ADR-0020](../decisions/0020-opaque-push-wakeup.md)). Coalescing: at most one wake per 10 s per device. Desktop: no FCM; long-lived HTTPS/WSS to home while running.

---

## 4. Phase completion

Phase 6 is complete. Envelope bytes are unchanged across modes.
