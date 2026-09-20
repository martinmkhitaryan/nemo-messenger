# Phase 7 — Application protocol

- **Status:** Complete for v1 message types
- **Date:** 2026-09-19
- **Phase:** 7 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** [02-cryptographic-protocol.md](02-cryptographic-protocol.md) (what the AEAD wraps)
- **Decisions:** [ADR-0012](../decisions/0012-no-global-message-identifiers.md), [0015](../decisions/0015-bounded-mailbox-log-and-acks.md), [0023](../decisions/0023-no-backup-no-recovery.md), [0024](../decisions/0024-voice-call-architecture.md)

Application plaintext lives **only** inside Double Ratchet or MLS. Servers never see these fields.

Canonical CBOR map, key 0 = `version` (uint, 1), key 1 = `msg_type` (tstr).

---

## 1. Common fields

| Key | Field | Notes |
| --- | --- | --- |
| 0 | `version` | 1 |
| 1 | `msg_type` | see §2 |
| 2 | `conv_seq` | uint, per-conversation, client-assigned; **not** a server seq |
| 3 | `sent_at` | uint, sender Unix seconds; a claim |
| 4 | `reply_to` | optional uint, `conv_seq` in this conversation |

No UUID. `conv_seq` is local to the pair or group as the clients agree (start at 1 after session/group creation).

---

## 2. `msg_type` values (v1)

| `msg_type` | Extra keys | Notes |
| --- | --- | --- |
| `text` | 5 `text` (tstr, UTF-8, max 8192 bytes) | |
| `attachment` | 5 `enc_file` (bstr, inner file ciphertext), 6 `meta` (map: `name`, `mime`, `size` plaintext of original — these are inside E2EE) | 1:1: whole application message padded as A* DR envelope. Groups: `meta` + `fetch_token` (key 7, 32 bytes) in MLS app msg; file bytes on host |
| `reaction` | 5 `target` (`conv_seq`), 6 `emoji` (tstr, max 32) | |
| `delete` | 5 `target` | Delete-for-everyone **request**; cooperating clients hide; not enforcement |
| `protocol_ack` | 5 `upto` (`conv_seq`) | Batch allowed |
| `capability` | 5 `contact_capability` (bstr 32) | Recipient's new write token for this conversation |
| `binding_gossip` | 5 `identity_id`, 6 `seq`, 7 `server_id` | Gossip home-server binding |
| `disappear` | 5 `seconds` (uint) or 0 = off | Per-conversation setting; last one wins; show in UI |
| `call_invite` | 5 `call_id` (bstr 16), 6 `sdp`, 7 `dtls_fp` (tstr), 8 `ice` (array of tstr), 9 `expires_at` | Envelope `ttl_bucket` = 1 |
| `call_ringing` | 5 `call_id` | |
| `call_answer` | 5 `call_id`, 6 `sdp`, 7 `dtls_fp`, 8 `ice` | |
| `call_ice` | 5 `call_id`, 6 `ice` | |
| `call_reject` / `call_cancel` / `call_end` | 5 `call_id` | |

Unknown `msg_type`: ignore, do not disconnect the session.

`call_id` is random per call, not a server id. ICE: relay candidates only; MUST NOT include host or srflx ([ADR-0024](../decisions/0024-voice-call-architecture.md)).

---

## 3. Group application extras

MLS application messages use the same map. Membership changes are MLS Commits, not `msg_type`. Invites/admits are host objects plus MLS Add, not these maps.

---

## 4. Disappearing messages

After `seconds` from `sent_at` (sender claim) **or** from local display, whichever the setting defines: v1 is **from display**. Delete plaintext and message keys. Envelope `ttl_bucket` SHOULD be 2 or 1 so undelivered ciphertext dies. Not a confidentiality guarantee against the other party.

---

## 5. Inputs to phase 8

Phase 8 may expose:

- client HTTPS/WSS to home: register, discovery get, prekey upload/fetch (with share token), OuterEnvelope post, mailbox fetch/ack, group append (member credential);
- S2S as phase 5;
- Postgres tables that store **only** phase 1–5 fields (public keys, bindings, capabilities, padded blobs, seq, expiry, member credentials, peer pins).

No table for plaintext, nicknames, receipts, or senders.

---

## 6. Phase completion

Phase 7 v1 application types are complete. Phase 8 (API and persistence) may start when an implementation is written; it MUST NOT add server-stored fields that phases 1–5 did not require.
