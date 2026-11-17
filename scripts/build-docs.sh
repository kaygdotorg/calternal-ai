#!/bin/sh
# Build validated reference sites from comments; Rust deps are intentionally omitted.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
./scripts/check-docs.sh
npm run build --prefix scripts/docs-tools
printf '%s\n' 'Rust: reference-docs/rust/doc/calternal_core/index.html' \
  'TypeScript: reference-docs/typescript/plugin-sdk/index.html'
