#!/usr/bin/env bash
# Run the desktop Compose shell (builds host nemo-ffi first via Gradle).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck disable=SC1091
source "${ROOT}/scripts/dev-env.sh"

cd "${ROOT}/apps/compose"
exec ./gradlew run "$@"
