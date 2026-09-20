# Licences

Specification and MIT crates in this tree are MIT. See [LICENSE](LICENSE).

When more implementation crates exist, licences split as follows ([ADR-0028](docs/decisions/0028-implementation-languages-and-libraries.md)):

| Path | Licence | Why |
| --- | --- | --- |
| `README.md`, `docs/`, this file, [LICENSE](LICENSE) | MIT | Specification; independent implementations must be possible |
| `crates/nemo-wire`, `nemo-server`, `deploy/` | MIT | Blind infrastructure; no Double Ratchet; self-hosters can fork without AGPL |
| `crates/nemo-core`, `nemo-ffi`, `apps/` | AGPL-3.0 | Links [libsignal](https://github.com/signalapp/libsignal) (AGPL-3.0-only) |

Rules:

- The server **MUST NOT** depend on `nemo-core` or on `libsignal`.
- The client **MUST** be AGPL-3.0 if it links `libsignal`.
- A Cargo workspace **MUST** fail CI (`cargo-deny` or equivalent) if a MIT crate pulls `libsignal` or a client crate is not AGPL.
- Do not read the root `LICENSE` file as covering a distributed client binary. That binary is AGPL once `libsignal` is linked.

`crates/nemo-wire` (MIT) encodes version-1 cards, signatures, envelopes, and HPKE. `crates/nemo-server` (MIT) is the blind mailbox, group-stream, local HTTP, and S2S engine. `crates/nemo-core` (AGPL-3.0-only) is the client identity, PQXDH/Double Ratchet, and MLS crate. `crates/nemo-ffi` (AGPL) is the UniFFI surface. `apps/compose` (AGPL) is the Kotlin shell. `nemo-wire` and `nemo-server` MUST NOT depend on `libsignal` or `nemo-core`.
