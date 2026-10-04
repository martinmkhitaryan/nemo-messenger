# ADR-0036: Android in-app keyboard (opt-in, nice-to-have)

- **Status:** Proposed
- **Date:** 2026-10-04
- **Affects:** README.md sections 48, 51 (if accepted); ADR-0026, ADR-0028; `apps/compose` `androidMain` only

## Context

Request for a built-in keyboard in the app, scoped to Android + Gboard, as a nice-to-have change. Concern is third-party keyboard exposure (learning, history, network exfil) for message text and secrets.

Current state: the app uses the system IME everywhere. Composer uses default `KeyboardOptions` (`Session.kt:2377`, soft Enter = newline, hardware Enter = send); only the passphrase/secret field opts out of learning (`Session.kt:3839`, `KeyboardType.Password`, no autocorrect). There is no prior decision about a custom keyboard; ADR-0028 chose Compose partly because it supports accessibility and IME.

## Decision

If built, it is an Android-only, in-app Compose keyboard, opt-in, with the system IME remaining the default:

- **In-app view only.** A Composable keyboard inside Nemo. NOT an `InputMethodService` / system IME registration. No Settings enable step, no effect on other apps.
- **Opt-in, default OFF.** System keyboard (Gboard or other) stays the default on all platforms. A setting toggles the in-app keyboard; the user can always fall back.
- **Minimal v1 scope.** One layout set: QWERTY + numbers/symbols, plus delete/space/enter. No swipe, no prediction, no voice, no multi-language layouts in v1.
- **Covers message composer first**, then passphrase/secret fields. All other text fields keep system IME until the keyboard proves usable.
- **No learning, no network, no keystroke persistence** beyond the current text field contents.
- **Desktop unchanged.** Linux/Windows keep system keyboard (ADR-0026 single-UI rule is explicitly diverged for this feature only).
- **Accessibility gate:** TalkBack support and a system-keyboard fallback are required before this can move from Proposed to Accepted.

No change to Rust core, wire format, server, or federation.

## Pros

- Removes one exposure vector: Gboard (or any third-party IME) no longer sees plaintext when the built-in is enabled.
- Works offline with no IME dependency; predictable behavior for privacy-conscious users.
- Small blast radius: `androidMain` only, no protocol/server change.

## Cons

- Large UX cost for parity: languages, emoji, autocorrect, swipe, voice, and accessibility all have to be rebuilt or are lost in v1.
- False sense of security: the OS, screenshots, and accessibility services still see input. Compromised-device plaintext remains out of scope (README 0.1).
- Android-only divergence from the single Compose UI across Android/Linux/Windows (ADR-0026).
- Maintenance burden: keyboard bugs become messenger bugs.

## Alternatives considered

### System-wide IME (`InputMethodService`)

Ship Nemo as a full Android keyboard the user enables in Settings.

Rejected for this scope: requires OS enable flow, affects every other app, invites Play-review and security scrutiny, and is far larger than a nice-to-have in-app view.

### Harden system-IME usage only

Set `flagNoPersonalizedLearning` / incognito hints and disable autocorrect/learning for the composer as well as secrets, without building any keyboard.

Rejected as the sole answer: reduces Gboard retention but Gboard still receives plaintext key events. Kept as a sensible interim step regardless of whether ADR-0036 is built.

### Status quo (system IME everywhere)

Do nothing.

Rejected as the long-term-only answer by the proposer, but retained as the default path in this record: even if the built-in ships, system IME stays default.

## Consequences

- Implementation lives in `apps/compose/composeApp/src/androidMain/kotlin/org/nemo/`; no `nemo-core`, `nemo-wire`, or `nemo-server` change.
- Needs a user-visible setting (default OFF), per-field wiring (composer first), and QA on Android 15+ with Gboard installed.
- README sections 0, 48, and 51 stay unchanged while Proposed. If accepted, add a row to README section 0 and a note in sections 48/51 that Android offers an opt-in in-app keyboard.
- ADR-0028 consequence (Compose IME/accessibility rationale) and ADR-0026 (single UI) need an explicit exception note on acceptance.

## History

- 2026-10-04 — Proposed.
