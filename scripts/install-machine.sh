#!/bin/sh
# Rendered at /install with the release key pinned into the server build.
{
set +x
set -eu
umask 077
LC_ALL=C
export LC_ALL

die() { printf '%s\n' "$*" >&2; exit 1; }
server= image=docker.io/library/alpine:3 backend=auto max=4294967295 code=${CALTERNAL_ENROLL_CODE:-}
unset CALTERNAL_ENROLL_CODE
release_key=
yes=false uninstall=false
while [ "$#" -gt 0 ]; do
  case $1 in
    --yes) yes=true; shift; continue ;;
    --uninstall) uninstall=true; shift; continue ;;
  esac
  [ "$#" -ge 2 ] || die "This option needs a value: $1"
  case $1 in
    --server) server=${2%/} ;;
    --code) [ "$2" = - ] || die 'Use the code prompt, or pass --code - to read stdin.'; code=- ;;
    --image) image=$2 ;;
    --backend) backend=$2 ;;
    --max-sandboxes) max=$2 ;;
    *) die "Unknown installer option: $1" ;;
  esac
  shift 2
done

machine_dir=${CALTERNAL_MACHINE_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/calternal/machine}
target=$machine_dir/bin/calternal
system=$(uname -s)
case $system in
  Linux) os=linux; service=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/calternal-machine.service ;;
  Darwin) os=macos; service=$HOME/Library/LaunchAgents/org.calternal.machine.plist ;;
  *) die "The machine service does not support $system." ;;
esac
case "$target$service$image" in
  *'
'*) die 'Use installation paths without line breaks.' ;;
esac

confirm() {
  if "$yes"; then return; fi
  printf 'Continue? [y/N] ' >&2
  if ! { IFS= read -r answer </dev/tty; } 2>/dev/null; then
    die 'Run in a terminal to confirm, or pass --yes after reviewing the plan.'
  fi
  case $answer in y|Y|yes|YES) ;; *) die 'Cancelled.' ;; esac
}

if "$uninstall"; then
  printf 'Uninstall the user service %s and remove %s.\n' "$service" "$target"
  printf 'Keep enrollment and workspace files in %s.\n' "$machine_dir"
  confirm
  case $system in
    Linux)
      if [ -f "$service" ]; then
        systemctl --user disable --now calternal-machine.service
        unlink "$service"
        systemctl --user daemon-reload
      fi
      ;;
    Darwin)
      label=gui/$(id -u)/org.calternal.machine
      if launchctl print "$label" >/dev/null 2>&1; then launchctl bootout "$label"; fi
      if [ -f "$service" ]; then unlink "$service"; fi
      ;;
  esac
  for installed in "$target" "$machine_dir/bin/.calternal-tmp-machine-update"; do
    if [ -f "$installed" ]; then unlink "$installed"; fi
  done
  printf 'Machine service removed. Revoke the machine in Calternal if it is retired.\n'
  exit 0
fi

case $server in
  https://?*) ;;
  *) die 'Supply an HTTPS server URL with --server.' ;;
esac
case $server in *[!a-zA-Z0-9.:/\[\]-]*) die 'Use a plain HTTPS server origin.' ;; esac
authority=${server#https://}
case $authority in */*) die 'Use a plain HTTPS server origin.' ;; esac
case $(uname -m) in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *) die 'This processor architecture is unsupported.' ;;
esac
platform=$os-$arch
case $max in ''|*[!0-9]*) die 'Use a positive sandbox limit.' ;; esac
[ "${#max}" -le 10 ] && [ "$max" -gt 0 ] && [ "$max" -le 4294967295 ] || die 'Use a positive sandbox limit.'
case $backend in auto|podman|direct) ;; *) die 'Choose auto, podman or direct.' ;; esac
printf 'Download the machine program for %s from %s.\n' "$platform" "$server"
printf 'Verify its signed release using the pinned Calternal key before running it.\n'
printf 'Install %s and enroll this machine using the app code.\n' "$target"
printf 'Start your user service at %s. The service updates between turns.\n' "$service"
if [ "$backend" = direct ]; then printf 'Direct mode runs turns as your user without sandbox isolation.\n'; fi
confirm

[ -n "$release_key" ] || die 'This installer has no pinned release key.'
command -v ssh-keygen >/dev/null || die 'Install OpenSSH to verify the release signature.'
case $backend in
  auto|podman)
    engine=$(command -v podman) || die 'Install Podman and start it for this user before enrolling.'
    "$engine" info >/dev/null || die 'Start Podman for this user before enrolling.'
    ;;
  direct) engine=podman ;;
esac
case "$engine" in
  *'
'*) die 'Use a Podman path without line breaks.' ;;
esac
if [ -f "$machine_dir/credentials" ]; then
  enrolled_server=$(sed -n 's/^server: //p' "$machine_dir/credentials")
  [ "$enrolled_server" = "$server" ] || die 'This machine is already enrolled with a different server.'
fi

download() {
  (
    # Cap writes even when a server omits Content-Length. POSIX sh uses 512-byte blocks.
    ulimit -f $((($1 + 511) / 512))
    curl --disable --proto '=https' --tlsv1.2 --fail --silent --show-error \
      --connect-timeout 10 --max-time 120 --max-filesize "$1" --output "$2" "$3"
  )
}
work=$(mktemp -d "${TMPDIR:-/tmp}/calternal-machine.XXXXXXXX")
binary_stage=
terminal_mode=
cleanup() {
  if [ -n "$terminal_mode" ]; then stty "$terminal_mode" </dev/tty; fi
  for file in version manifest manifest.sig allowed_signers calternal; do
    if [ -f "$work/$file" ]; then unlink "$work/$file"; fi
  done
  rmdir "$work"
  if [ -n "$binary_stage" ] && [ -f "$binary_stage" ]; then unlink "$binary_stage"; fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
root=$server/api/v1/machine/release
download 81 "$work/version" "$root/version" || die 'No machine release is available from this server.'
IFS= read -r version < "$work/version" || die 'Invalid machine release version.'
case $version in ''|[!a-zA-Z0-9]*|*[!a-zA-Z0-9._-]*) die 'Invalid machine release version.' ;; esac
[ "${#version}" -le 80 ] && [ "$(wc -c < "$work/version")" -eq "$((${#version} + 1))" ] \
  || die 'Invalid machine release version.'
prefix=$root/$version/$platform
download 512 "$work/manifest" "$prefix/manifest"
download 4096 "$work/manifest.sig" "$prefix/manifest.sig"
printf 'calternal-release %s\n' "$release_key" > "$work/allowed_signers"
ssh-keygen -Y verify -f "$work/allowed_signers" -I calternal-release -n calternal-machine-release-v1 \
  -s "$work/manifest.sig" < "$work/manifest" >/dev/null || die 'The machine release signature is invalid.'

{
  IFS= read -r format && IFS= read -r signed_version && IFS= read -r signed_platform &&
    IFS= read -r size && IFS= read -r digest
} < "$work/manifest" || die 'Invalid machine release manifest.'
[ "$format" = calternal-machine-release-v1 ] && [ "$signed_version" = "$version" ] &&
  [ "$signed_platform" = "$platform" ] &&
  [ "$(wc -l < "$work/manifest")" -eq 5 ] || die 'The signed release has the wrong version or platform.'
# Require canonical framing and decimal size, matching the release preparation script.
[ "$(wc -c < "$work/manifest")" -eq $((${#format} + ${#signed_version} + ${#signed_platform} + ${#size} + ${#digest} + 5)) ] \
  || die 'Invalid machine release manifest.'
case $size in ''|0*|*[!0-9]*) die 'Invalid machine program size.' ;; esac
[ "${#size}" -le 9 ] && [ "$size" -le 268435456 ] || die 'Invalid machine program size.'
case $digest in *[!0-9a-f]*) die 'Invalid machine program digest.' ;; esac
[ "${#digest}" -eq 64 ] || die 'Invalid machine program digest.'
download "$size" "$work/calternal" "$prefix/calternal"
[ "$(wc -c < "$work/calternal")" -eq "$size" ] || die 'The machine program size does not match the signed release.'
if command -v sha256sum >/dev/null; then
  sum=$(sha256sum < "$work/calternal")
else
  sum=$(shasum -a 256 < "$work/calternal")
fi
[ "${sum%% *}" = "$digest" ] || die 'The machine program digest does not match the signed release.'
chmod 0700 "$work/calternal"

if [ -f "$machine_dir/credentials" ] && [ -x "$target" ] && [ -f "$service" ] &&
   ! cmp -s "$work/calternal" "$target"; then
  printf '%s\n' 'The installed service updates its program after active turns finish.'
  exit 0
fi

if [ -f "$machine_dir/credentials" ]; then
  printf '%s\n' 'This machine is already enrolled.'
else
  if [ -z "$code" ]; then
    printf 'One-time code from Calternal: ' >&2
    if ! terminal_mode=$(stty -g </dev/tty 2>/dev/null); then die 'Open a terminal or use the command from the app.'; fi
    stty -echo </dev/tty
    IFS= read -r code </dev/tty || code=
    stty "$terminal_mode" </dev/tty
    terminal_mode=
    printf '\n' >&2
    [ -n "$code" ] || die 'Get a fresh enrollment code from the app.'
  fi
  set -- "$work/calternal" machine enroll --server "$server" --dir "$machine_dir" --code -
  if [ "$backend" = direct ]; then set -- "$@" --direct; fi
  if [ "$code" = - ]; then "$@"; else printf '%s\n' "$code" | "$@"; fi
  unset code
fi

mkdir -p "$machine_dir/bin"
changed=false
if ! [ -x "$target" ] || ! cmp -s "$work/calternal" "$target"; then
  binary_stage=$(mktemp "$target.XXXXXXXX")
  cp "$work/calternal" "$binary_stage"
  chmod 0700 "$binary_stage"
  mv -f "$binary_stage" "$target"
  changed=true
fi

set -- "$target" machine run --self-update --dir "$machine_dir" --backend "$backend" --podman "$engine" --image "$image" --max-sandboxes "$max"
printf '%s\n' 'Installing and starting the machine service for your account.'
case $system in
  Linux)
    unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
    unit=$unit_dir/calternal-machine.service
    mkdir -p "$unit_dir"
    {
      printf '%s\n' '[Unit]' 'Description=Calternal machine' '[Service]'
      printf 'ExecStart=:'
      for arg do
        escaped=$(printf '%s' "$arg" | sed 's/[\\"]/\\&/g; s/%/%%/g')
        printf '"%s" ' "$escaped"
      done
      printf '\n%s\n' 'UMask=0077' 'UnsetEnvironment=CONTAINER_HOST CONTAINER_CONNECTION DOCKER_HOST' 'TimeoutStopSec=105s' 'Restart=on-failure' 'RestartSec=5' 'RestartPreventExitStatus=3' '[Install]' 'WantedBy=default.target'
    } > "$unit.new"
    if cmp -s "$unit.new" "$unit"; then
      rm -f "$unit.new"
    else
      mv -f "$unit.new" "$unit"
      systemctl --user daemon-reload
      changed=true
    fi
    if "$changed"; then
      systemctl --user enable calternal-machine.service
      systemctl --user restart calternal-machine.service
    else
      systemctl --user enable --now calternal-machine.service
    fi
    ;;
  Darwin)
    agents=$HOME/Library/LaunchAgents
    plist=$agents/org.calternal.machine.plist
    domain=gui/$(id -u)
    mkdir -p "$agents"
    {
      printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' '<plist version="1.0"><dict>'
      printf '%s\n' '<key>Label</key><string>org.calternal.machine</string>' '<key>ProgramArguments</key><array>'
      for arg do
        escaped=$(printf '%s' "$arg" | sed 's/\&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g')
        printf '<string>%s</string>\n' "$escaped"
      done
      printf '%s\n' '</array><key>RunAtLoad</key><true/>' '<key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>' '</dict></plist>'
    } > "$plist.new"
    if cmp -s "$plist.new" "$plist"; then
      rm -f "$plist.new"
    else
      mv -f "$plist.new" "$plist"
      changed=true
    fi
    if "$changed"; then
      launchctl bootout "$domain/org.calternal.machine" 2>/dev/null || true
    fi
    if ! launchctl print "$domain/org.calternal.machine" >/dev/null 2>&1; then
      launchctl bootstrap "$domain" "$plist"
    fi
    ;;
esac
printf '%s\n' 'The service is installed. Return to Calternal to check that the machine connected.'
}
