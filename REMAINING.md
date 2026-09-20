# Missing besides Track N

**Status:** working list after call signaling (`4cf55cd`) and binding gossip (`06a75c6`).  
**Not a specification.** `README.md`, `docs/decisions/`, and `docs/protocol/` win if this file disagrees.  
**Delete this file** with `IMPLEMENTATION.md` when I12 freeze is done.

Next free ADR is **0036**. Do not invent schema, ICE, or push tables without a record.

---

## Still open (not Track N)

Protocol types for v1 messages, 1:1 media, groups, vault, and wakeup are in the tree. What is left is **human freeze checks** plus a few README/ADR **MUST**s that run on send today and do not run while the client is merely online.

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
| R1 | Idle discovery refresh | README / ADR-0004: clients MUST refresh discovery on a coarse interval (hours) for 1:1 contacts **and** every MLS-group identity, without waiting for the next send. `send_to` and group send already refresh when stale (`DISCOVERY_REFRESH_SECS`). `fetch_now` / `expire_now` (the 2 s shell loop) do not walk contacts or group members. |
| R2 | MLS Remove on revocation | README / ADR-0004: after a valid `RevocationStatement`, clients MUST commit MLS Remove in every shared group. `refresh_contact` marks `revoked` and send fails closed. `remove_group_member` exists (tests use it). FFI does not emit `RemoveBundle` when discovery shows revoke, and the shell has no Remove control. |
| R3 | Quiet-group MLS Update | README / ADR-0005: clients MUST periodically commit Updates so a quiet group still heals. `maybe_self_update` (7 days) runs on group send / admit / file, not on fetch or online. `UPDATE_ON_ONLINE` is defined and unused by the flush path. |
| R4 | Identity move to another home | README §11: same key, new mailbox, higher binding `seq`, gossip, then `refresh_fanout` under each member credential. The **old** server drops the mailbox when it sees a higher `seq` (implemented). FFI/UI can register once; they cannot re-home, bump `seq`, or call `HomeSession::refresh_fanout`. Group **host** migration stays §52. |

`call_ice` trickle send is not listed: offer/answer wait until ICE gathering completes and put relay candidates in the invite/answer. Receive of `CallIce` already works.

---

## Shipped (do not re-open as missing)

M1–M12 leftover ADR MUST, 1:1 call ringing / reject / cancel / hangup, Private Opus CBR with DTX off, binding-gossip send plus conflict alert, wakeup WebSocket + poll, hosted groups, attachments, reactions / delete / disappear, SQLCipher vault, UniFFI Compose shell (desktop + Android 16).

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
