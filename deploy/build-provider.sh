#!/usr/bin/env bash
# Build an exact official harness image on the development VM.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
image=${1:-localhost/calternal-ai-claude:current}
package=${2:-calternal-claude-code}
[[ $package =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]] || { echo 'Invalid provider package name.' >&2; exit 2; }
source="plugins/$package"
test -f "$source/dist/runtime.json" ||
  { echo 'Build the provider first with npm run build:plugins.' >&2; exit 1; }
context=$(mktemp -d)
trap 'rm -rf -- "$context"' EXIT
mkdir -p "$context/$source"
cp -R "$source"/{dist,bin} "$context/$source/"
cp "$source/Containerfile" "$context/"
cp deploy/install-harness.mjs "$context/"
cat >> "$context/Containerfile" <<'CONTAINER'
COPY install-harness.mjs /tmp/install-harness.mjs
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && node /tmp/install-harness.mjs /opt/calternal-provider/dist/runtime.json \
    && npm cache clean --force
CONTAINER
printf '{"auths":{}}\n' > "$context/auth.json"
podman build --authfile "$context/auth.json" --tag "$image" "$context"
