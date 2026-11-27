#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
image=${1:-localhost/calternal-chat-claude-code-fake:dev}
adapter="$repo_root/plugins/calternal-claude-code"
harness="$repo_root/plugins-v2/calternal-chat-claude-code"

if [ ! -f "$adapter/dist/main.js" ]; then
  echo "Build the adapter first with npm run build:plugins (on the dev VM)." >&2
  exit 1
fi

context=$(mktemp -d)
trap 'rm -rf "$context"' EXIT
trap 'exit 1' HUP INT TERM

mkdir -p "$context/plugins/calternal-claude-code"
cp -R "$adapter/dist" "$adapter/bin" "$context/plugins/calternal-claude-code/"
cp "$adapter/Containerfile" "$context/Containerfile"
cp "$harness/test/fake-claude-echo.mjs" "$context/fake-claude-echo.mjs"

cat >> "$context/Containerfile" <<'EOF'

COPY fake-claude-echo.mjs /usr/local/bin/claude
RUN chmod 0755 /usr/local/bin/claude
EOF

printf '{"auths":{}}\n' > "$context/auth.json"
podman build --authfile "$context/auth.json" -t "$image" "$context"
