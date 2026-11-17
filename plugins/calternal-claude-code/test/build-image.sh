#!/bin/sh
# Build the account-free provider fixture on a development VM, using a bounded
# context and anonymous registry access. No ambient registry/provider login is read.
set -eu
provider_root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
provider_image=${1:-localhost/calternal-provider-fixture}
if [ ! -f "$provider_root/plugins/calternal-claude-code/dist/main.js" ]; then
  echo 'Build the adapter first: npm run build:plugins (on a development VM).' >&2
  exit 1
fi
provider_context=$(mktemp -d)
trap 'rm -rf "$provider_context"' 0
trap 'exit 1' HUP INT TERM
mkdir -p "$provider_context/plugins/calternal-claude-code"
cp -R "$provider_root/plugins/calternal-claude-code/dist" "$provider_root/plugins/calternal-claude-code/bin" "$provider_context/plugins/calternal-claude-code/"
cp "$provider_root/plugins/calternal-claude-code/Containerfile" "$provider_context/Containerfile"
cp "$provider_root/plugins/calternal-claude-code/test/fake-claude.mjs" "$provider_context/fake-claude.mjs"
printf '\nCOPY fake-claude.mjs /usr/local/bin/claude\nRUN chmod 0755 /usr/local/bin/claude\n' >> "$provider_context/Containerfile"
printf '{"auths":{}}\n' > "$provider_context/auth.json"
podman build --authfile "$provider_context/auth.json" -t "$provider_image" "$provider_context"
