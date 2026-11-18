#!/usr/bin/env bash
# Build the co-located machine's official Claude Code image on the build VM.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
image=${1:-localhost/calternal-ai-claude:current}
test -f plugins/calternal-claude-code/dist/main.js ||
  { echo 'Build the adapter first with npm run build:plugins.' >&2; exit 1; }
context=$(mktemp -d)
trap 'rm -rf -- "$context"' EXIT
mkdir -p "$context/plugins/calternal-claude-code"
cp -R plugins/calternal-claude-code/{dist,bin} "$context/plugins/calternal-claude-code/"
cp plugins/calternal-claude-code/Containerfile "$context/"
cat >> "$context/Containerfile" <<'CONTAINER'
RUN npm install --global --no-audit --no-fund --registry=https://registry.npmjs.org @anthropic-ai/claude-code@2.1.326 \
    && claude --version \
    && npm cache clean --force
CONTAINER
printf '{"auths":{}}\n' > "$context/auth.json"
podman build --authfile "$context/auth.json" --tag "$image" "$context"
