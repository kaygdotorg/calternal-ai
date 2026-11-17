#!/bin/sh
# Use an installed compiler; never install packages or change global toolchains.
set -eu
if ! command -v cc >/dev/null 2>&1; then
  for candidate in /nix/store/*-gcc-wrapper-*/bin; do
    if [ -x "$candidate/cc" ]; then
      PATH="$candidate:$PATH"
      export PATH
      break
    fi
  done
fi
if ! command -v cc >/dev/null 2>&1; then
  echo "A C compiler is required. Install your platform's build tools or enter a Nix development shell." >&2
  exit 1
fi
# OpenSSL headers (needed by webauthn-rs through openssl-sys): prefer the system's
# development package; otherwise use the newest copy already in the Nix store, as the
# existing app's flake does. Nothing is downloaded or installed. The library directory
# is also put on LD_LIBRARY_PATH so test binaries started through this wrapper load the
# same build they were linked against.
if [ -z "${OPENSSL_DIR:-}" ] && [ -z "${OPENSSL_LIB_DIR:-}" ] && [ ! -e /usr/include/openssl/ssl.h ]; then
  for dev in $(ls -d /nix/store/*-openssl-3.*-dev 2>/dev/null | sort -t- -k3 -V -r); do
    lib=$(tr ' ' '\n' < "$dev/nix-support/propagated-build-inputs" 2>/dev/null | grep -v -e '-bin$' -e '^$' | head -n 1)
    if [ -f "$dev/include/openssl/ssl.h" ] && [ -n "$lib" ] && [ -d "$lib/lib" ]; then
      OPENSSL_INCLUDE_DIR="$dev/include"
      OPENSSL_LIB_DIR="$lib/lib"
      LD_LIBRARY_PATH="$lib/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
      export OPENSSL_INCLUDE_DIR OPENSSL_LIB_DIR LD_LIBRARY_PATH
      break
    fi
  done
fi
: "${CARGO_BUILD_JOBS:=2}"
export CARGO_BUILD_JOBS
exec "$@"
