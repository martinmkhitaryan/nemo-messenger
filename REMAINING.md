# Missing besides Track N

**Status:** working list after identity re-home (R4).  
**Not a specification.** `README.md`, `docs/decisions/`, and `docs/protocol/` win if this file disagrees.  
**Delete this file** with `IMPLEMENTATION.md` when I12 freeze is done.

Next free ADR is **0036**. Do not invent schema, ICE, or push tables without a record.

---

## Still open (not Track N)

Protocol types for v1 messages, 1:1 media, groups, vault, and wakeup are in the tree. R1–R4 specified MUST work is in the client/server path. What is left is **human freeze checks**.

### Freeze checks

Product verification, not missing codecs.

| Item | Why it is still open |
| --- | --- |
| Live audible 1:1 call | Capture, AEC, Opus CBR (Private), and relay ICE are in code. CI has no microphone. Confirm two clients through local coturn with a real mic. Direct ICE still refused. |
| Android APK on a device | CI `assembleDebug` exists. Install on Android 16 and complete register / 1:1 / group / call. |
| Delete `IMPLEMENTATION.md` | I12 last checkbox. Only after the two rows above. |

### Specified MUST not fully on the idle path

| ID | Item | Where it stops today |
| --- | --- | --- |
| R1 | Idle discovery refresh | **Done** — `expire_now` / `fetch_now` walk stale 1:1 pins and group identities (`DISCOVERY_REFRESH_SECS`). |
| R2 | MLS Remove on revocation | **Done** — idle sweep emits `RemoveBundle` for each shared group and a `revoked` row. |
| R3 | Quiet-group MLS Update | **Done** — `maybe_self_update` also fires after `UPDATE_ON_ONLINE` (24 h) and is flushed from the idle path. |
| R4 | Identity move to another home | **Done** — `POST /v1/binding` maps `observe_binding`; `HomeSession::rehome` mints a higher `seq`, registers on B, restocks, refreshes fan-out on the old host, and notifies A. FFI `register` while already registered moves home, gossips the binding, and sends new contact capabilities. Group **ops stay on the original host** (`HostGroup.host_base`); moving the group itself stays §52. |

`call_ice` trickle send is not listed: offer/answer wait until ICE gathering completes and put relay candidates in the invite/answer. Receive of `CallIce` already works.

---

## Shipped (do not re-open as missing)

M1–M12 leftover ADR MUST, 1:1 call ringing / reject / cancel / hangup, Private Opus CBR with DTX off, binding-gossip send plus conflict alert, idle discovery refresh, MLS Remove on revoke, quiet-group Updates, identity re-home (R4), wakeup WebSocket + poll, hosted groups, attachments, reactions / delete / disappear, SQLCipher vault, UniFFI Compose shell (desktop + Android 16).

---

## Track N (nice-to-have, not missing v1)

User-deferred. Do not treat as open MUST.

| Item | Where it is deferred |
| --- | --- |
| Tor / Arti | Track N, ADR-0021 (`High` hop flag only until then) |
| Cover traffic, constant-rate, Maximum mode | ADR-0021, README §52 |
| Group calls (SFU + SFrame), video | ADR-0024, README §52 |
| FCM HTTP v1 | Track N / I10b; needs **ADR-0036** because `0001_init.sql` has no push table |

Wakeup WebSocket + poll is the shipped non-Google path (ADR-0020 / ADR-0028).

---

## Out of v1 by README §52

iOS, macOS, web, multi-device, history export, key transparency, group migration, anonymous group membership, global directory lookup.

---

## Decision gates

Still stop for: FCM table, OS keystore wrapping SQLCipher, incompatible Argon2 bump, MLS storage-provider swap, P2P ICE, JSON protocol objects, libsignal on the server.
