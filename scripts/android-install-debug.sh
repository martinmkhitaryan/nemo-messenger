#!/usr/bin/env bash
# Compat wrapper — prefer: ./scripts/android-install.sh [debug|release]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec "${ROOT}/scripts/android-install.sh" debug "$@"
