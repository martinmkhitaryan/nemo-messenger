# Licences

This repository uses **two** licences **for Nemo-authored code**, by path. Third-party libraries keep their own licences; see [NOTICE](NOTICE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Do not assume a single GitHub licence badge covers every crate, a shipped client binary, or dependency code.

## Nemo’s code

| Path | Licence | Full text | Why |
| --- | --- | --- | --- |
| Root [`LICENSE`](LICENSE) (pointer), this file | — | — | Map / entry points |
| Specification (`README.md`, `docs/`) | MIT | [`LICENSE-MIT`](LICENSE-MIT) | Independent implementations must be possible |
| `crates/nemo-wire`, `crates/nemo-server`, `deploy/` | MIT | [`LICENSE-MIT`](LICENSE-MIT) | Blind infrastructure; no Double Ratchet; self-hosters can fork without AGPL |
| `crates/nemo-core`, `crates/nemo-ffi`, `apps/` | AGPL-3.0-only | [`LICENSE-AGPL-3.0`](LICENSE-AGPL-3.0) | Links [libsignal](https://github.com/signalapp/libsignal) (AGPL-3.0-only) |

Rules ([ADR-0028](docs/decisions/0028-implementation-languages-and-libraries.md)):

- The server **MUST NOT** depend on `nemo-core` or on `libsignal`.
- The client **MUST** be AGPL-3.0-only if it links `libsignal`.
- A Cargo workspace **MUST** fail CI (`cargo-deny` or equivalent) if a MIT crate pulls `libsignal` or a client crate is not AGPL.
- Do not read a hosting-site “MIT” label as covering a distributed client binary. That binary is AGPL once `libsignal` is linked.

## Third-party code

| Artifact | Purpose |
| --- | --- |
| [NOTICE](NOTICE) | Copyright + required attribution summary for redistributors |
| [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) | Curated direct-dependency notices and compatibility notes |
| [`deny.toml`](deny.toml) | Allowed SPDX set and ban on `libsignal` outside `nemo-core` |

Nemo’s SPDX identifiers apply only to Nemo-authored files. They do not relicense dependencies.

## Crate roles

- `crates/nemo-wire` (MIT) — version-1 cards, signatures, envelopes, HPKE.
- `crates/nemo-server` (MIT) — blind mailbox, group-stream, local HTTP, S2S.
- `crates/nemo-core` (AGPL-3.0-only) — client identity, PQXDH/Double Ratchet, MLS.
- `crates/nemo-ffi` (AGPL-3.0-only) — UniFFI surface.
- `apps/compose` (AGPL-3.0-only) — Kotlin / Compose Multiplatform shell.
- `deploy/` (MIT) — container, Caddy, PostgreSQL, optional coturn.

`nemo-wire` and `nemo-server` **MUST NOT** depend on `libsignal` or `nemo-core`.
