# ADR-0024: Voice call architecture — E2EE signaling, WebRTC media, SFrame keyed from MLS for groups

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 5, 26, 51, 52, 64

## Context

Voice calls need three things a messaging stack does not: low-latency signaling, real-time media transport through NATs, and media encryption that survives a relay. The question is how much of the existing architecture calls reuse, what new infrastructure is required, and how honest the privacy story can be for a traffic type that is long-lived and near constant-rate.

## Decision

Calls are built as three separable parts.

**Signaling** rides the existing end-to-end encrypted conversation. `call_invite`, `call_ringing`, `call_answer`, `call_ice`, `call_reject`, `call_cancel`, `call_end` are application messages in the 1:1 (Double Ratchet) or group (MLS) conversation. Servers see ordinary envelopes. Invites use the smallest `ttl_bucket` (ADR-0014) so stale invites are dropped. ICE candidates travel encrypted because they contain IP addresses.

**Media transport** is WebRTC through TURN. Each server may host TURN (and later an SFU) and advertises them through discovery. TURN credentials are ephemeral, issued per call by the caller's home server, and not tied to identity. Clients MUST set `iceTransportPolicy=relay` and MUST NOT gather host or srflx candidates. Audio is still DTLS-SRTP: TURN forwards ciphertext and never holds media keys. Fingerprints travel inside the encrypted `call_invite` / `call_answer`, so a relay cannot sit in the middle of DTLS.

Always-relay is the call architecture, not a v1 shortcut and not a setting. Direct ICE (host or srflx) would give the peer the user's IP. That is a metadata leak. Encryption of the audio does not hide it. A user-selectable "direct" mode would make the privacy story depend on whoever clicked the faster option, including when the two sides disagree. Do not add that path. A new ADR that supersedes this record is required before any other media path.

**Media encryption:**

- 1:1: DTLS-SRTP via TURN only. The DTLS fingerprints are exchanged inside the encrypted `call_invite`/`call_answer`, so the media path inherits the conversation's authentication. No separate short authentication string is required.
- Groups (deferred, see below): SFrame (RFC 9605) with per-sender keys derived from the MLS exporter of a **call-specific MLS group** containing only the participants. The SFU forwards SFrame ciphertext and never decrypts. Join and leave are MLS Commits, so key rotation is automatic. The caller's home server hosts the SFU; the SFU join capability travels inside the encrypted invite; participants from other servers connect directly to it (no media federation).

**Scope:** 1:1 voice is in the first release. Group calls are v1.1. Video is v1.2.

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

### Direct ICE (host / srflx) or a user-selectable direct path

Rejected. Direct ICE exists in WebRTC to skip the relay: the two devices exchange local and public IPs and try to send media to each other. That is cheaper and often a few tens of milliseconds faster. It is also how the peer learns the user's network address (ISP, approximate location, a stable identifier across calls).

Nemo already treats who-talks-to-whom and from-where as a first-class threat (ADR-0021). Call audio is encrypted either way (DTLS-SRTP). Choosing a direct path does not strengthen confidentiality; it only spends the IP. A toggle is worse than a hard rule: most people will not know what it means, the two sides will disagree, and "most private wins" still means one side can probe the other by offering host candidates.

Always-relay is the coherent choice for this product. TURN operators see that two IPs are in a call and for how long; that cost is stated. Direct ICE is prohibited architecture, same class as the README product prohibitions. Do not list it as leftover work.

### Calls over Tor

Rejected as a supported mode: Tor carries no UDP and TURN-over-TCP-over-Tor gives multi-second latency.

## Consequences

- README section 64 is normative for v1 1:1 calls and design-level for group calls.
- Server capabilities include TURN and SFU endpoints.
- Push (ADR-0020) uses one priority class for every wake, including call invites.
- The privacy modes (ADR-0021) define call behaviour per mode. Always-relay is mandatory wherever calls are available.
- Follow-on and leftover documents must not list a direct media path as unimplemented. The architecture is always-relay ([README](../../README.md) 0.1 and 64).

## History

- 2026-09-19 — Accepted. E2EE signaling, always-relay WebRTC.
- 2026-09-21 — Amendment 1: media path is always TURN. Direct ICE is prohibited, not a later default and not a user choice.
- 2026-09-21 — Amendment 2: always-relay is architecture, not leftover work.
- 2026-09-21 — Amendment 3: group calls are v1.1; video is v1.2.
