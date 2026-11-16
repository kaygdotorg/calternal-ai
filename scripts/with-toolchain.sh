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
: "${CARGO_BUILD_JOBS:=2}"
export CARGO_BUILD_JOBS
exec "$@"
