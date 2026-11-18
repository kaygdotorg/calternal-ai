#!/bin/sh
# Run the lane's required repository gates on a development VM; keep each result.
set -u
cd "$(dirname "$0")/../.."
mkdir -p apps/docs/.cache/gates
summary="$PWD/apps/docs/.cache/gates/results.txt"
: > "$summary"
run_gate() {
  name=$1
  shift
  if "$@" > "apps/docs/.cache/gates/$name.log" 2>&1; then
    printf '%s PASS\n' "$name" | tee -a "$summary"
  else
    code=$?
    printf '%s FAIL (%s)\n' "$name" "$code" | tee -a "$summary"
    tail -35 "apps/docs/.cache/gates/$name.log"
  fi
}
run_gate plugin-build npm run build:plugins
run_gate build cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins
run_gate workspace ./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features -j6
run_gate conformance env CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1 --show-output
run_gate integration npm run test:integration
run_gate plugins npm run test:plugins
run_gate genui npm test --workspace @calternal/genui
run_gate contract cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
run_gate docs sh scripts/check-docs.sh
run_gate publication node --test scripts/docs-site/publication.test.mjs
if grep -q ' FAIL ' "$summary"; then exit 1; fi
