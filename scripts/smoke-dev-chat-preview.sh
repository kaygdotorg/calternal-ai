#!/usr/bin/env bash
# Run Chromium against the deployed release using disposable data and a transient user service.
set -euo pipefail
umask 077

checkout=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$checkout"
release=$(readlink -f "${PREVIEW_SMOKE_RELEASE:-$HOME/Developer/lanes/chat-preview/current}")
port=${PREVIEW_SMOKE_PORT:-24318}
if [[ ! $port =~ ^[1-9][0-9]{3,4}$ || $port == 24317 || $port == 4313 ||
      -n $(ss -Hln "sport = :$port") ]]; then
    printf 'Choose an unused preview smoke port distinct from 24317 and 4313.\n' >&2
    exit 2
fi
scratch=$(mktemp -d "$checkout/.preview-smoke.XXXXXXXX")
unit="calternal-preview-smoke-${scratch##*.}.service"
cleanup() {
    systemctl --user stop "$unit" 2>/dev/null || true
    systemctl --user reset-failed "$unit" 2>/dev/null || true
    rm -rf -- "$scratch"
}
trap cleanup EXIT

# Only the known release settings enter the disposable unit. Refuse any image
# that lacks a release-specific fake tag or differs from the recorded ID.
awk -F= '
    /^CALTERNAL_WORKER_(BWRAP|NODE)=\/nix\/store\/[A-Za-z0-9_./+-]+$/ { print; workers++ }
    /^CALTERNAL_RUN_IMAGE=(sha256:)?[0-9a-f]+$/ { print; images++ }
    END { if (workers != 2 || images != 1) exit 1 }
' "$release/runtime.env" >"$scratch/runtime.env"
image=$(sed -n 's/^CALTERNAL_RUN_IMAGE=//p' "$scratch/runtime.env")
read -r tag <"$release/image.tag"
if [[ ! $image =~ ^(sha256:)?[0-9a-f]{64}$ ||
      ! $tag =~ ^localhost/calternal-dev-chat-fake:release\.[A-Za-z0-9]{8}$ ||
      $(podman image inspect --format '{{.Id}}' "$tag") != "$image" ]]; then
    printf 'Preview release does not have its recorded local fake image.\n' >&2
    exit 2
fi

export HOST_PREVIEW_RELEASE="$release"
export HOST_BIN="$release/calternal-host" HOST_PREVIEW_UNIT="$unit"
export HOST_SHOT_DIR="$scratch/shots"
export CALTERNAL_WORKER_SDK="$release/sdk"
CALTERNAL_WORKER_BWRAP=$(sed -n 's/^CALTERNAL_WORKER_BWRAP=//p' "$scratch/runtime.env")
CALTERNAL_WORKER_NODE=$(sed -n 's/^CALTERNAL_WORKER_NODE=//p' "$scratch/runtime.env")
export CALTERNAL_WORKER_BWRAP CALTERNAL_WORKER_NODE
export CALTERNAL_TEST_PROVIDER_IMAGE="$image"
unset CALTERNAL_REAL_RUN_IMAGE CALTERNAL_RUN_SIGN_IN_ACCOUNT CALTERNAL_RUN_SIGN_IN_SPACE
unset CALTERNAL_RUN_SIGN_IN_PROGRAM CALTERNAL_RUN_SIGN_IN_FILE DEBUG PWDEBUG
origin="http://localhost:$port"

if (( $# == 0 )); then
    set -- chat runs
fi
for name in "$@"; do
    case "$name" in chat|runs) ;; *) exit 2 ;; esac
    directory="$scratch/$name"
    mkdir "$directory"
    if ! node "apps/web/e2e/host-$name.mjs" --fixture "$directory" "$origin"; then
        journalctl --user -u "$unit" -n 30 --no-pager
        exit 1
    fi
    systemctl --user stop "$unit"
done
