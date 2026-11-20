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
# Firecracker's Unix socket paths must fit even when Nix nests TMPDIR.
proof=$(mktemp -d "${CALTERNAL_MICROVM_PROOF_TMPDIR:-/tmp}/cm.XXXXXXXX")
trap 'rm -rf -- "$scratch" "$proof"' EXIT
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
nix build --impure --out-link "$scratch/candidate" --expr '
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
# Qualify the exact image while its output is still private. The proof
# opens KVM and exercises two restores through the shipped guest agent.
export CALTERNAL_PROOF_FIRECRACKER=$(command -v firecracker)
export CALTERNAL_PROOF_SERIAL="$proof/serial.log"
cat > "$proof/firecracker" <<'WRAPPER'
#!/bin/sh
if [ "${1:-}" != --version ]; then
  exec >>"$CALTERNAL_PROOF_SERIAL" 2>&1
fi
exec "$CALTERNAL_PROOF_FIRECRACKER" "$@"
WRAPPER
chmod 0700 "$proof/firecracker"
python3 - "$proof/small.json" "$scratch/candidate" "$proof/firecracker" "$(command -v mkfs.ext4)" <<'PYCONFIG'
import hashlib
import json
from pathlib import Path
import sys

config, image, firecracker, mkfs = sys.argv[1:]
manifest = Path(image) / "image.sha256"
Path(config).write_text(json.dumps(dict(
    firecracker=firecracker,
    image_directory=image,
    image_digest=hashlib.sha256(manifest.read_bytes()).hexdigest(),
    mkfs_ext4=mkfs,
    vcpus=2, memory_mib=256, disk_mib=1024, max_guests=1, warm_guests=0,
)))
PYCONFIG
export CALTERNAL_SMALL_VM_TEST_CONFIG="$proof/small.json" CALTERNAL_SMALL_VM_PROOF_REQUIRED=1
if ! (
  cd "$CALTERNAL_GUEST_REPO"
  if [[ -n ${CALTERNAL_MICROVM_PROOF_MAX_LOAD:-} ]]; then
    until awk -v limit="$CALTERNAL_MICROVM_PROOF_MAX_LOAD" 'NR == 1 { exit !($1 <= limit) }' /proc/loadavg; do
      sleep 5
    done
  fi
  nix develop -c cargo test --locked -j"${CARGO_BUILD_JOBS:-3}" -p calternal-machine --lib \
    image_standby_refreshes_randomness_and_preserves_denials -- --nocapture --test-threads=3
) | tee "$proof/test.log"; then
  if [[ -f "$CALTERNAL_PROOF_SERIAL" ]]; then tail -100 "$CALTERNAL_PROOF_SERIAL" >&2; fi
  exit 1
fi
grep -Eq '^test .*::image_standby_refreshes_randomness_and_preserves_denials \.\.\. ok$' "$proof/test.log"
output=$(realpath -- "$(dirname -- "$4")")/$(basename -- "$4")
nix-store --add-root "$output" --indirect --realise "$(readlink -f "$scratch/candidate")"
sha256sum "$4/image.sha256"
