#!/usr/bin/env bash
# Run inside the pinned x86_64-linux deployment shell on the build host.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
image=${1:?Usage: build-image.sh IMAGE}
target=x86_64-unknown-linux-musl
build_dir=${CARGO_TARGET_DIR:-target}
binary=$build_dir/$target/release/calternal
# Coordinated machine artifacts use this same revision and an owner-supplied public key.
if [[ -n ${RELEASE_REVISION:-} ]]; then
  export CALTERNAL_MACHINE_RELEASE_ID=${CALTERNAL_MACHINE_RELEASE_ID:-$RELEASE_REVISION}
fi

npm ci --ignore-scripts --no-audit --no-fund
npm run build:plugins
cargo build --locked --release --target "$target" -j6 -p calternal-server --bin calternal
# A shipped server serves its own web app: the bundle talks to that server by default
# (fixtures stay reachable with ?server=0), at the site root.
VITE_SERVER_DEFAULT=1 npm run web:build

context=$(mktemp -d)
trap 'rm -rf -- "$context"' EXIT
root=$context/rootfs
install -d "$root/usr/local/bin" "$root/app/plugins" "$root/data/"{appdata,userdata-ssd,userdata-hdd}
install -m 0755 "$binary" "$root/usr/local/bin/calternal"
rustc --edition=2024 --target "$target" -C opt-level=s -C strip=symbols \
  -C "linker=$CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER" \
  deploy/health.rs -o "$root/usr/local/bin/calternal-health"
strip --strip-unneeded "$root/usr/local/bin/calternal"
for executable in "$root/usr/local/bin/"*; do
  if readelf -l "$executable" | awk '/INTERP/ { found=1 } END { exit !found }'; then
    echo "Expected a static executable: $executable" >&2
    exit 1
  fi
done

# The same registry drives server admission and image packaging.
python3 - "$root/app/plugins" <<'PY'
import hashlib
import json
import shutil
import sys
from pathlib import Path

registry = Path("crates/server/trusted-plugins.json")
destination = Path(sys.argv[1])
shutil.copyfile(registry, destination / registry.name)
for plugin in json.loads(registry.read_text())["plugins"]:
    for key in ("manifest", "worker"):
        if path := plugin[key]:
            source = Path("plugins", path)
            if key == "worker" and hashlib.sha256(source.read_bytes()).hexdigest() != plugin["worker_sha256"]:
                sys.exit(f"Worker digest mismatch: {plugin['id']}")
            output = destination / path
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, output)
PY

test -f apps/web/dist/index.html
cp -R apps/web/dist "$root/app/web"
find "$root" -type d -exec chmod 0755 {} +
find "$root/app" -type f -exec chmod 0644 {} +
cp deploy/Containerfile "$context/"
env -u SOURCE_DATE_EPOCH podman build --format docker --timestamp "${SOURCE_DATE_EPOCH:-0}" \
  --label "org.calternal.deploy.repository=${image%:*}" \
  --label "org.opencontainers.image.revision=${RELEASE_REVISION:-unknown}" \
  --tag "$image" "$context"
