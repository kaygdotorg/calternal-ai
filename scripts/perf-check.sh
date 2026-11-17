#!/usr/bin/env bash
# Build on dev-01, copy artifacts, then measure under the perf-01 lock. No perf builds.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${PERF_BASELINE_BIN:?Set PERF_BASELINE_BIN to an absolute baseline binary path on perf-01}"
: "${PERF_BASELINE_COMMIT:?Set PERF_BASELINE_COMMIT to the baseline git commit}"
[[ "$PERF_BASELINE_BIN" =~ ^/[a-zA-Z0-9_./-]+$ ]]
[[ "$PERF_BASELINE_COMMIT" =~ ^[a-f0-9]{7,40}$ ]]
run="$(date -u +%Y%m%dT%H%M%SZ)-$$"
head_commit=$(git rev-parse HEAD)
dev=claude@calternal-dev-01-compute02
perf=kayg@calternal-perf-01-backup01
archive="/tmp/perf-budgets-$run.tar"
trap 'rm -f "$archive"' EXIT
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf "$archive"
ssh "$dev" 'mkdir -p ~/Developer/lanes/perf-budgets && cd ~/Developer/lanes/perf-budgets && tar xf -' < "$archive"
ssh "$dev" 'cd ~/Developer/lanes/perf-budgets && nix develop -c bash -s' <<'REMOTE'
set -euo pipefail
npm ci --ignore-scripts --no-audit --no-fund
npm run build:plugins
cargo build --release -j6 -p calternal-server -p calternal-perf
REMOTE
ssh "$perf" "mkdir -p ~/calternal-ai-perf/$run/bin"
# A Nix-built executable needs its runtime closure, not a build on the perf VM.
# Export the runtime closure through the coordinator; the VMs need not reach each other.
ssh "$dev" 'bash -s' <<'REMOTE'
set -euo pipefail
cd ~/Developer/lanes/perf-budgets
ldd target/release/calternal target/release/calternal-perf | sed -nE 's@.*(/nix/store/[^/ ]+).*@\1@p' | sort -u > runtime-paths.txt
nix-store -qR $(cat runtime-paths.txt) > runtime-closure.txt
REMOTE
ssh "$dev" 'cd ~/Developer/lanes/perf-budgets && nix-store --export $(cat runtime-closure.txt)' | ssh "$perf" 'nix-store --import >/dev/null'
scp -3 -q "$dev:Developer/lanes/perf-budgets/target/release/calternal" "$perf:calternal-ai-perf/$run/bin/calternal"
scp -3 -q "$dev:Developer/lanes/perf-budgets/target/release/calternal-perf" "$perf:calternal-ai-perf/$run/bin/calternal-perf"
# Only source manifests and built workers are transferred, not node_modules.
ssh "$dev" 'cd ~/Developer/lanes/perf-budgets && tar --exclude=node_modules -cf - plugins' | ssh "$perf" "cd ~/calternal-ai-perf/$run && tar xf -"
scp -q perf/budgets.json "$perf:calternal-ai-perf/$run/budgets.json"
set +e
ssh "$perf" /usr/local/bin/perf-lock.sh bash -s -- "$run" "$head_commit" "$PERF_BASELINE_BIN" "$PERF_BASELINE_COMMIT" > "/tmp/perf-budgets-$run.log" <<'REMOTE'
set -uo pipefail
run=$1 commit=$2 baseline=$3 baseline_commit=$4
root="$HOME/calternal-ai-perf/$run"
cd "$root"
./bin/calternal-perf --bin "$root/bin/calternal" --baseline "$baseline"   --work "$root/work" --plugins "$root/plugins" --budgets budgets.json   --commit "$commit" --baseline-commit "$baseline_commit" --json > full-output.json
full_status=$?
[[ -f "$root/work/summary.json" ]] || exit 2
./bin/calternal-perf --bin "$root/bin/calternal" --baseline "$baseline"   --work "$root/idle" --plugins "$root/plugins" --budgets budgets.json   --commit "$commit" --baseline-commit "$baseline_commit" --idle-only --json > idle-output.json
idle_status=$?
printf 'Full suite: %s\nIdle suite: %s\n' "$full_status" "$idle_status"
if [[ "$full_status" == 1 || "$idle_status" == 1 ]]; then exit 1; fi
exit 2 # Mandatory instrumentation gaps remain explicit, never silently green.
REMOTE
rc=$?
set -e
printf 'Run: %s\nHead: %s\nBaseline: %s\nExit: %s\nEvidence: %s\n' "$run" "$head_commit" "$PERF_BASELINE_COMMIT" "$rc" "/tmp/perf-budgets-$run.log"
# Only this lane's targets; never another lane or the existing app checkout.
ssh "$dev" 'rm -rf ~/Developer/lanes/perf-budgets/target ~/Developer/lanes/perf-budgets/tests/conformance/target'
exit "$rc"
