#!/usr/bin/env bash
# Build and install the debug APK. Starts the 'nemo' AVD if no device is connected.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck disable=SC1091
source "${ROOT}/scripts/dev-env.sh"

ADB="${ANDROID_HOME}/platform-tools/adb"
AVD="${NEMO_AVD:-nemo}"
WAIT_SECS="${NEMO_ADB_WAIT_SECS:-120}"

if [[ ! -x "${ADB}" ]]; then
  echo "adb not found at ${ADB}" >&2
  echo "Install Android platform-tools (see deploy/README.md)." >&2
  exit 1
fi

if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "${ANDROID_NDK_HOME}" ]]; then
  echo "ANDROID_NDK_HOME is unset or missing (need ndk 27.2.12479018)." >&2
  echo "See deploy/README.md Android section." >&2
  exit 1
fi

device_ready() {
  "${ADB}" devices 2>/dev/null | awk 'NR>1 && $2=="device" { found=1 } END { exit !found }'
}

if ! device_ready; then
  echo "No device/emulator in 'device' state."
  if "${ANDROID_HOME}/emulator/emulator" -list-avds 2>/dev/null | grep -qx "${AVD}"; then
    echo "Starting AVD '${AVD}' in the background…"
    "${ANDROID_HOME}/emulator/emulator" -avd "${AVD}" >/tmp/nemo-emulator.log 2>&1 &
    echo "Waiting up to ${WAIT_SECS}s for adb device (log: /tmp/nemo-emulator.log)…"
    "${ADB}" wait-for-device
    deadline=$((SECONDS + WAIT_SECS))
    until device_ready; do
      if (( SECONDS >= deadline )); then
        echo "Timed out waiting for emulator boot." >&2
        echo "Last emulator log lines:" >&2
        tail -n 40 /tmp/nemo-emulator.log >&2 || true
        exit 1
      fi
      sleep 2
    done
    # sys.boot_completed is more reliable than 'device' alone
    boot_deadline=$((SECONDS + WAIT_SECS))
    until [[ "$("${ADB}" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]]; do
      if (( SECONDS >= boot_deadline )); then
        echo "Timed out waiting for sys.boot_completed." >&2
        exit 1
      fi
      sleep 2
    done
  else
    echo "No AVD named '${AVD}'. Either:" >&2
    echo "  - plug in a phone with USB debugging, or" >&2
    echo "  - create the AVD (deploy/README.md), then: scripts/android-emulator.sh" >&2
    exit 1
  fi
fi

"${ADB}" devices -l
cd "${ROOT}/apps/compose"
exec ./gradlew installDebug "$@"
