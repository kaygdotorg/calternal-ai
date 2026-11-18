#!/usr/bin/env bash
# Enroll the browser owner's disposable machine, with an optional fixture proof.
set -euo pipefail
umask 077
mode=claude
case "$#:$*" in
  0:) ;;
  1:--verify-fixture) mode=fixture ;;
  *) echo 'Usage: after-owner-setup.sh [--verify-fixture]' >&2; exit 2 ;;
esac
[[ $(hostname -s) == calternal-dev-01-compute02 && $(id -un) == claude ]] ||
  { echo 'Run on claude@calternal-dev-01-compute02.' >&2; exit 1; }
exec 9>"$HOME/calternal-e2e/browser/after-owner-setup.lock"
flock -n 9 || { echo 'Browser machine setup is already running.' >&2; exit 1; }
service=$(dirname -- "$0")/service.sh
home=$("$service" browser-machine "$mode")
account_dir=${home%/home}
if [[ $mode == fixture && ! -f $account_dir/fixture-verified ]]; then
  started=$(mktemp)
  trap 'unlink -- "$started"' EXIT
  printf '%s\n' 'In the preview, open a new Thread and send: Verify the fixture machine'
  printf '%s' 'Press Enter here after the fixture replies "Hello 🌏" and finishes. '
  IFS= read -r _
  marker=
  if [[ -d $account_dir/threads ]]; then
    marker=$(find "$account_dir/threads" -mindepth 3 -maxdepth 3 \
                  -path '*/work/.fake-session' -type f -newer "$started" -print -quit)
  fi
  if [[ -z $marker ]]; then
    echo 'No new fixture run was found on this machine. Rerun this script and send from a new Thread.' >&2
    exit 1
  fi
  workspace=${marker%/work/.fake-session}
  # The browser can see the terminal event before the machine removes the container.
  for ((attempt=0; attempt<60; attempt++)); do
    containers=$(podman ps -aq --filter "label=org.calternal.sandbox-root=${account_dir%/accounts/*}" \
                        --filter "label=org.calternal.thread=${workspace##*/}")
    [[ -n $containers ]] || break
    sleep .5
  done
  if [[ -n $containers ]]; then
    echo 'The fixture sandbox is still stopping. Rerun the script once it has stopped.' >&2
    exit 1
  fi
  touch "$account_dir/fixture-verified"
  printf '%s\n' 'Fixture ran in the enrolled account sandbox.'
  "$service" browser-machine claude >/dev/null
fi
printf 'Sandbox home  %s\n' "$home"
printf "ssh -t claude@calternal-dev-01-compute02 'podman run --rm -it -e HOME=/home/agent -v %s:/home/agent localhost/calternal-e2e-claude:dev claude auth login'\n" "$home"
