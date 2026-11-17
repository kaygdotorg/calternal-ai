#!/usr/bin/env bash
# Build one of the lane images from an already-built provider adapter.
set -euo pipefail
cd -- "$(dirname -- "$0")/../.."
case ${1:-} in
  fixture)
    exec plugins/calternal-claude-code/test/build-image.sh localhost/calternal-e2e-fixture:dev
    ;;
  claude) ;;
  *) echo 'Usage: image.sh fixture|claude' >&2; exit 2 ;;
esac

# A bounded build context keeps the host's working tree and home out of the image.
context=$(mktemp -d)
trap 'rm -rf -- "$context"' EXIT
mkdir -p "$context/plugins/calternal-claude-code"
cp -R plugins/calternal-claude-code/{dist,bin} "$context/plugins/calternal-claude-code/"
cp plugins/calternal-claude-code/Containerfile "$context/Containerfile"
cat >> "$context/Containerfile" <<'CONTAINER'
RUN npm install --global --no-audit --no-fund --registry=https://registry.npmjs.org @anthropic-ai/claude-code@2.1.326 \
    && claude --version \
    && npm cache clean --force
CONTAINER
printf '{"auths":{}}\n' > "$context/auth.json"
podman build --authfile "$context/auth.json" -t localhost/calternal-e2e-claude:dev "$context"
