# ADR-0024: Voice call architecture — E2EE signaling, WebRTC media, SFrame keyed from MLS for groups

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 5, 26, 51, 52, 64

## Context

Voice calls need three things a messaging stack does not: low-latency signaling, real-time media transport through NATs, and media encryption that survives a relay. The question is how much of the existing architecture calls reuse, what new infrastructure is required, and how honest the privacy story can be for a traffic type that is long-lived and near constant-rate.

## Decision

Calls are built as three separable parts.

**Signaling** rides the existing end-to-end encrypted conversation. `call_invite`, `call_ringing`, `call_answer`, `call_ice`, `call_reject`, `call_cancel`, `call_end` are application messages in the 1:1 (Double Ratchet) or group (MLS) conversation. Servers see ordinary envelopes. Invites use the smallest `ttl_bucket` (ADR-0014) so stale invites are dropped. ICE candidates travel encrypted because they contain IP addresses.

**Media transport** is WebRTC: ICE/STUN/TURN for 1:1, an SFU for groups. Each server may host TURN and an SFU and advertises them through discovery. TURN credentials are ephemeral, issued per call by the caller's home server, and not tied to identity. Media is **always relayed through TURN**. There is no peer-to-peer ICE. Clients MUST set `iceTransportPolicy=relay` and MUST NOT gather host or srflx candidates, so the peer never learns the user's IP. Direct ICE (host or srflx) would show the peer the user's IP; that is a privacy loss, not a security gain. There is no user-selectable P2P mode. A new ADR that supersedes this record is required before any direct ICE.

**Media encryption:**

- 1:1: DTLS-SRTP via TURN only. The DTLS fingerprints are exchanged inside the encrypted `call_invite`/`call_answer`, so the media path inherits the conversation's authentication. No separate short authentication string is required.
- Groups (deferred, see below): SFrame (RFC 9605) with per-sender keys derived from the MLS exporter of a **call-specific MLS group** containing only the participants. The SFU forwards SFrame ciphertext and never decrypts. Join and leave are MLS Commits, so key rotation is automatic. The caller's home server hosts the SFU; the SFU join capability travels inside the encrypted invite; participants from other servers connect directly to it (no media federation).

**Scope:** 1:1 voice is in the first release. Group calls and video are deferred (README section 52).

**Privacy limits, stated explicitly:** Opus CBR with DTX/VAD off in Private mode and above (VBR packet sizes leak speech content). Calls are unavailable in Tor-only modes (no UDP, unusable latency). TURN/SFU operators see who is in a call by IP and for how long.

## Pros

- No new signaling channel or server component; signaling privacy equals messaging privacy.
- Media keys are bound to the same verified identities as messages; verifying a contact once covers calls.
- SFrame + MLS exporter is the standardised path for E2EE group calls through an SFU and matches the group protocol already chosen (ADR-0005).
- TURN and SFU are commodity self-hostable components (coturn, LiveKit, mediasoup, Janus).
- ADR-0003 (one installation, one identity) removes the multi-device ring race entirely.

## Cons

- Two media-key paths (DTLS-SRTP for 1:1, SFrame/MLS for groups) because of ADR-0005.
- A call is the worst case for traffic analysis; the privacy layer can shape it only partially.
- New infrastructure (TURN, later SFU) with its own operational and abuse surface; call spam must be throttled client-side because signaling is sealed.
- Group call E2EE depends on SFrame support in the chosen WebRTC stack (libwebrtc frame encryptor / browser Encoded Transform).
- Cross-server TURN credential issuance is an open item (README 53.12).

## Alternatives considered

### Dedicated signaling service outside the E2EE channel

Faster to ring, simpler server logic. Rejected: exposes call metadata (who calls whom, when, how long) to a server in the clear.

### Hop-by-hop SRTP through the SFU without SFrame

Rejected: the SFU would hold media plaintext, violating ADR-0001.

### Deriving 1:1 media keys from the Double Ratchet instead of DTLS-SRTP

Possible and slightly stronger (no DTLS handshake trust). Not chosen for v1 because DTLS-SRTP with authenticated fingerprints is the well-trodden WebRTC path; may be revisited.

### Calls over Tor

Rejected as a supported mode: Tor carries no UDP and TURN-over-TCP-over-Tor gives multi-second latency.

## Consequences

- README section 64 is normative for v1 1:1 calls and design-level for group calls.
- Server capabilities include TURN and SFU endpoints.
- Push (ADR-0020) uses one priority class for every wake, including call invites.
- The privacy modes (ADR-0021) define call behaviour per mode. Always-relay is mandatory wherever calls are available.

## History

- 2026-09-19 — Accepted. E2EE signaling, always-relay WebRTC, no P2P.
- 2026-09-21 — Amendment 1: P2P ICE is forbidden (peer IP leak). Not a later default and not a user choice.
