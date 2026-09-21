# ADR-0026: Android, Linux and Windows first; iOS later; shared Rust core

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 33, 51, 54

## Context

Platform choice drives library choice, the push-notification design and the size of the first release. The cryptographic libraries selected in ADR-0005 (libsignal, OpenMLS / mls-rs) are Rust. iOS requires Apple-specific push (APNs, PushKit for calls) and a paid developer program.

## Decision

- The product targets **Android, Linux and Windows**.
- **iOS**, **macOS**, and **web** are out of scope. Nothing in the protocol may assume their absence, and nothing in this tree implements them. A new ADR is required before any of those shells.
- All protocol, cryptographic and storage logic lives in a **single Rust core** with thin platform shells. Per ADR-0003 each installation is one identity, so the core has no cross-device sync layer.

## Pros

- Rust core matches the chosen cryptographic libraries with no FFI to a second language for the security-critical path.
- Android first exercises the opaque push design (README section 33) early.
- Desktop Linux/Windows exercise the long-lived-connection and no-push path.
- Avoids Apple program and review friction during the protocol-stabilisation period.

## Cons

- No iOS at launch excludes a large share of potential users.
- Desktop and mobile installations are separate identities (ADR-0003). That is the product rule, not a gap to close.
- Windows and Linux background delivery without a platform push service needs its own design (persistent connection or periodic poll).

## Alternatives considered

### Mobile only (Android + iOS)

Rejected: delays desktop, adds Apple dependency to the first release.

### Mobile + desktop + web

Rejected: web needs a browser-safe key storage story and WebRTC Encoded Transform for calls; too much for v1.

## Consequences

- Push section of the README must cover FCM (opaque wake) for Android and a non-push path for desktop.
- Call design (README voice-call section) uses libwebrtc via the Rust core on all three platforms; iOS PushKit/CallKit handling is deferred with the platform.

## Amendment 1 (ADR-0028)

The platforms, “no web in v1”, and “single Rust core with thin shells” rules are unchanged. The consequence that calls use libwebrtc via the Rust core on all three platforms is **amended**: signaling and DTLS fingerprint binding stay in the Rust core; the media engine (capture, AEC/AGC/NS, RTP) may live in the platform shell. See [ADR-0028](0028-implementation-languages-and-libraries.md).

## History

- 2026-09-19 — Accepted.
- 2026-09-19 — Amendment 1: media engine may live in the shell (ADR-0028). The Decision section is unchanged.
- 2026-09-21 — Amendment 2: iOS, macOS, and web are out of scope until a new ADR. Android / Linux / Windows is the product, not a first-release subset.
