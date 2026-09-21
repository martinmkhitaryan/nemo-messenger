# Manual product tests

v1 automated tests run in CI (`cargo test`, `desktopTest`, Android emulator instrumented tests). This file is the **live audible 1:1** you still run by hand, plus how to point a phone at a local home.

Two vaults in one desktop window count as two identities. Headphones on at least one side, or you hear yourself twice.

Follow-on: [`v1.1.md`](v1.1.md) (group calls, FCM), then [`v1.2.md`](v1.2.md).

---

## What CI already covers

From the repo root:

```text
cargo test --workspace
cd apps/compose && ./gradlew desktopTest
cd apps/compose && ./gradlew assembleDebug
```

`desktopTest` registers two vaults on a local `nemo-server`, exchanges text and a file, and joins a group.

Android instrumented tests run on an Android 16 (API 36) emulator in CI (`connectedDebugAndroidTest`). They are the Android product suite: create-screen copy, QR, vault, 1:1 text/file, group join. They do not listen to a microphone. A live audible call is still a manual check.

```text
# host server the emulator reaches as 10.0.2.2
NEMO_LISTEN=0.0.0.0:18787 NEMO_S2S_LISTEN=127.0.0.1:19443 cargo run -p nemo-server
cd apps/compose && ./gradlew connectedDebugAndroidTest
```

An ADB MCP can drive extra taps in a Cursor chat. CI uses Gradle, not MCP.

---

## Run the same checks as GitHub before pushing

These are the `test` and `android` jobs in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml). They are not the live-mic freeze.

```text
# licences + libsignal ban (needs: cargo install cargo-deny --locked)
cargo deny check

cargo test --workspace
cargo test -p nemo-wire -p nemo-server --test no_libsignal

# desktop Compose (needs a JDK 21)
cd apps/compose && ./gradlew --no-daemon desktopTest

# APK (needs Android SDK 36 + NDK r27c + cargo-ndk)
cd apps/compose && ./gradlew --no-daemon assembleDebug
```

`cargo-deny` must parse `deny.toml`. The `allow` list is SPDX identifiers only (`MIT`, `ISC`, `OpenSSL`, …), not `ISC AND MIT AND OpenSSL`.

The Android GitHub job also installs SDK packages with `android-actions/setup-android`. Do not request the old `tools` package; Google removed it. Local SDK manager:

```text
sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "emulator"
```

Emulator instrumented tests still need a running `nemo-server` on `0.0.0.0:18787` (see below). The Windows job is `cargo build -p nemo-ffi` and only runs on Windows or in GitHub. Linux `cargo build -p nemo-ffi` does not catch this. On a Windows machine, use an “x64 Native Tools” prompt so `cl` is `CC`/`CXX` (sqlcipher’s OpenSSL nmake is `VC-WIN64A`; GNU `clang` rejects `/Zi`). Keep Strawberry Perl’s `c++` off `PATH`, but leave Strawberry `perl` on `PATH` (Git’s MSYS perl cannot run `Configure`). Use a short `CARGO_TARGET_DIR` (`MAX_PATH`). MSVC needs C++20 for APM designated initializers; GitHub uses a `cl.exe` shim that rewrites `/std:c++17` to `/std:c++20` (Cargo will not run a `meson.cmd` wrapper):

```text
pip install meson ninja
set CARGO_TARGET_DIR=C:\t
cargo build -p nemo-ffi
```

GitHub’s default `cc` is Strawberry MinGW, which cannot compile `webrtc-audio-processing` (abseil).

---

## Shared home (desktop and Android)

Android 16 (`minSdk` 36). The debug APK talks to the same home as desktop.

```text
# same secret on nemo-server and coturn (ADR-0035)
export NEMO_TURN_SECRET=nemo-dev-turn
# desktop-only freeze: loopback is enough
export NEMO_TURN_URL=turn:127.0.0.1:3478

docker compose -f deploy/compose.yml --profile calls up --build
curl -k https://localhost:8443/v1/bundle
```

Caddy uses a local CA (`tls internal`). The Nemo client treats that as transport, not identity.

A phone cannot use `localhost` or `10.0.2.2`. Use the computer’s LAN address instead:

```text
# example: this machine is 192.168.1.20
export NEMO_TURN_URL=turn:192.168.1.20:3478
docker compose -f deploy/compose.yml --profile calls up --build
```

Open host firewall TCP **8443**, UDP/TCP **3478**, and UDP **49152–49200**.

---

## Desktop freeze (you still need to do this)

One window, two panes (`Left` and `Right`). Vaults: `~/.local/share/nemo/left` and `…/right`.

```text
cargo build -p nemo-ffi
cd apps/compose
./gradlew run
```

On **each** pane:

1. **Create identity** — passphrase at least 8 characters. Copy says the identity cannot be recovered or exported.
2. Write down the revocation phrase, then **I wrote it down**.
3. Settings → **Home URL** `https://localhost:8443` → **Connect**.
4. Settings → **Copy my contact card** (or show the QR).

Add the other pane: **+** → **New chat** → paste the `nemo:1:…` URI (or **Scan QR** and pick a screenshot of the QR). Confirm the fingerprint, set a name, **Add**.

Then:

| Check | How |
| --- | --- |
| 1:1 text | Send from Left, see it on Right (and back). Quit and unlock both; send again. |
| File | Paperclip on a 1:1 chat. |
| Group | **+** → **New group**. Settings → **Copy invite**. Other pane: **+** → **Join group**, paste invite. That copies a `nemo-j:1:…` join request. First pane Settings → paste join request → **Admit to group**. Both send in the group. |
| Live call | Open the 1:1 chat, grant the OS microphone if asked, press **Call**, answer on the other pane. Speak; you must hear the other side through coturn. End the call. |

Pass only if the call is audible with a real mic, not silence frames in a unit test.

---

## Android (emulator)

Emulator default home URL is `https://10.0.2.2:8443` (the host). Keep `NEMO_TURN_URL=turn:10.0.2.2:3478` while the emulator is the caller or callee.

```text
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_SDK_ROOT=$ANDROID_HOME
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk

"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" \
  "platforms;android-36" "build-tools;36.0.0" "ndk;27.2.12479018" \
  "platform-tools" "emulator" "system-images;android-36;google_apis;x86_64"

"$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" create avd -n nemo \
  -k "system-images;android-36;google_apis;x86_64" -d pixel --force
"$ANDROID_HOME/emulator/emulator" -avd nemo

cd apps/compose
./gradlew installDebug
```

Create / unlock / **Connect** like desktop. Pair with the desktop **Right** pane (or a second emulator).

- Paste a contact card, or **Scan QR**: allow camera, photograph the desktop QR.
- Confirm fingerprint before **Add**.
- Allow **Microphone** when calling.

Wipe the vault: uninstall the app, or `adb uninstall org.nemo`.

---

## Android (physical device)

USB debugging, Android 16, debug APK.

```text
adb devices
cd apps/compose
./gradlew installDebug
```

On the phone, Settings → **Home URL** `https://<LAN-IP>:8443` → **Connect**. Do not leave `10.0.2.2` (that is emulator-only).

ICE UDP on Android binds `0.0.0.0`. TURN must be the same LAN address you set in `NEMO_TURN_URL` before compose up.

Walk the same checklist as desktop: register, 1:1 text, group invite→accept→admit, then a live call with the mic.

---

## Failures that are setup, not product bugs

| Symptom | Usual cause |
| --- | --- |
| Connect fails from a phone | Home URL still `localhost` / `10.0.2.2`, or port 8443 not reachable on LAN |
| Call rings, no audio | Coturn profile not up; `NEMO_TURN_URL` is still `127.0.0.1` on a phone |
| TURN 401 | `NEMO_TURN_SECRET` missing on `nemo-server` or not the same as coturn `--static-auth-secret` |
| App will not install | Device below Android 16 |
