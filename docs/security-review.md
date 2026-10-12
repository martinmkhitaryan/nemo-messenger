# Security review playbook

How to re-run a review like the 2026-10 audit (findings N-01..N-17) without
starting from zero. This is for future agents and humans. It is not a
substitute for the ADRs.

## Scope

Nemo is an E2EE messenger:

- 1:1: PQXDH + Double Ratchet (libsignal)
- Groups: MLS RFC 9420 suite `0x0003` via OpenMLS
- Untrusted server: invite → accept → admit, host-framed group stream,
  SQLCipher vault, domain-separated Ed25519 (`nemo-v1/<context>\0`)

The server crate must not depend on `nemo-core` or libsignal. The wire crate
must not depend on libsignal.

## Inputs (read first)

1. `docs/decisions/0005`, `0017`, `0018`, `0031`, `0032`, `0033`
2. `docs/protocol/01-security-model.md`, `02-cryptographic-protocol.md`
3. Current code: `crates/nemo-wire`, `nemo-core` (`group.rs`, `home.rs`,
   `identity.rs`, `app.rs`), `nemo-server` (`home.rs`, `group.rs`, `http.rs`,
   `pg.rs`), `nemo-ffi/src/lib.rs`, Android manifests
4. Prior findings in this file and in the audit canvas, if present

Do not invent extra features. A review produces findings; a fix pass only
lands those findings.

## Method

Work in this order. Each step is a question, not a checklist of APIs.

### 1. Threat model

The server is untrusted for confidentiality and membership truth. TLS / Web
PKI is transport, not identity (ADR-0033). Clients authenticate a home via
`ServerBundle` (HPKE + Ed25519) and signed HTTP responses (`nemo-nonce` +
`nemo-server-sig`). Ask: can an on-path attacker, a malicious home, or a
malicious group member violate that?

### 2. SAST (read the code)

For each control, find the *enforcement point* and the *caller*. If they
disagree, that is a finding.

| Control | Where it must hold |
| --- | --- |
| MLS suite 0x0003 | `Group` create/join/admit |
| No External Commit / ReInit / unframed Remove | `apply_handshake` |
| Credential map from ratchet tree | `sync_credential_ids` |
| Own Commit staged until host echo | `pending` + `HandshakeOutcome` |
| Leaf `0xF001` immutable, `0xF002` = leaf sig key | `validate_commit` |
| Admit KeyPackage id = host-reserved id | `Group::admit` expected id + host `lookup_pending` |
| Remove sidecar binds group_id + SHA-256(commit) | `RemoveBundle` |
| Bound invite salt stored on host | `StoredInvite` / `PendingRecord` |
| CapabilityIntro signed, bound to session key | `sign_capability_intro` / `apply_contact_capability` |
| Owner auth scoped + nonced | `MailboxOwnerAuth` + `HomeServer::check_owner` |
| Group create owner-auth | `POST /v1/groups` |
| Credential secrets hashed at rest | `hash_secret` in `GroupHost` |
| Signed prekey rotation | `maybe_rotate_signed_prekey` (48h) |
| Cleartext HTTP only on loopback | `HttpHome::new`, Android network security config |
| `NEMO_HOST_ORIGINS` debug-only | `host_origin_override` |

Search for: unsigned structs that later gained a signature field, `verify(`
call sites missing a new argument, eager `merge_pending_commit`,
`usesCleartextTraffic="true"`, plaintext `credential_secret` compares.

### 3. DAST (prove or refute)

Write a small test or PoC in the crate that owns the code. Run with
`CARGO_TARGET_DIR` inside the repo if the sandbox disk is tight.

A finding is confirmed only if a test fails (or a PoC prints a violation)
on *current* mainline. If the PoC does not reproduce, drop or downgrade
the finding. Proposal-store poisoning was refuted that way once; do not
re-file it without a new proof.

Useful cases:

- Admit with the wrong reserved credential id
- Replay the same owner-auth nonce
- `POST /v1/groups` with no `nemo-owner`
- Forged CapabilityIntro (wrong session key / recipient)
- Welcome that does not match a pending KeyPackage must not consume it
- Remove sidecar captured from group A applied to group B
- Own Commit echo vs a concurrent foreign Commit (`OwnCommitSuperseded`)

Delete throwaway PoCs before finishing. Keep regression tests in
`crates/*/tests`.

### 4. Report

For each finding: id, severity, what breaks, where (file + function),
and the ADR/RFC clause it violates. Separate “code does not match ADR”
from “ADR is silent.”

## Fix pass

1. Make the workspace compile after each cluster of API changes
   (owner-auth scope, MLS admit/remove signatures, `HttpResponse.headers`).
2. Update every call site, including tests. `MailboxOwnerAuth::sign` and
   `check_owner` now take `AuthScope`; compute `server_id` *before* a
   `&mut self` call to avoid borrow errors.
3. MLS tests that admit then encrypt must merge the own Commit echo first
   (`apply_handshake` / `HandshakeOutcome::OwnCommitMerged`).
4. Do not commit unless asked.

## Re-audit after a fix

1. `CARGO_TARGET_DIR` inside the repo if the sandbox disk is tight:
   `CARGO_TARGET_DIR=$PWD/target cargo test --workspace --offline`
2. Re-run the PoCs that originally confirmed findings; they must now fail
   to violate (assert `Err`, 401, or no merge). Keep those as regression
   tests in `crates/*/tests` (do not leave throwaway PoC files).
3. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo deny`
   when disk/network allow. Style lints in `nemo-server` (type complexity)
   are not security findings.
4. Update this playbook if a new class of bug needed a new question.

### FFI sequencing (N-04)

An own MLS Commit is staged in `Group` until it is safe to merge. In FFI,
successful `group_append` of that Commit **is** the sequencer ack: call
`apply_handshake` / `apply_remove_bundle` immediately after a successful
append (admit, remove, self-update). Do not merge before append, and do
not leave `pending` set after append or `encrypt` / `member_count` stay
wrong. If append fails, `discard_pending`.

### Bound invites (N-08)

The host-stored salt is the source of truth. Any live member who can
`lookup_pending` and who holds a valid `InviteeProof` may admit. Tests
must not assume inviter-only admit; assert proof-less joins fail instead.

## Re-audit 2026-10-12 (post N-01..N-17)

Workspace tests: 199 passed, 0 failed (`cargo test --workspace --offline`).

Original PoCs now regression tests:

- N-03: `late_joiner_applies_remove_of_earlier_member` (carol applies Alice's
  Remove of Bob)
- N-04: `concurrent_self_update_supersedes_pending`
- N-05: `group_create_requires_owner`
- N-07: `admit_rejects_key_package_id_mismatch`
- N-08: `bound_invite_requires_inviter_and_proof` (proof-less fails; any
  live member with proof can admit)
- N-10: `owner_auth_nonce_cannot_be_replayed`

Closed: N-01..N-17.

N-09: FFI ingest classifies retryable MLS (`MlsWrongEpoch`, pending, policy)
vs terminal 1:1. `ack` is prefix-only; the cursor stops at the first
retryable seq. After 8 retries the row is skipped with a "lost" marker.
1:1 AEAD / unknown session still ack.

N-16: `NEMO_IDENTITY_FILE` (0600 CBOR) holds HPKE + signing secrets.
Postgres `server_identity` secret columns are nullable and written NULL.
Legacy rows are copied into the file once on start.
