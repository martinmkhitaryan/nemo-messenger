# ADR-0021: Metadata privacy as a separate, tunable layer; Tor optional; cover traffic deferred

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 2.3, 24–32, 44, 46.3, 47, 61

## Context

End-to-end encryption hides content but not who talks to whom, when, how much, and from which network address. Protecting that metadata costs bandwidth, battery and latency, and the strongest protections (constant-rate traffic, mix networks) are unusable for most people most of the time. A design must decide whether metadata protection is built into the message protocol, bolted on as a transport, mandatory, or optional.

## Decision

- **Metadata privacy is a separate layer** below the envelope and above the network transport. The cryptographic message format (ADR-0005, ADR-0009–0014) never depends on it and never changes when it changes.
- **Privacy is tunable** through modes the user selects per installation:

```text
Normal      real traffic only; padding buckets (always on, ADR-0010); calls always-relay
Private     + batching, timing jitter; calls always-relay; CBR audio
High        + client-generated cover traffic, stronger timing protection, optional Tor
Maximum     + near constant-rate traffic, aggressive cover, Tor or privacy relays; calls unavailable
```

- **Tor is supported, never required.** The transport interface abstracts direct, Tor and future relay transports; the same protocol runs over all of them.
- **Cover traffic is client-generated**, never server-generated as the primary mechanism (the server knows what it generated). Real and dummy envelopes must be indistinguishable to every server.
- **Cover traffic and constant-rate modes are not in the first release.** The envelope format is fixed so they can be added without a wire change: fixed size buckets, opaque payloads, no field that distinguishes a dummy from a message.
- The product does not claim protection against a global passive observer unless the user is in a mode that actually implements the required mechanisms, and the documentation says which mode provides what.

## Pros

- The messaging protocol can be finished, audited and shipped without waiting for the hardest privacy work.
- Users who need strong metadata protection can pay for it; users who do not are not forced to.
- Client-generated cover traffic defends against the server itself; server-generated traffic would not.
- Padding from day one (ADR-0010) means early clients are not distinguishable from later ones.

## Cons

- Normal mode leaks timing, frequency and IP address to the home server and to the network path. This must be stated plainly.
- Mode fragmentation: users in different modes have different traffic shapes, which itself is a signal.
- Cover traffic and constant-rate designs are deferred, so the "Maximum" mode is a promise about the architecture, not a shipped feature.
- Tor support adds an operational surface (onion services, TCP-only, no calls).

## Alternatives considered

### Mandatory strong privacy for all users (mixnet-style messenger)

Rejected: battery and latency costs make the product unusable as a daily messenger for most users.

### No privacy layer; rely on E2EE and Tor only

Rejected: leaves size and timing leakage unaddressed and makes later additions a wire-format change.

### Server-generated cover traffic as the primary mechanism

Rejected: provides no protection against the server.

## Consequences

- README section 26 modes are normative; section 47 documents the cost of each mechanism.
- Calls (ADR-0024) inherit mode behaviour: always-relay is mandatory in every call-capable mode; CBR in Private and above; unavailable in Tor-only modes.
- Development phase 6 (privacy transport) can proceed independently of phases 2–5.

## History

- 2026-09-19 — Accepted. Metadata privacy as a separate, tunable layer.
