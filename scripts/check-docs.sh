#!/bin/sh
# Fail on missing public documentation and broken Rust/TypeScript reference links.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
# Isolate pinned documentation tools from the product dependency tree and lockfile.
if [ ! -d scripts/docs-tools/node_modules/typedoc ]; then
  npm ci --prefix scripts/docs-tools --ignore-scripts --no-audit --no-fund
fi
./scripts/with-toolchain.sh cargo check --locked -j4 \
  -p calternal-core -p calternal-client -p calternal-cli -p calternal-bindings \
  -p calternal-fsio -p calternal-roots \
  --all-targets --all-features
# rustdoc link checks run only during documentation generation, not cargo check.
./scripts/with-toolchain.sh cargo doc --locked -j4 --no-deps \
  -p calternal-core -p calternal-client -p calternal-cli -p calternal-bindings \
  -p calternal-fsio -p calternal-roots \
  --all-features --target-dir reference-docs/rust
npm run check --prefix scripts/docs-tools
