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
# Unpack the verified local image with the OCI-aware tool. Nix's builder
# forbids restoring set-ID bits present in upstream distro layers.
scratch=$(mktemp -d "$(realpath -- "$(dirname -- "$4")")/.guest-oci.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
skopeo --insecure-policy copy "docker-archive:$(realpath "$3")" "oci:$scratch/oci:standard"
umoci unpack --rootless --image "$scratch/oci:standard" "$scratch/bundle"
tar --sort=name --mtime=@1735689600 --format=gnu --numeric-owner \
  --owner=0 --group=0 --mode='u-s,g-s' -C "$scratch/bundle/rootfs" -cf - . \
  | gzip -n -1 > "$scratch/rootfs.tar.gz"
export CALTERNAL_GUEST_AGENT CALTERNAL_GUEST_CLI CALTERNAL_GUEST_ROOT CALTERNAL_GUEST_REPO
CALTERNAL_GUEST_AGENT=$(realpath "$1")
CALTERNAL_GUEST_CLI=$(realpath "$2")
CALTERNAL_GUEST_ROOT="$scratch/rootfs.tar.gz"
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
    standardRoot = input "CALTERNAL_GUEST_ROOT";
  }'
sha256sum "$4/image.sha256"
