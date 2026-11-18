#!/bin/sh
# Install the Calternal machine service for this user.
# Usage: sh install-machine.sh --server URL --code - --image IMAGE [--binary PATH]
set +x
set -eu
umask 077

die() { printf '%s\n' "$*" >&2; exit 1; }
# The server defaults to sandboxing and can later honor an explicit owner choice.
server= code= image= binary= backend=auto max=4294967295
while [ "$#" -gt 0 ]; do
  [ "$#" -ge 2 ] || die "This option needs a value: $1"
  case $1 in
    --server) server=$2 ;;
    --code) code=$2 ;;
    --image) image=$2 ;;
    --binary) binary=$2 ;;
    --backend) backend=$2 ;;
    --max-sandboxes) max=$2 ;;
    *) die "Unknown installer option." ;;
  esac
  shift 2
done
[ -n "$server" ] || die 'Supply the server URL with --server.'
[ -n "$image" ] || image=docker.io/library/alpine:3
system=$(uname -s)
case $system in
  Linux|Darwin) ;;
  *) die "The machine service does not support $system." ;;
esac
[ -n "$binary" ] || binary=$(command -v calternal || true)
[ -x "$binary" ] || die 'Put the Calternal binary on your PATH, or run this script with --binary /path/to/calternal.'
case $backend in
  podman|auto)
    engine=$(command -v podman) || die 'Install Podman for this user and start it before enrolling.'
    "$engine" info >/dev/null || die 'Start Podman for this user before enrolling.'
    ;;
  direct) engine=podman ;;
  *) die 'Choose the podman, auto or direct backend.' ;;
esac

target=$HOME/.local/bin/calternal
machine_dir=${XDG_DATA_HOME:-$HOME/.local/share}/calternal/machine
case "$target$machine_dir$engine$image$max" in
  *'
'*) die 'Use installation paths without line breaks.' ;;
esac
if [ -f "$machine_dir/credentials" ]; then
  enrolled_server=$(sed -n 's/^server: //p' "$machine_dir/credentials")
  [ "$enrolled_server" = "${server%/}" ] || die 'This directory is already enrolled with a different server.'
fi

printf '%s\n' 'Copying Calternal into your user bin directory.'
mkdir -p "$HOME/.local/bin"
changed=false
if ! [ -x "$target" ] || ! cmp -s "$binary" "$target"; then
  binary_stage=$(mktemp "$target.XXXXXX")
  trap 'rm -f "$binary_stage"' EXIT
  cp "$binary" "$binary_stage"
  chmod 0755 "$binary_stage"
  mv -f "$binary_stage" "$target"
  trap - EXIT
  changed=true
fi

if [ -f "$machine_dir/credentials" ]; then
  printf '%s\n' 'This machine is already enrolled.'
else
  printf '%s\n' 'Enrolling this machine with a single-use code.'
  [ -n "$code" ] || die 'Get a new installer from the app, then run it again.'
  if [ "$code" = - ]; then
    "$target" machine enroll --server "$server" --dir "$machine_dir" --code -
  else
    printf '%s\n' "$code" | "$target" machine enroll --server "$server" --dir "$machine_dir" --code -
  fi
fi

set -- "$target" machine run --dir "$machine_dir" --backend "$backend" --podman "$engine" --image "$image" --max-sandboxes "$max"
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
      printf '\n%s\n' 'Restart=on-failure' 'RestartSec=5' 'RestartPreventExitStatus=3' '[Install]' 'WantedBy=default.target'
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
