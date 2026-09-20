# Remaining work

**Status:** working list after leftover ADR MUST work (M1–M12) shipped.  
**Not a specification.** `README.md`, `docs/decisions/`, and `docs/protocol/` win if this file disagrees.  
**Delete this file** when I12 freeze is done (`IMPLEMENTATION.md` deleted or replaced with a one-paragraph shipped note).

Next free ADR is **0036**. Do not invent schema, ICE, or push tables without a record.

---

## Before freeze

These are product checks, not missing protocol.

| Item | Why it is still open |
| --- | --- |
| Live audible 1:1 call | Capture, AEC, Opus, and relay ICE are in code. Private mode uses Opus CBR with DTX off (ADR-0024). CI has no microphone. Confirm two clients through local coturn with a real mic; Direct ICE still refused. |
| Android APK on a device | CI `assembleDebug` exists. Install on Android 16 and complete register / 1:1 / group / call. |
| Delete `IMPLEMENTATION.md` | I12 last checkbox. Only after the two rows above. |

---

## Track N (nice-to-have, not v1 blockers)

User-deferred. Do not treat as missing v1.

| Item | Where it is deferred |
| --- | --- |
| Tor / Arti | Track N, ADR-0021 (High hop flag only until then) |
| Cover traffic, constant-rate, Maximum mode | ADR-0021, README §52 |
| Group calls (SFU + SFrame), video | ADR-0024, README §52 |
| FCM HTTP v1 | Track N / I10b; needs **ADR-0036** because `0001_init.sql` has no push table |

Wakeup WebSocket + poll is the shipped non-Google path (ADR-0020 / ADR-0028).

---

## Out of v1 by README §52

iOS, macOS, web, multi-device, history export, key transparency, group migration.

---

## Decision gates

Still stop for: FCM table, OS keystore wrapping SQLCipher, incompatible Argon2 bump, MLS storage-provider swap, P2P ICE, JSON protocol objects, libsignal on the server.
