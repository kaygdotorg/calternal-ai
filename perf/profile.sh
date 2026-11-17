#!/usr/bin/env bash
# Instrument a separate 1k control run. Never substitute profiled samples for gate data.
set -euo pipefail
if [[ ${CALTERNAL_PERF_HELD:-0} != 1 ]]; then
  exec /usr/local/bin/perf-lock.sh "$0" "$@"
fi
suite=${1:?Usage: profile.sh /absolute/suite-directory commit}
commit=${2:?Supply measured product commit}
[[ "$suite" =~ ^/[a-zA-Z0-9_./-]+$ && "$commit" =~ ^[a-f0-9]{7,40}$ ]]
tools="$HOME/calternal-ai-perf/tools/extracted"
work="$suite/profile-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$work"
"$suite/bin/calternal-perf" --bin "$suite/bin/calternal" \
  --baseline "$suite/bin/calternal" --work "$work/run" \
  --plugins "$suite/plugins" --budgets "$suite/budgets.json" \
  --commit "$commit" --baseline-commit "$commit" \
  --sizes 1000 --spaces 1 --pairs 3 --cold-pairs 0 --samples 100 --json \
  > "$work/output.json" 2> "$work/driver.log" &
driver=$!
# Only the first arm is profiled. Its server stays alive through draft-feed samples.
startup=''
for ((i=0;i<600;i++)); do
  startup=$(find "$work/run" -path '*-0-baseline/appdata/startup.json' -print -quit 2>/dev/null || true)
  [[ -z "$startup" ]] || break
  if ! kill -0 "$driver" 2>/dev/null; then wait "$driver"; exit 2; fi
  sleep 0.1
done
[[ -n "$startup" ]] || { echo 'No server to profile; inspect driver.log' >&2; exit 2; }
server=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$startup")
set +e
sudo -n timeout -s INT 12 "$tools/usr/bin/strace" -f -c -w \
  -e fsync,fdatasync,pread64,pwrite64,openat -p "$server" -o "$work/syscalls.txt" \
  > "$work/strace.stdout" 2> "$work/strace.stderr" &
trace=$!
sudo -n env LD_LIBRARY_PATH="$tools/usr/lib/x86_64-linux-gnu" \
  "$tools/usr/bin/perf" record -o "$work/perf.data" -e cpu-clock:u -F 99 \
  --call-graph dwarf -p "$server" -- sleep 12 > "$work/perf.stdout" 2> "$work/perf.stderr"
perf_status=$?
wait "$trace"
trace_status=$?
sudo -n env LD_LIBRARY_PATH="$tools/usr/lib/x86_64-linux-gnu" \
  "$tools/usr/bin/perf" report --stdio --no-children -i "$work/perf.data" \
  > "$work/perf-report.txt" 2> "$work/perf-report.stderr"
report_status=$?
wait "$driver"
driver_status=$?
set -e
printf 'Profile: %s\nperf record: %s\nperf report: %s\nstrace: %s (124 means timeout)\ndriver: %s\n' \
  "$work" "$perf_status" "$report_status" "$trace_status" "$driver_status" > "$work/status.txt"
cat "$work/status.txt"
[[ "$perf_status" == 0 && "$report_status" == 0 ]]
