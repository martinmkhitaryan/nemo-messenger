# Remaining work

**Status:** leftover code from the last audit is in the tree.  
**Not a specification.** `README.md`, `docs/decisions/`, and `docs/protocol/` win if this file disagrees.  
**Delete this file** with `IMPLEMENTATION.md` when I12 freeze is done.

How to run the freeze checks: [`docs/testing.md`](docs/testing.md).

Next free ADR is **0036**. Do not invent schema, ICE, or push tables without a record.

---

## Still open

Product verification. CI cannot do these.

| Item | Why it is still open |
| --- | --- |
| Live audible 1:1 call | Capture, AEC, Opus CBR (Private), relay ICE, and `call_ice` trickle are in code. Confirm two clients through local coturn with a real mic. Direct ICE still refused. |
| Android APK on a device | CI `assembleDebug` exists. Install on Android 16 and complete register / 1:1 / group / call. |
| Delete `IMPLEMENTATION.md` | I12 last checkbox. Only after the two rows above. |

---

## Track N (nice-to-have)

Tor / Arti, cover traffic / Maximum, group calls + video, FCM HTTP v1 (needs **ADR-0036** — no push table in frozen DDL).

---

## Out of v1 by README §52

iOS, macOS, web, multi-device, key transparency, group migration, anonymous group membership, global directory lookup.

---

## Decision gates

Stop for: FCM table, OS keystore wrapping SQLCipher, incompatible Argon2 bump, MLS storage-provider swap, P2P ICE, JSON protocol objects, libsignal on the server.
