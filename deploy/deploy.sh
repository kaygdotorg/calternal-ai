#!/usr/bin/env bash
# Build on a dev machine, transfer the image, then switch the target's user services.
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(dirname -- "$here")
die() { printf '%s\n' "$*" >&2; exit 2; }
usage() { printf '%s\n' 'Usage: deploy/deploy.sh --target FILE [--tag TAG | --image TAG] [--initialize | --compatibility-proof FILE | --allow-incompatible] [--dry-run] [--show-setup-token]'; }
dry_run=0 initialize=0 show_token=0 config= build=1 tag= incompatible=0 proof_file=
while (($#)); do
  case $1 in
    --target) config=${2:?--target needs a file}; shift ;;
    --tag) tag=${2:?--tag needs a tag}; shift ;;
    --image) tag=${2:?--image needs an existing tag}; build=0; shift ;;
    --initialize) initialize=1 ;;
    --allow-incompatible) incompatible=1 ;;
    --compatibility-proof) proof_file=${2:?--compatibility-proof needs a file}; shift ;;
    --dry-run) dry_run=1 ;;
    --show-setup-token) show_token=1 ;;
    --help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
  shift
done
[[ -f $config ]] || die 'Supply an operator-owned target file with --target.'
[[ $config == /* ]] || config=$PWD/$config
compatibility_proof=
if [[ -n $proof_file ]]; then
  ((incompatible == 0 && initialize == 0)) || die 'Use either verified compatibility or an explicit incompatible upgrade.'
  [[ -f $proof_file && $(wc -c < "$proof_file") -le 4096 ]] || die 'Supply the release gate evidence file.'
  compatibility_proof=$(<"$proof_file")
fi
((incompatible == 0 || initialize == 0)) || die 'An incompatible upgrade requires an existing release.'
source "$here/target.sh"
load_target "$config"
cd -- "$repo"
if [[ -z $tag ]]; then
  tag=$(git rev-parse --short=12 HEAD)
  [[ -z $(git status --porcelain) ]] || tag=$tag-dirty
fi
[[ $tag =~ ^[a-zA-Z0-9_][a-zA-Z0-9_.-]{0,79}$ && $tag != current && $tag != rollback ]] ||
  die 'Use a release tag other than current or rollback.'
candidate=$image:$tag
if ((dry_run)); then
  if ((build)); then printf 'nix develop .#deploy -c %q %q\n' "$here/build-image.sh" "$candidate"; fi
  printf 'podman save %q | ' "$candidate"
  remote podman load --quiet
  remote bash -s -- deploy "${target_args[@]}" IMAGE_ID "$tag" "$initialize" "$show_token" "$incompatible"
  print_remote_script
  if ((show_token)); then printf '  The explicit token flag prints the setup token if setup is pending.\n'; fi
  exit 0
fi
if ((build)); then
  [[ $(hostname -s) != calternal-cloud ]] || die 'Build images on the dev VM.'
  RELEASE_REVISION=$(git rev-parse HEAD 2>/dev/null || printf '%s' "$tag")
  SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD 2>/dev/null || printf 0)
  export RELEASE_REVISION SOURCE_DATE_EPOCH
  nix develop .#deploy -c "$here/build-image.sh" "$candidate"
fi
image_id=$(podman image inspect --format '{{.Id}}' "$candidate")
image_id=${image_id#sha256:}
podman save "$candidate" | remote podman load --quiet
# Shell-quote the fixed templates into stdin before executing the remote driver.
# No target settings or secrets are expanded into template source.
{
  printf 'server_template=%q\nmachine_template=%q\nrecovery_template=%q\ncompatibility_proof=%q\n' \
    "$(<"$here/calternal-ai.container")" "$(<"$here/calternal-ai-machine.service")" \
    "$(<"$here/recover-start.sh")" "$compatibility_proof"
  cat "$here/remote.sh"
} | remote bash -s -- deploy "${target_args[@]}" "$image_id" "$tag" "$initialize" "$show_token" "$incompatible"
