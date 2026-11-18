#!/usr/bin/env bash
# Assemble a guest from immutable inputs on the dev VM. No root or global profile.
set -euo pipefail
if [[ $# != 4 ]]; then
  printf 'Usage: %s STATIC_GUEST_AGENT STATIC_CALTERNAL STANDARD_DOCKER_ARCHIVE OUTPUT_LINK\n' "$0" >&2
  exit 2
fi
for binary in "$1" "$2"; do
  headers=$(readelf -l "$binary")
  if grep 'INTERP' >/dev/null <<< "$headers"; then
    printf 'Expected a static guest binary: %s\n' "$binary" >&2
    exit 2
  fi
done
export CALTERNAL_GUEST_AGENT CALTERNAL_GUEST_CLI CALTERNAL_GUEST_ARCHIVE CALTERNAL_GUEST_REPO
CALTERNAL_GUEST_AGENT=$(realpath "$1")
CALTERNAL_GUEST_CLI=$(realpath "$2")
CALTERNAL_GUEST_ARCHIVE=$(realpath "$3")
CALTERNAL_GUEST_REPO=$(cd -- "$(dirname -- "$0")/../.." && pwd)
nix build --impure --out-link "$4" --expr '
  let
    repository = builtins.getEnv "CALTERNAL_GUEST_REPO";
    lock = builtins.fromJSON (builtins.readFile (repository + "/flake.lock"));
    nixpkgs = builtins.fetchTree lock.nodes.nixpkgs.locked;
    input = variable: builtins.path {
      path = builtins.getEnv variable;
      name = variable;
    };
  in import (repository + "/deploy/microvm/image.nix") {
    pkgs = import nixpkgs { system = "x86_64-linux"; };
    agent = input "CALTERNAL_GUEST_AGENT";
    cli = input "CALTERNAL_GUEST_CLI";
    standardArchive = input "CALTERNAL_GUEST_ARCHIVE";
  }'
sha256sum "$4/image.sha256"
