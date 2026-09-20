# Phase 5 — Federation

- **Status:** Complete
- **Date:** 2026-09-19
- **Phase:** 5 of 8 ([ADR-0027](../decisions/0027-protocol-first-development-order.md))
- **Depends on:** [04-delivery-protocol.md](04-delivery-protocol.md)
- **Decisions:** [ADR-0016](../decisions/0016-server-to-server-routing.md), [0029](../decisions/0029-cryptographic-identifiers-and-encodings.md), [0032](../decisions/0032-server-signing-key-and-federation-tls.md)

---

## 1. Server keys

| Key | Algorithm | Role |
| --- | --- | --- |
| `server_hpke_public_key` | X25519 | Nested InnerEnvelope HPKE; `server_id = SHA-256(this)` |
| `server_sign_public_key` | Ed25519 | TLS 1.3 identity on the S2S hop; signs ServerBundle |

Rotate HPKE ⇒ new `server_id` ⇒ every user on that server publishes a new `HomeServerBinding` with a higher `seq`. Rotate signing key only: new ServerBundle, same `server_id`, peers pin the new sign key after verifying the old key signed a `server-sign-rotate` payload (context `server-sign-rotate`, CBOR `{new_public_key, seq}`) **or** operators replace pins out of band. v1 MUST support out-of-band pin replace; in-band rotate is SHOULD.

### 1.1 ServerBundle

Canonical CBOR map:

| Key | Field |
| --- | --- |
| 0 | `version` = 1 |
| 1 | `server_id` (32) |
| 2 | `server_hpke_public_key` (32) |
| 3 | `server_sign_public_key` (32) |
| 4 | `host` (tstr, 1–64) |
| 5 | `s2s_port` (uint) |
| 6 | `signature` (64), context `server-bundle` over 0–5 |

Clients: after accepting a contact card, fetch this bundle from the card's `host` (transport may use Web PKI; **identity check is the bundle + card HPKE match**, not the CA). MUST reject HPKE/id mismatch.

Peers: pin bundle at first successful handshake.

---

## 2. TLS hop

- TLS 1.3, `rustls` in this implementation ([ADR-0028](../decisions/0028-implementation-languages-and-libraries.md)).
- Certificates: self-signed Ed25519, SubjectPublicKeyInfo matching `server_sign_public_key`.
- Mutual TLS. Each side aborts if the peer key is not in the pin set.
- S2S SHOULD listen on `s2s_port` on localhost or a private interface; Caddy is for client HTTPS, not a substitute for these pins.
- ALPN: `nemo-s2s/1`.

---

## 3. Frames (after TLS)

```text
counter         u64     per-direction, start 1, monotonic
payload_len     u32
payload         OuterEnvelope.hpke_ciphertext  OR  control CBOR
```

First frame each direction: control CBOR `{0:1, 1:sender_server_id, 2:receiver_server_id}` so a mis-pin fails closed.

Replay: drop `counter ≤ last_seen`. Gaps: fail the connection; reconnect and resume from last committed append (idempotency tokens make this safe).

---

## 4. Forward path

1. Server A has an outbound OuterEnvelope for `destination_server_id = B`.
2. Open or reuse mTLS to B.
3. Send frame with `hpke_ciphertext`.
4. B opens HPKE (info `nemo-v1/s2s-inner`), runs mailbox append ([phase 4](04-delivery-protocol.md)).
5. B replies with control `{ok, seq}` or `{fail, generic}`. No capability echoed back.
6. On `ok`, A drops the outbound row. On `fail` that is not clearly transient, A MUST NOT retry forever; drop after 14 days.

Retry on disconnect: backoff 1s, 2s, 4s, … cap 300s.

---

## 5. Rate limits and refuse

Default: 100 successful appends / second / peer. Excess: `fail` without distinguishing cause.

A server MUST refuse a peer (close TLS, no retry from A until operators intervene) for abuse. Refusal is operational, not a protocol-level identity ban (there is no registry).

---

## 6. Group fan-out

The hosting server is A for each fan-out OuterEnvelope to a member's home server B. Same hop, same limits. Fan-out MUST NOT skip the HPKE inner: the host does not send `delivery_capability` in the clear to a foreign B beyond what InnerEnvelope already contains after B opens it — B is the recipient home and is allowed to see the capability.

---

## 7. Inputs to phase 6

Privacy transport MUST NOT change envelope bytes. It may batch, delay, or send dummy OuterEnvelopes in the same buckets. Tor is an alternative transport for the **client → home** hop, not for S2S in v1.

---

## 8. Phase completion

This implementation binds TLS 1.3 mTLS on `NEMO_S2S_LISTEN` (default `127.0.0.1:{s2s_port}`), ALPN `nemo-s2s/1`. Peers are pinned from `NEMO_PEERS_DIR` (`*.cbor` ServerBundle files) and the `peers` table. Caddy is not on this port.

Phase 5 is complete. Phase 6 may start (and may run conceptually in parallel with 2–5; those phases are already written).
