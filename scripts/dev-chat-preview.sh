#!/usr/bin/env bash
# Deploy the persistent, loopback-only Chat preview on the developer VM.
set -euo pipefail
umask 077

checkout=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
root="$HOME/Developer/lanes/chat-preview"
unit=calternal-chat-preview.service
port=24317

if [[ ${1:-} == --build-release && $# == 2 ]]; then
    release=$2
    cd "$checkout"
    "$checkout/scripts/dev-chat-host.sh" --build
    image_key=$(printf '%s\n' "$checkout" | cksum)
    install -m 700 "${CARGO_TARGET_DIR:-$checkout/target}/debug/calternal-host" "$release/calternal-host"
    cp -R apps/web/dist "$release/web"
    mkdir "$release/sdk"
    cp -R packages/plugin-sdk/dist packages/plugin-sdk/package.json "$release/sdk/"
    cp -R plugins-v2/{calternal-chat,calternal-chat-claude-code,calternal-reading-list,calternal-tasks} "$release/"
    printf 'CALTERNAL_WORKER_BWRAP=%s\nCALTERNAL_WORKER_NODE=%s\nCALTERNAL_RUN_IMAGE=%s\n' \
        "$(command -v bwrap)" "$(command -v node)" "localhost/calternal-dev-chat-fake:${image_key%% *}" >"$release/runtime.env"
    exit
fi
if (( $# )); then
    printf 'Usage: %s\n' "$0" >&2
    exit 2
fi

mkdir -p "$root/build" "$root/data" "$HOME/.config/systemd/user"
exec 9>"$root/deploy.lock"
flock 9
if ! systemctl --user is-active --quiet "$unit" && [[ -n $(ss -Hln "sport = :$port") ]]; then
    printf 'Preview port %s is already in use.\n' "$port" >&2
    exit 1
fi

release=$(mktemp -d "$root/build/release.XXXXXXXX")
discard_unpublished() {
    if [[ $(readlink "$root/current" || true) != "$release" ]]; then
        find "$release" -depth -delete
    fi
}
trap discard_unpublished EXIT
export CARGO_TARGET_DIR="$root/build/target"
cd "$checkout"
"$HOME/bin/lock-shared" nix develop --profile "$release/nix-shell" -c "$checkout/scripts/dev-chat-preview.sh" --build-release "$release"

unit_path="$HOME/.config/systemd/user/$unit"
unit_tmp=$(mktemp "$unit_path.XXXXXXXX")
cat >"$unit_tmp" <<'UNIT'
[Unit]
Description=Calternal Chat development preview
After=network.target

[Service]
WorkingDirectory=%h/Developer/lanes/chat-preview
Environment=PATH=/nix/var/nix/profiles/default/bin:/usr/local/bin:/usr/bin:/bin
Environment=CALTERNAL_WORKER_SDK=%h/Developer/lanes/chat-preview/current/sdk
EnvironmentFile=%h/Developer/lanes/chat-preview/current/runtime.env
ExecStart=%h/Developer/lanes/chat-preview/current/calternal-host preview-serve %h/Developer/lanes/chat-preview/data %h/Developer/lanes/chat-preview/current/web %h/Developer/lanes/chat-preview/current/calternal-chat %h/Developer/lanes/chat-preview/current/calternal-chat-claude-code %h/Developer/lanes/chat-preview/current/calternal-reading-list %h/Developer/lanes/chat-preview/current/calternal-tasks 24317
Restart=on-failure
RestartSec=3
UMask=0077

[Install]
WantedBy=default.target
UNIT
mv -f "$unit_tmp" "$unit_path"
systemctl --user daemon-reload

previous=$(readlink "$root/current" || true)
publish() {
    ln -s "$1" "$root/.current.$$"
    mv -Tf "$root/.current.$$" "$root/current"
}
# The worker lazily opens its SDK from current, so stop before moving the link.
systemctl --user stop "$unit"
publish "$release"
if systemctl --user start "$unit"; then
    for _ in {1..40}; do
        if systemctl --user is-active --quiet "$unit" &&
            curl -fsS --max-time 1 "http://localhost:$port/v1/auth/status" >/dev/null 2>&1; then
            systemctl --user enable "$unit"
            for old in "$root"/build/release.*; do
                if [[ -d $old && ! -L $old && $old != "$release" && $old != "$previous" ]]; then
                    find "$old" -depth -delete
                fi
            done
            printf 'ssh -L %s:127.0.0.1:%s claude@calternal-dev-01-compute02\n' "$port" "$port"
            printf 'Address http://localhost:%s/host\n' "$port"
            printf 'First-owner code file %s/data/first-owner-code\n' "$root"
            exit
        fi
        sleep 1
    done
fi
systemctl --user stop "$unit"
if [[ -n $previous ]]; then
    publish "$previous"
    systemctl --user start "$unit"
else
    unlink "$root/current"
fi
printf 'Preview did not start. Inspect journalctl --user -u %s.\n' "$unit" >&2
exit 1
