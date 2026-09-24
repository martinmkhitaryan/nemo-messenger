# Third-party notices

Nemo’s **own** code is licensed per [LICENSE.md](LICENSE.md). Dependencies keep their original licences; incorporating them does **not** put those packages under Nemo’s MIT or AGPL.

This file lists **direct** dependencies that matter for compliance and attribution. It is not a complete transitive tree. Policy: [`deny.toml`](deny.toml).

## Compatibility (why the split exists)

| Component | Nemo licence | Key third-party constraint |
| --- | --- | --- |
| Spec / `nemo-wire` / `nemo-server` / `deploy` | MIT | Must stay free of AGPL/GPL-linked code (no `libsignal`) |
| `nemo-core` / `nemo-ffi` / `apps/compose` | AGPL-3.0-only | Links `libsignal` (AGPL-3.0-only); MIT/Apache deps are fine *into* AGPL |

Direction matters: permissive deps may be used in AGPL Nemo. AGPL/`libsignal` must not be pulled into MIT Nemo crates.

AGPL **allows commercial use**. It is not a “no commercial use” licence. If the goal were source-available / non-commercial-only, that would be a different (non-OSI) licence — and it would still conflict with shipping a `libsignal`-linked binary under that licence alone.

## Client stack (`nemo-core`, `nemo-ffi`, `apps/compose`) — AGPL-3.0-only product

| Dependency | Role | Licence (typical) | Notes |
| --- | --- | --- | --- |
| [libsignal](https://github.com/signalapp/libsignal) (`libsignal-protocol`) | PQXDH / Double Ratchet primitives | AGPL-3.0-only | Forces client AGPL; Nemo does not speak Signal’s service protocol |
| [OpenMLS](https://github.com/openmls/openmls) (`openmls`, providers) | MLS groups | MIT | Copyright (c) 2020 OpenMLS Authors; full text in [NOTICE](NOTICE) |
| `rusqlite` + SQLCipher (vendored) | Local vault | MIT / related | See crate and SQLCipher notices when redistributing |
| `webrtc` / `rtc` | Call media | MIT / Apache-2.0 (check crate) | |
| `webrtc-audio-processing` | Desktop AEC | BSD-3-Clause | Clarified in `deny.toml` |
| `libopus_sys` (bundled Opus) | Audio codec | Opus / BSD-style | |
| `tokio`, `rustls`, `ring`, `reqwest`, … | Async / TLS / HTTP | MIT / Apache-2.0 | |
| `uniffi` | FFI to Kotlin | MPL-2.0 | File-level copyleft; see MPL terms |
| JetBrains Compose / Kotlin / AndroidX / JNA | UI shell | Apache-2.0 | |
| ZXing (`com.google.zxing:core`) | QR | Apache-2.0 | |
| Stream WebRTC Android | Android call media | See upstream | |

## Blind server / wire (`nemo-server`, `nemo-wire`) — MIT product

| Dependency | Role | Licence (typical) |
| --- | --- | --- |
| `nemo-wire` deps (`ed25519-dalek`, `hpke`, `sha2`, …) | Encodings / crypto helpers | MIT / Apache-2.0 / BSD |
| `axum`, `tokio`, `tower` | HTTP / WS | MIT |
| `sqlx` | PostgreSQL | MIT / Apache-2.0 |
| `rustls`, `ring`, `tokio-rustls` | TLS | MIT / Apache-2.0 / ISC-style (ring) |
| `rcgen`, `x509-parser` | Certs | MIT / Apache-2.0 |

These crates **MUST NOT** depend on `libsignal` or `nemo-core`.

## What to ship with a binary

1. [NOTICE](NOTICE)  
2. [LICENSE.md](LICENSE.md)  
3. [LICENSE-MIT](LICENSE-MIT) and/or [LICENSE-AGPL-3.0](LICENSE-AGPL-3.0) as applicable  
4. This file (or a build-generated full inventory)

Do not replace upstream copyright headers inside dependency source with Nemo’s licence.
