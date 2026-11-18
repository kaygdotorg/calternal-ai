#!/usr/bin/env bash
# Recover an interrupted cutover before systemd restarts the previous release.
set -euo pipefail
umask 077
root=${1:?server root} hdd=${2:?userdata HDD root}
installation=$(sed -n 's/^CALTERNAL_INSTALLATION_ID=//p' "$root/server.env")
[[ $installation =~ ^[a-fA-F0-9-]{36}$ ]] || exit 1

# The deploy transaction owns recovery while it is alive. Its normal activation
# can call this hook without reacquiring the same deployment lock.
exec 9>"$root/releases/lock"
flock -n 9 || exit 0
upgrade=("$root/releases/current/calternal" --json upgrade
  --appdata "$root/appdata" --userdata-ssd "$root/userdata-ssd" --userdata-hdd "$hdd"
  --installation-id "$installation" --machine-dir "$root/machine")
status=$("${upgrade[@]}" status)
if grep -Eq '"needs_recovery"[[:space:]]*:[[:space:]]*true' <<< "$status"; then
  "${upgrade[@]}" recover >/dev/null
fi
