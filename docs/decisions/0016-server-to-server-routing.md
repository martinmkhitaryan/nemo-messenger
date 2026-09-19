# ADR-0016: Server-to-server routing with nested encrypted envelopes

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 7, 10, 22, 32, 43, 44, 53.5, 56, 58

## Context

Two routes were considered for a message from Alice (home server A) to Bob (home server B):

1. **Direct:** Alice's client posts straight to Server B's delivery endpoint. Server A is not on the path.
2. **Server-to-server:** Alice posts to Server A; Server A forwards to Server B.

Direct routing hides Alice's traffic from her own server but exposes her IP address and a client session to Bob's server. Plain server-to-server routing hides Alice's IP from Server B but lets Server A see the destination capability of every message she sends, which is her contact graph by token.

## Decision

Messages are routed **server-to-server**, and the inner destination is hidden from the sender's server with a **nested envelope**:

```text
Alice -> Server A
  OuterEnvelope {
    destination_server_id,
    hpke_ciphertext = HPKE_Seal(server_B_public_key, InnerEnvelope)
  }

InnerEnvelope {                  padded to an outer bucket before HPKE (ADR-0010)
  delivery_capability,
  ttl_bucket,                    optional early-expiry request (ADR-0014)
  padded_message_envelope        opaque to every server (ADR-0009, ADR-0010)
}

Server A -> Server B
  forwards hpke_ciphertext over a server-authenticated channel

Server B
  opens the HPKE layer, appends padded_message_envelope to the mailbox behind delivery_capability
```

- Server A learns: Alice sent something to Server B.
- Server B learns: Server A delivered something to this capability. It does not see Alice's IP or client session.
- When **A ≠ B**, neither server sees both ends of the relationship.
- Servers authenticate each other with their server keys (README section 4), not only Web PKI. Server B's public key travels in Bob's contact card as part of the home-server binding (ADR-0006).
- Same-server delivery uses the same structure with A = B. In that case the operator already has Alice's client session and opens the inner HPKE, so it sees Alice → Bob's capability. That graph is an accepted residual (with F1). Do not claim same-server hides it.
- Group fan-out (ADR-0017) uses the same primitive: the hosting server appends into each member's home mailbox via server-to-server delivery.

## Pros

- Sender's IP and client session are never seen by the recipient's server.
- Sender's server can queue while the recipient's server is unreachable.
- Abuse control is between authenticated servers, not against anonymous clients.
- One federation primitive ("append this blob to a capability on that server") covers 1:1 delivery and group fan-out.
- The nested envelope removes the main privacy cost of relaying: the sender's server does not learn the destination mailbox.

## Cons

- One extra hop of latency and one HPKE operation per message.
- Server A still learns which *servers* Alice talks to and how often. On a small federation this approximates the contact graph. When A = B the operator sees both ends (accepted residual).
- Server B's public key must be distributed and rotated via the home-server binding; a stale key means delivery failure until the card is refreshed.
- Server A is a single point of failure and censorship for Alice's outbound traffic. Privacy modes with Tor apply to the Alice → Server A hop.

## Alternatives considered

### Direct to recipient server

Rejected: exposes sender IP and a client session to a server the sender has no relationship with, and prevents server-side outbound queuing. Would have been the choice if hiding traffic from one's own server were the primary goal.

### Selectable route per privacy mode (direct / relay / Tor)

Considered as a superset. Rejected for v1 to keep one routing model; the transport abstraction (README section 32) keeps it possible later. Revisiting requires a new ADR.

## Consequences

- README section 10 and flows 56/58 are rewritten with the nested envelope.
- Server data model gains `ServerPeer` (peer server id, public key, endpoints, trust state).
- Federation protocol (README phase 5) must specify: server authentication, HPKE suite, replay protection on the server-to-server hop, retry and expiry, and rate limiting per peer.

## History

- 2026-09-19 — Accepted. Server-to-server routing with nested encrypted envelopes.
