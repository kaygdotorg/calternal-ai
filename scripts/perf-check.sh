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
cargo build --release -j6 -p calternal-server
cargo build --release -j6 -p calternal-perf
cp target/release/calternal target/release/calternal-production
cargo build --release -j6 -p calternal-server --features test-plugins
cp target/release/calternal target/release/calternal-test
REMOTE
ssh "$perf" "mkdir -p ~/calternal-ai-perf/$run/bin"
# A Nix-built executable needs its runtime closure, not a build on the perf VM.
# Export the runtime closure through the coordinator; the VMs need not reach each other.
ssh "$dev" 'bash -s' <<'REMOTE'
set -euo pipefail
cd ~/Developer/lanes/perf-budgets
ldd target/release/calternal-production target/release/calternal-test target/release/calternal-perf | sed -nE 's@.*(/nix/store/[^/ ]+).*@\1@p' | sort -u > runtime-paths.txt
nix-store -qR $(cat runtime-paths.txt) > runtime-closure.txt
REMOTE
ssh "$dev" 'cd ~/Developer/lanes/perf-budgets && nix-store --export $(cat runtime-closure.txt)' | ssh "$perf" 'nix-store --import >/dev/null'
scp -3 -q "$dev:Developer/lanes/perf-budgets/target/release/calternal-production" "$perf:calternal-ai-perf/$run/bin/calternal"
scp -3 -q "$dev:Developer/lanes/perf-budgets/target/release/calternal-perf" "$perf:calternal-ai-perf/$run/bin/calternal-perf"
scp -3 -q "$dev:Developer/lanes/perf-budgets/target/release/calternal-test" "$perf:calternal-ai-perf/$run/bin/calternal-test"
# Only source manifests and built workers are transferred, not node_modules.
ssh "$dev" 'cd ~/Developer/lanes/perf-budgets && tar --exclude=node_modules -cf - plugins tests/plugins' | ssh "$perf" "cd ~/calternal-ai-perf/$run && tar xf -"
scp -q perf/budgets.json perf/repeat-plan.py "$perf:calternal-ai-perf/$run/"
set +e
ssh "$perf" /usr/local/bin/perf-lock.sh bash -s -- "$run" "$head_commit" "$PERF_BASELINE_BIN" "$PERF_BASELINE_COMMIT" > "/tmp/perf-budgets-$run.log" <<'REMOTE'
set -uo pipefail
run=$1 commit=$2 baseline=$3 baseline_commit=$4
root="$HOME/calternal-ai-perf/$run"
cd "$root"
./bin/calternal-perf --bin "$root/bin/calternal" --baseline "$baseline"   --work "$root/work" --plugins "$root/plugins" --budgets budgets.json   --commit "$commit" --baseline-commit "$baseline_commit" --json > full-output.json
full_status=$?
[[ -f "$root/work/summary.json" ]] || exit 2
python3 repeat-plan.py "$root/work/summary.json" > repeat-plan.txt || exit 2
while read -r spaces notes cold; do
  [[ -n "$spaces" ]] || continue
  sizes=''
  if [[ "$spaces" == 1 ]]; then sizes=$notes; fi
  cold_pairs=0
  if [[ "$cold" == 1 ]]; then cold_pairs=3; fi
  ./bin/calternal-perf --bin "$root/bin/calternal" --baseline "$baseline" \
    --work "$root/repeat-${spaces}x${notes}" --plugins "$root/plugins" --budgets budgets.json \
    --commit "$commit" --baseline-commit "$baseline_commit" --sizes "$sizes" --spaces "$spaces" \
    --pairs 3 --cold-pairs "$cold_pairs" --json > "repeat-${spaces}x${notes}-output.json"
  [[ -f "$root/repeat-${spaces}x${notes}/summary.json" ]] || exit 2
done < repeat-plan.txt
./bin/calternal-perf --bin "$root/bin/calternal" --baseline "$baseline"   --work "$root/idle" --plugins "$root/plugins" --budgets budgets.json   --commit "$commit" --baseline-commit "$baseline_commit" --idle-only --json > idle-output.json
idle_status=$?
for repeat in 0 1 2; do
  ./bin/calternal-perf --bin "$root/bin/calternal-test" --baseline "$root/bin/calternal-test" \
    --work "$root/jobs-$repeat" --plugins "$root/plugins" --test-plugin-root "$root/tests/plugins" \
    --budgets budgets.json --commit "$commit" --baseline-commit "$commit" --jobs-only --json > "jobs-$repeat-output.json"
  [[ -f "$root/jobs-$repeat/jobs-summary.json" ]] || exit 2
done
printf 'Full suite: %s\nIdle suite: %s\n'  "$full_status" "$idle_status"
if [[ "$full_status" == 1 || "$idle_status" == 1 ]]; then exit 1; fi
exit 2 # Mandatory instrumentation gaps remain explicit, never silently green.
REMOTE
rc=$?
set -e
printf 'Run: %s\nHead: %s\nBaseline: %s\nExit: %s\nEvidence: %s\n' "$run" "$head_commit" "$PERF_BASELINE_COMMIT" "$rc" "/tmp/perf-budgets-$run.log"
# Only this lane's targets; never another lane or the existing app checkout.
ssh "$dev" 'rm -rf ~/Developer/lanes/perf-budgets/target ~/Developer/lanes/perf-budgets/tests/conformance/target'
exit "$rc"
