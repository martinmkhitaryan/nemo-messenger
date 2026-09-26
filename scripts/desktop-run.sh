#!/usr/bin/env bash
# Run the desktop Compose shell (builds host nemo-ffi first via Gradle).
# Usage: ./scripts/desktop-run.sh [debug|release] [-- gradle args...]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck disable=SC1091
source "${ROOT}/scripts/dev-env.sh"

BUILD="${1:-debug}"
case "${BUILD}" in
  debug|release) shift || true ;;
  -*) BUILD=debug ;;
  *)
    echo "usage: $0 [debug|release] [-- gradle args...]" >&2
    exit 1
    ;;
esac

TASK=run
if [[ "${BUILD}" == "release" ]]; then
  TASK=runRelease
fi

cd "${ROOT}/apps/compose"
exec ./gradlew "${TASK}" "$@"
