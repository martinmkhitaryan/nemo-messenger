#!/usr/bin/env bash
# Source from any shell:  source scripts/dev-env.sh
# Sets clang/bindgen paths for webrtc-audio-processing and Android SDK env.
# Works when sourced from bash or zsh; other scripts invoke it under bash.

_nemo_sourced=0
if [[ -n "${ZSH_VERSION:-}" ]]; then
  case "${ZSH_EVAL_CONTEXT:-}" in *:file*) _nemo_sourced=1 ;; esac
elif [[ -n "${BASH_VERSION:-}" ]]; then
  [[ "${BASH_SOURCE[0]}" != "$0" ]] && _nemo_sourced=1
fi
if [[ "${_nemo_sourced}" -eq 0 ]]; then
  echo "source this file:  source scripts/dev-env.sh" >&2
  return 1 2>/dev/null || exit 1
fi
unset _nemo_sourced

_nemo_this=
if [[ -n "${BASH_SOURCE[0]:-}" ]]; then
  _nemo_this="${BASH_SOURCE[0]}"
elif [[ -n "${ZSH_VERSION:-}" ]]; then
  # eval so bash never parses zsh's %x expansion
  eval '_nemo_this="${(%):-%x}"'
fi
if [[ -z "${_nemo_this}" ]]; then
  echo "dev-env: could not resolve scripts/ path; set ANDROID_HOME yourself" >&2
  _nemo_root=""
else
  _nemo_scripts_dir="$(cd "$(dirname "${_nemo_this}")" && pwd)"
  _nemo_root="$(cd "${_nemo_scripts_dir}/.." && pwd)"
  unset _nemo_scripts_dir
fi
unset _nemo_this

# --- host native deps (webrtc-audio-processing bindgen) ---
if [[ -z "${LIBCLANG_PATH:-}" ]]; then
  _libclang="$(find /usr/lib /usr/lib64 -name 'libclang.so*' 2>/dev/null | head -n1 || true)"
  if [[ -n "${_libclang}" ]]; then
    export LIBCLANG_PATH="$(dirname "${_libclang}")"
  fi
  unset _libclang
fi

if [[ -z "${BINDGEN_EXTRA_CLANG_ARGS:-}" ]] && command -v gcc >/dev/null 2>&1; then
  export BINDGEN_EXTRA_CLANG_ARGS="-I/usr/lib/gcc/$(gcc -dumpmachine)/$(gcc -dumpversion)/include"
fi

# --- Android SDK / NDK ---
: "${ANDROID_HOME:=${HOME}/Android/Sdk}"
export ANDROID_HOME
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"

_ndk_preferred="${ANDROID_HOME}/ndk/27.2.12479018"
if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  if [[ -d "${_ndk_preferred}" ]]; then
    export ANDROID_NDK_HOME="${_ndk_preferred}"
  else
    _ndk_any="$(ls -d "${ANDROID_HOME}"/ndk/* 2>/dev/null | tail -n1 || true)"
    if [[ -n "${_ndk_any}" ]]; then
      export ANDROID_NDK_HOME="${_ndk_any}"
    fi
    unset _ndk_any
  fi
fi
unset _ndk_preferred
if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
  export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-$ANDROID_NDK_HOME}"
fi

case ":${PATH}:" in
  *":${ANDROID_HOME}/platform-tools:"*) ;;
  *) export PATH="${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/emulator:${PATH}" ;;
esac

echo "nemo dev-env (${_nemo_root:-?})"
echo "  LIBCLANG_PATH=${LIBCLANG_PATH:-<unset>}"
echo "  BINDGEN_EXTRA_CLANG_ARGS=${BINDGEN_EXTRA_CLANG_ARGS:-<unset>}"
echo "  ANDROID_HOME=${ANDROID_HOME}"
echo "  ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-<unset>}"
unset _nemo_root
