#!/usr/bin/env bash
# Prove the two exact local images and publish evidence for the selected mode.
set -euo pipefail
umask 077
test_name=release_artifacts_preserve_live_data_in_both_directions
format=calternal-upgrade-proof/v1
if [[ ${1:-} == --incompatible ]]; then
  shift
  test_name=release_artifacts_migrate_security_receipts_from_v3_to_v4
  format=calternal-incompatible-upgrade-proof/v1
fi
if [[ $# != 3 ]]; then
  printf '%s\n' 'Usage: verify-additive-upgrade.sh [--incompatible] PREVIOUS_IMAGE CANDIDATE_IMAGE PROOF_OUTPUT' >&2
  exit 2
fi
previous_image=$1 candidate_image=$2 proof_output=$3
[[ $proof_output == /* ]] || proof_output=$PWD/$proof_output
repo=$(cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo"
temporary=$(mktemp -d)
container=
cleanup() {
  if [[ -n $container ]]; then podman rm -f --ignore "$container" >/dev/null || true; fi
  # This is the owned temporary directory returned by mktemp above.
  find "$temporary" -depth -delete
}
trap cleanup EXIT
extract() {
  local image=$1 path=$2
  mkdir -m 0700 -- "$path"
  container=$(podman create --network=none "$image")
  podman cp "$container:/usr/local/bin/calternal" "$path/calternal"
  podman cp "$container:/app/plugins" "$path/plugins"
  podman rm "$container" >/dev/null
  container=
  chmod 0700 "$path/calternal"
}
previous=$(podman image inspect --format '{{.Id}}' "$previous_image")
candidate=$(podman image inspect --format '{{.Id}}' "$candidate_image")
previous=${previous#sha256:}
candidate=${candidate#sha256:}
[[ $previous =~ ^[a-f0-9]{64}$ && $candidate =~ ^[a-f0-9]{64}$ && $previous != "$candidate" ]]
extract "$previous" "$temporary/previous"
extract "$candidate" "$temporary/candidate"

export CALTERNAL_RELEASE_N_BIN=$temporary/previous/calternal CALTERNAL_RELEASE_NEXT_BIN=$temporary/candidate/calternal
export CALTERNAL_RELEASE_N_IMAGE=$previous CALTERNAL_RELEASE_NEXT_IMAGE=$candidate
npm run build:plugins
scripts/check-podman-tests.sh cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --features release-upgrade \
  --test isolation "$test_name" -- --test-threads=1

# Publish evidence only after both actual release executables have served the data.
stage=$(mktemp -- "${proof_output}.XXXXXXXX")
printf '{"format":"%s","previous":"%s","candidate":"%s"}\n' \
  "$format" "$previous" "$candidate" > "$stage"
mv -T -- "$stage" "$proof_output"
