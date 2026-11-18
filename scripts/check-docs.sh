#!/bin/sh
# Fail on missing public documentation and broken Rust/TypeScript reference links.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
# Isolate pinned documentation tools from the product dependency tree and lockfile.
if [ ! -d scripts/docs-tools/node_modules/typedoc ]; then
  npm ci --prefix scripts/docs-tools --ignore-scripts --no-audit --no-fund
fi
./scripts/with-toolchain.sh cargo check --locked -j"${CARGO_BUILD_JOBS:-6}" --workspace --all-targets --all-features
# The conformance suite has its own manifest outside the workspace.
./scripts/with-toolchain.sh cargo check --locked -j"${CARGO_BUILD_JOBS:-6}" --manifest-path tests/conformance/Cargo.toml --all-targets
# rustdoc link checks run only during documentation generation, not cargo check.
./scripts/with-toolchain.sh cargo doc --locked -j"${CARGO_BUILD_JOBS:-6}" --no-deps --workspace \
  --all-features --target-dir reference-docs/rust
# TypeScript reference checks resolve workspace packages through their built type output.
npm run build:plugins --silent
npm ci --prefix apps/docs --ignore-scripts --no-audit --no-fund
node scripts/docs-site/generate.mjs --check
npm run check --prefix scripts/docs-tools
node packages/genui/scripts/check-docs.mjs
