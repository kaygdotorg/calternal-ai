#!/usr/bin/env bash
# Enroll and qualify the signed-in browser owner's disposable machine.
set -euo pipefail
umask 077
[[ $(hostname -s) == calternal-dev-01-compute02 && $(id -un) == claude ]] ||
  { echo 'Run on claude@calternal-dev-01-compute02.' >&2; exit 1; }
exec 9>"$HOME/calternal-e2e/browser/after-owner-setup.lock"
flock -n 9 || { echo 'Browser machine setup is already running.' >&2; exit 1; }
exec python3 "$(dirname -- "$0")/prove.py" after-owner-setup
