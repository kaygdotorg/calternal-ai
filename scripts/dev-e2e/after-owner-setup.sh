#!/usr/bin/env bash
# Enroll and qualify the signed-in browser owner's disposable machine.
set -euo pipefail
umask 077
[[ $(hostname -s) == calternal-dev-01-compute02 && $(id -un) == claude ]] ||
  { echo 'Run on claude@calternal-dev-01-compute02.' >&2; exit 1; }
exec 9>"$HOME/calternal-e2e/browser/after-owner-setup.lock"
flock -n 9 || { echo 'Browser machine setup is already running.' >&2; exit 1; }
service=$(dirname -- "$0")/service.sh
home=$("$service" browser-machine fixture)
account_dir=${home%/home}
if [[ ! -f $account_dir/fixture-verified ]]; then
  started=$(mktemp)
  trap 'unlink -- "$started"' EXIT
  printf '%s\n' 'In the preview, open a new Thread and send: Verify the fixture machine'
  printf '%s' 'Press Enter here after the fixture replies "Hello 🌏" and finishes. '
  IFS= read -r _
  if [[ ! -d $account_dir/threads ]] ||
     [[ -z $(find "$account_dir/threads" -mindepth 3 -maxdepth 3 \
                -path '*/work/.fake-session' -type f -newer "$started" -print -quit) ]]; then
    echo 'No new fixture run was found on this machine. Rerun this script and send from a new Thread.' >&2
    exit 1
  fi
  touch "$account_dir/fixture-verified"
  printf '%s\n' 'Fixture ran in the enrolled account sandbox.'
fi
"$service" browser-machine claude >/dev/null
printf 'Sandbox home  %s\n' "$home"
printf "ssh -t claude@calternal-dev-01-compute02 'podman run --rm -it -e HOME=/home/agent -v %s:/home/agent localhost/calternal-e2e-claude:dev claude auth login'\n" "$home"
