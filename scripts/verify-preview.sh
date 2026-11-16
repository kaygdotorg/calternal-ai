#!/bin/sh
# Run from a clean dependency install; no provider credentials or real data.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
./scripts/with-toolchain.sh cargo fmt --all -- --check
./scripts/with-toolchain.sh cargo run --locked -p calternal-core --bin export-contract -- --check
npm run build
npm run check --workspace @calternal/web
npm test --workspace @calternal/web
npm run test:plugins
./scripts/with-toolchain.sh cargo test --locked --workspace
./scripts/with-toolchain.sh cargo build --locked -p calternal-server -p calternal-cli
npm run test:integration
python3 contract/verify-cli.py
./scripts/with-toolchain.sh cargo check --locked -p calternal-bindings --features uniffi
