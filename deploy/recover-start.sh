#!/usr/bin/env bash
# Recover an interrupted cutover before systemd can restart the public unit.
set -euo pipefail
umask 077
root=${1:?server root} hdd=${2:?userdata HDD root}
installation=$(sed -n 's/^CALTERNAL_INSTALLATION_ID=//p' "$root/server.env")
[[ $installation =~ ^[a-fA-F0-9-]{36}$ ]] || exit 1

# The deployment driver owns recovery during normal activation.
exec 9>"$root/releases/lock"
flock -n 9 || exit 0
release=$root/releases/current
pin=$root/releases/pending-recovery
if [[ -L $pin ]]; then
  target=$(readlink -- "$pin")
  [[ $target =~ ^release\.[a-zA-Z0-9]+$ ]] || exit 1
  release=$root/releases/$target
fi
upgrade=("$release/calternal" --json upgrade
  --appdata "$root/appdata" --userdata-ssd "$root/userdata-ssd" --userdata-hdd "$hdd"
  --installation-id "$installation" --machine-dir "$root/machine")
status=$("${upgrade[@]}" status)
if [[ -L $pin ]]; then
  id=$(<"$release/image.id")
  [[ $id =~ ^[a-f0-9]{64}$ ]] || exit 1
  if grep -Eq '"phase"[[:space:]]*:[[:space:]]*"committed"' <<< "$status" &&
     grep -Eq '"candidate"[[:space:]]*:[[:space:]]*"'"$id"'"' <<< "$status"; then
    # An interrupted promotion needs the deploy driver to finish tagging and
    # installing units. Starting an old current image here would downgrade it.
    [[ $(realpath -- "$root/releases/current") == "$release" ]] || {
      printf '%s\n' 'The committed image is pending activation. Retry the deploy.' >&2
      exit 1
    }
  else
    probe=$(<"$release/probe.name")
    [[ $probe =~ ^calternal-ai(-[a-z0-9-]+)?-upgrade-probe$ ]] || exit 1
    podman stop --ignore "$probe" >/dev/null
    podman rm --ignore "$probe" >/dev/null
  fi
fi
if grep -Eq '"needs_recovery"[[:space:]]*:[[:space:]]*true' <<< "$status"; then
  "${upgrade[@]}" recover >/dev/null
fi
# The deployment driver retires the release pin and its staging directory.
