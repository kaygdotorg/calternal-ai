#!/bin/sh
# Build the fake provider and serve a disposable Chat host on loopback.
set -eu

checkout=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$checkout"
image_key=$(printf '%s\n' "$checkout" | cksum)
image="localhost/calternal-dev-chat-fake:${image_key%% *}"
export CALTERNAL_RUN_IMAGE="$image"

case "${1:-}" in
    --build)
        npm ci
        npm run build:plugins
        plugins-v2/calternal-chat-claude-code/test/build-echo-image.sh "$image"
        CAL_BASE=/ CAL_OUT=dist npm run web:build
        cargo build --locked -p calternal-host
        ;;
    --serve)
        umask 077
        data=$(mktemp -d "$checkout/.dev-chat-host.XXXXXXXX")
        export CALTERNAL_WORKER_BWRAP="$(command -v bwrap)"
        export CALTERNAL_WORKER_NODE="$(command -v node)"
        export CALTERNAL_WORKER_SDK="$checkout/packages/plugin-sdk"
        exec "${CARGO_TARGET_DIR:-$checkout/target}/debug/calternal-host" dev-serve \
            "$data" "$checkout/apps/web/dist" \
            "$checkout/plugins-v2/calternal-chat" "$checkout/plugins-v2/calternal-chat-claude-code"
        ;;
    "")
        "$HOME/bin/lock-shared" nix develop -c "$checkout/scripts/dev-chat-host.sh" --build
        exec nix develop -c "$checkout/scripts/dev-chat-host.sh" --serve
        ;;
    *)
        printf 'Usage: %s\n' "$0" >&2
        exit 2
        ;;
esac
