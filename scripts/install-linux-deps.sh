#!/usr/bin/env bash
# Install Linux packages needed to build nemo-ffi (same set as CI).
set -euo pipefail

if [[ "$(id -u)" -eq 0 ]]; then
  SUDO=()
else
  SUDO=(sudo)
fi

"${SUDO[@]}" apt-get update
"${SUDO[@]}" apt-get install -y \
  build-essential \
  meson \
  ninja-build \
  cmake \
  pkg-config \
  clang \
  libclang-dev \
  protobuf-compiler

echo
echo "Installed. In each new shell:"
echo "  source scripts/dev-env.sh"
echo
echo "Also need: Rust stable, JDK 21, and (for Android) the SDK/NDK — see deploy/README.md."
