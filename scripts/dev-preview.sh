#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
DATA_DIR=${CALTERNAL_DATA_DIR:-"$ROOT/.dev-data"}
PORT=${CALTERNAL_PORT:-3210}
if [ ! -f apps/web/dist/index.html ] || [ ! -f plugins/notes-insights/dist/worker.js ]; then
  echo "Build the web app and plugins first: npm install && npm run build" >&2
  exit 1
fi
exec "$ROOT/scripts/with-toolchain.sh" cargo run --locked -p calternal-server -- serve \
  --data-dir "$DATA_DIR" --port "$PORT" --web-dir "$ROOT/apps/web/dist" --plugin-root "$ROOT/plugins"
