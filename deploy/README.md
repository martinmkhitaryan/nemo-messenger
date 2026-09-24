# Reference deploy

MIT. Caddy terminates TLS; `nemo-server` listens on `0.0.0.0:8787` by default (`NEMO_LISTEN`). PostgreSQL is started with the frozen DDL ([ADR-0033](../docs/decisions/0033-local-http-api-and-postgres-schema.md)). With `DATABASE_URL` set, the process loads that schema on start and write-throughs after successful mutating requests. Unset `DATABASE_URL` to keep the in-memory engine.

There is no cloud vendor API. Operators run this compose file (or an equivalent Caddy + Postgres + `nemo-server` layout) on their own machines.

## Compose

From the repository root:

```text
docker compose -f deploy/compose.yml up --build
```

Default HTTPS port on the host: **8443**. Caddy uses `tls internal` (a local CA, not public Web PKI) and issues that cert on first handshake (`on_demand`, because the site is a catch-all `:443` published as host port 8443). That certificate is **not** identity: clients fetch `/v1/bundle` and check HPKE against the contact card ([ADR-0033](../docs/decisions/0033-local-http-api-and-postgres-schema.md)). The Nemo client accepts this transport TLS on purpose.

```text
curl -k https://localhost:8443/v1/bundle
```

Federation mTLS is **9443** inside the compose network (`NEMO_S2S_LISTEN`), not through Caddy. Pin peer bundles as `NEMO_PEERS_DIR/*.cbor`.

Optional 1:1 media relay:

```text
docker compose -f deploy/compose.yml --profile calls up --build
```

TURN credentials are issued by `POST /v1/turn` (ADR-0035). Set the same `NEMO_TURN_SECRET` on `nemo-server` and coturn `--static-auth-secret` (compose defaults both to `nemo-dev-turn`). `NEMO_TURN_URL` defaults to `turn:127.0.0.1:3478`; use the host LAN address when an Android device must allocate. Static `NEMO_TURN_USER` / `NEMO_TURN_PASS` remain a client fallback when that route is unreachable.

## Postgres password

The compose file defaults `POSTGRES_PASSWORD` and the matching `DATABASE_URL` to `nemo`. That value is a **reference** so a fresh clone boots. Change it before any real deployment:

```text
export NEMO_POSTGRES_PASSWORD='a-password-you-chose'
docker compose -f deploy/compose.yml up --build
```

Do not reuse `nemo` on a host that is reachable beyond localhost.

## Build prerequisites (Linux)

Host `nemo-ffi` (desktop and anything that compiles `webrtc-audio-processing`) needs Meson and a working libclang for bindgen. Same packages as CI:

```text
./scripts/install-linux-deps.sh
source scripts/dev-env.sh
```

`dev-env.sh` sets `LIBCLANG_PATH`, `BINDGEN_EXTRA_CLANG_ARGS` (fixes bindgen `stddef.h` not found), and Android SDK/NDK env if present. Source it in every new shell (or add it to your profile).

Also need: **Rust stable**, **JDK 21**, Docker (for this compose file). Android additionally needs the SDK + NDK (below).

Without those clang packages you typically see:

```text
fatal error: 'stddef.h' file not found
Unable to generate bindings
```

## Two clients on this home

The Compose Multiplatform shell is AGPL (it links libsignal through `nemo-ffi`). JVM run is the supported desktop path; `jpackage` is optional.

```text
source scripts/dev-env.sh
./scripts/desktop-run.sh
# equivalent: cd apps/compose && ./gradlew run
```

`./gradlew run` builds `nemo-ffi` first and sets `jna.library.path` to `target/debug`. Create two vaults (two process windows, or two data directories), set **Home server** to `https://localhost:8443`, and Register. Local cargo without Caddy still uses `http://0.0.0.0:8787` (reachable as `http://<LAN-IP>:8787` from a phone).

### Windows JVM run

CI builds `nemo_ffi.dll` (`windows-ffi` job). On a Windows machine:

```text
cargo build -p nemo-ffi
cd apps/compose
gradlew.bat run
```

The shell looks for `target/debug/nemo_ffi.dll` on `jna.library.path`. A `.msi` installer is optional; JVM run is the v1 path (ADR-0026 / I11). See [`docs/testing.md`](../docs/testing.md) for MSVC / Meson notes.

Optional native installer (needs a JDK with `jpackage`, and still needs `libnemo_ffi.so` on `jna.library.path`):

```text
cd apps/compose
./gradlew packageDeb
```

## Licence split

`deploy/`, `nemo-wire`, and `nemo-server` are MIT. The desktop binary is AGPL-3.0-only. See [LICENSE.md](../LICENSE.md).

## Android debug APK

The Compose module is one Gradle project for desktop JVM and Android (ADR-0028). Desktop: `./scripts/desktop-run.sh` or `cd apps/compose && ./gradlew desktopTest`. Android needs the SDK + NDK (not a cloud vendor API):

```text
source scripts/dev-env.sh
# one-time SDK packages (paths match apps/compose ndkVersion):
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" \
  "platforms;android-36" "build-tools;36.0.0" "ndk;27.2.12479018" \
  "platform-tools" "emulator" "system-images;android-36;google_apis;x86_64"
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk

# one-time AVD
"$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" create avd -n nemo \
  -k "system-images;android-36;google_apis;x86_64" -d pixel --force

# build + install (starts the 'nemo' AVD if nothing is connected)
./scripts/android-install-debug.sh
```

Or manually: `./scripts/android-emulator.sh` in one terminal, then `cd apps/compose && ./gradlew installDebug`. APK-only (no device): `./gradlew assembleDebug`.

`installDebug` fails with `No connected devices!` when adb sees nothing — start the emulator or plug in a phone with USB debugging (`adb devices` should list a `device`, not only `offline`).

CI on GitHub Actions runs `assembleDebug` with `nttld/setup-ndk` (r27c) and uploads the APK. The native library is `libnemo_ffi.so` via JNA (`arm64-v8a` and `x86_64`).

Register, 1:1, group, and a live call on an emulator or device: [`docs/testing.md`](../docs/testing.md).

## Rate limits (v1 defaults)

These are capability- and peer-scoped. Excess is a generic failure, never "rate limited for Alice" ([docs/protocol/04-delivery-protocol.md](../docs/protocol/04-delivery-protocol.md), [05-federation.md](../docs/protocol/05-federation.md)).

| Limit | Default |
| --- | --- |
| Contact-capability mailbox appends | 30 / minute / capability |
| Federation inners accepted | 100 / second / peer |
| Mailbox owner `ts` | rejected if older than 120 s or more than 120 s in the future |
| Mailbox retention | 14 days / 500 MiB |
| Group stream | 30 days / 2 GiB |

Operators MAY tighten these; they MUST NOT advertise unbounded retention. There is no sender identity to rate-limit.
