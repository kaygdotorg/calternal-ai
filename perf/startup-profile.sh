#!/usr/bin/env bash
# Profile startup after provisioning; never mix these samples with latency gates.
set -euo pipefail
if [[ ${CALTERNAL_PERF_HELD:-0} != 1 ]]; then exec /usr/local/bin/perf-lock.sh "$0" "$@"; fi
suite=${1:?Supply absolute suite directory}
commit=${2:?Supply product commit}
driver=${3:?Supply absolute current harness binary}
[[ "$suite" =~ ^/[a-zA-Z0-9_./-]+$ && "$driver" =~ ^/[a-zA-Z0-9_./-]+$ && "$commit" =~ ^[a-f0-9]{7,40}$ ]]
work="$suite/startup-profile-$(date -u +%Y%m%dT%H%M%SZ)"
roots="$work/roots"
tools="$HOME/calternal-ai-perf/tools/extracted"
mkdir -p "$work"
{ uname -a; lscpu; free -b; findmnt -T "$suite"; lsblk -o NAME,SIZE,ROTA,FSTYPE,MOUNTPOINTS; } > "$work/machine.txt"
sha256sum "$suite/bin/"* "$suite/budgets.json" > "$work/artifacts.sha256"
"$driver" --bin "$suite/bin/calternal" --baseline "$suite/bin/calternal" \
 --work "$roots" --plugins "$suite/plugins" --budgets "$suite/budgets.json" \
 --sizes 100 --spaces 1000 --prepare-only --commit "$commit" --baseline-commit "$commit" --json \
 > "$work/prepare.json" 2> "$work/prepare.log"
sudo -n /usr/local/sbin/perf-drop-caches
"$suite/bin/calternal" serve --port 0 --origin http://localhost:18080 \
 --installation-id 00000000-0000-0000-0000-000000000001 --create \
 --appdata "$roots/appdata" --userdata-ssd "$roots/userdata-ssd" \
 --userdata-hdd "$roots/userdata-hdd" --plugin-root "$suite/plugins" \
 > "$work/server.log" 2>&1 &
server=$!
trap 'kill -TERM "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true' EXIT
sudo -n env LD_LIBRARY_PATH="$tools/usr/lib/x86_64-linux-gnu" timeout -s INT 20 \
 "$tools/usr/bin/strace" -f -c -w -e fsync,fdatasync,openat,clone,clone3 -p "$server" \
 -o "$work/syscalls.txt" > "$work/strace.stdout" 2> "$work/strace.stderr" &
trace=$!
sudo -n env LD_LIBRARY_PATH="$tools/usr/lib/x86_64-linux-gnu" \
 "$tools/usr/bin/perf" record -o "$work/perf.data" -e cpu-clock:u -F 99 \
 --call-graph dwarf -p "$server" -- sleep 20 > "$work/perf.stdout" 2> "$work/perf.stderr"
sudo -n env LD_LIBRARY_PATH="$tools/usr/lib/x86_64-linux-gnu" \
 "$tools/usr/bin/perf" report --stdio --no-children -i "$work/perf.data" \
 > "$work/perf-report.txt" 2> "$work/perf-report.stderr"
wait "$trace" || true
kill -TERM "$server"
wait "$server" || true
trap - EXIT
rm -rf "$roots"
printf 'Startup profile: %s\n' "$work"
