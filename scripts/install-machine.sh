#!/bin/sh
# Install the Calternal machine role for the current user and enrol it.
#
#   scripts/install-machine.sh --server https://cal.example.org --code CODE [--binary PATH]
#                              [--backend podman|process] [--max-sandboxes N]
#
# Deterministic, no agent involved. Steps, each idempotent:
#   1. copy the `calternal` binary to ~/.local/bin/calternal (mode 0755);
#   2. enrol with the one-time code (skipped when credentials already exist);
#   3. install and start a user-level service:
#      Linux: ~/.config/systemd/user/calternal-machine.service, enabled with
#             `systemctl --user enable --now`; run `loginctl enable-linger $USER`
#             once (needs admin rights) so it runs without a login session.
#      macOS: ~/Library/LaunchAgents/org.calternal.machine.plist, loaded with
#             `launchctl bootstrap gui/$UID` (starts at login, restarts on exit).
# Exit code 3 from the service means the machine was revoked; systemd then does
# not restart it (launchd has no such filter: it retries every 10 s and exits again). Credentials: ~/.local/share/calternal/machine/credentials (0600).
set -eu

server= code= binary= backend=podman max=10
while [ $# -gt 0 ]; do
  case $1 in
    --server) server=$2; shift 2 ;;
    --code) code=$2; shift 2 ;;
    --binary) binary=$2; shift 2 ;;
    --backend) backend=$2; shift 2 ;;
    --max-sandboxes) max=$2; shift 2 ;;
    *) echo "install-machine: unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$server" ] || { echo "install-machine: --server is required" >&2; exit 2; }
[ -n "$binary" ] || binary=$(command -v calternal || true)
[ -n "$binary" ] && [ -x "$binary" ] || { echo "install-machine: no calternal binary (pass --binary)" >&2; exit 2; }

bin_dir=$HOME/.local/bin
machine_dir=${XDG_DATA_HOME:-$HOME/.local/share}/calternal/machine
target=$bin_dir/calternal
mkdir -p "$bin_dir"
if [ "$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")" != "$target" ]; then
  cp "$binary" "$target.new" && chmod 0755 "$target.new" && mv -f "$target.new" "$target"
fi
echo "install-machine: binary $target"

if [ -f "$machine_dir/credentials" ]; then
  echo "install-machine: already enrolled ($machine_dir/credentials)"
else
  [ -n "$code" ] || { echo "install-machine: --code is required to enrol" >&2; exit 2; }
  CALTERNAL_ENROL_CODE=$code "$target" machine enrol --server "$server" --dir "$machine_dir"
fi

run_args="machine run --dir $machine_dir --backend $backend --max-sandboxes $max"
case $(uname -s) in
  Linux)
    unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
    mkdir -p "$unit_dir"
    cat > "$unit_dir/calternal-machine.service.new" <<EOF
[Unit]
Description=Calternal machine role
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=$target $run_args
Restart=on-failure
RestartSec=5
RestartPreventExitStatus=3
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
EOF
    mv -f "$unit_dir/calternal-machine.service.new" "$unit_dir/calternal-machine.service"
    systemctl --user daemon-reload
    systemctl --user enable --now calternal-machine.service
    systemctl --user restart calternal-machine.service
    echo "install-machine: systemd user service calternal-machine running"
    ;;
  Darwin)
    agents=$HOME/Library/LaunchAgents
    plist=$agents/org.calternal.machine.plist
    mkdir -p "$agents" "$HOME/Library/Logs"
    args=""
    for a in $target $run_args; do args="$args    <string>$a</string>
"; done
    cat > "$plist.new" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>org.calternal.machine</string>
  <key>ProgramArguments</key>
  <array>
$args  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>StandardErrorPath</key><string>$HOME/Library/Logs/calternal-machine.log</string>
</dict>
</plist>
EOF
    mv -f "$plist.new" "$plist"
    launchctl bootout "gui/$(id -u)/org.calternal.machine" 2>/dev/null || true
    launchctl bootstrap "gui/$(id -u)" "$plist"
    echo "install-machine: launchd agent org.calternal.machine loaded"
    ;;
  *) echo "install-machine: unsupported system $(uname -s)" >&2; exit 1 ;;
esac
