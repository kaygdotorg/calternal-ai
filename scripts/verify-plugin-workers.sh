#!/bin/sh
# Full lane qualification: run inside nix develop on the assigned development VM.
set -eu
case "$(hostname)" in
  calternal-dev-01-compute02) ;;
  *) echo 'Run this gate on calternal-dev-01-compute02 inside nix develop.' >&2; exit 1 ;;
esac
npm run build:plugins
cargo fmt --all -- --check
rustfmt --edition 2024 --config skip_children=true --check tests/conformance/isolation.rs tests/conformance/worker_cases.rs
./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features -j6
cargo build --locked -p calternal-server --features test-plugins -j6
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1
npm run test:integration
npm run test:plugins
if podman version >/dev/null 2>&1; then
  CALTERNAL_WORKER_TEST_BACKEND=podman \
    ./scripts/check-podman-tests.sh cargo test --locked -p calternal-server --all-features -j6 --test plugin_host -- --test-threads=1
else
  echo 'SKIP container plugin host tests: Podman is unavailable.'
fi
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
cargo build --locked -p calternal-cli -j6
python3 contract/verify-cli.py
CARGO_TARGET_DIR="$PWD/target" scripts/check-docs.sh
