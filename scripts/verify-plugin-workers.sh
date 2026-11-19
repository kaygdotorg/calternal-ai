#!/bin/sh
# Full lane qualification: run inside nix develop on the assigned development VM.
set -eu
case "$(hostname)" in
  calternal-dev-01-compute02) ;;
  *) echo 'Run this gate on calternal-dev-01-compute02 inside nix develop.' >&2; exit 1 ;;
esac
: "${CALTERNAL_UPGRADE_PREVIOUS_IMAGE:?Set the exact previous release image tag or ID}"
: "${CALTERNAL_UPGRADE_CANDIDATE_IMAGE:?Set the exact candidate release image tag or ID}"
npm run build:plugins
cargo test --locked -p calternal-server --lib real_distribution_registers_every_bundled_package_and_operation -- --ignored
cargo fmt --all -- --check
rustfmt --edition 2024 --config skip_children=true --check tests/conformance/isolation.rs tests/conformance/worker_cases.rs
./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features -j6
cargo build --locked -p calternal-server --features test-plugins -j6
# The standalone crate has its own lockfile. Run every default target.
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 -- --test-threads=1
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
# Use the exact production-style builds on both sides of the upgrade.
CARGO_TARGET_DIR="$PWD/target" ./scripts/verify-additive-upgrade.sh \
  "$CALTERNAL_UPGRADE_PREVIOUS_IMAGE" "$CALTERNAL_UPGRADE_CANDIDATE_IMAGE" \
  "$PWD/target/installer-upgrade-proof.json"
