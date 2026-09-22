#!/usr/bin/env bash
# Start the local Android emulator AVD (default name: nemo).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck disable=SC1091
source "${ROOT}/scripts/dev-env.sh"

AVD="${1:-nemo}"
EMULATOR="${ANDROID_HOME}/emulator/emulator"

if [[ ! -x "${EMULATOR}" ]]; then
  echo "emulator not found at ${EMULATOR}" >&2
  echo "Install the Android SDK emulator package (see deploy/README.md)." >&2
  exit 1
fi

if ! "${EMULATOR}" -list-avds | grep -qx "${AVD}"; then
  echo "AVD '${AVD}' not found. Create it with:" >&2
  echo "  \"\${ANDROID_HOME}/cmdline-tools/latest/bin/avdmanager\" create avd -n ${AVD} \\" >&2
  echo "    -k \"system-images;android-36;google_apis;x86_64\" -d pixel --force" >&2
  exit 1
fi

echo "Starting AVD '${AVD}' (Ctrl-C does not stop a backgrounded emulator)."
exec "${EMULATOR}" -avd "${AVD}"
