#!/bin/sh
# Builds with cloud signing and uploads to Speedflight so a registered iPhone can install it from a link.
# Usage: ./share-build.sh "title" "notes". Prints the page link; never prints the upload secret
# (.env.speedflight, untracked, defines SPEEDFLIGHT_SECRET).
set -eu
cd "$(dirname "$0")"
. ./common.sh
. ./.env.speedflight
N=$(date +%Y%m%d%H%M)
archive_and_export "$N"
B=cloud.calternal.app; BASE=https://speedflight.dev
META=$(python3 -c "import json,sys;print(json.dumps({\"title\":sys.argv[1],\"notes\":sys.argv[2],\"deepLink\":\"calternal://\",\"branch\":\"local\",\"commit\":sys.argv[3],\"author\":\"coordinator\"}))" "${1:-Build}" "${2:-}" "$N")
C=$(curl -sS --retry 3 -X POST "$BASE/api/apps/$SPEEDFLIGHT_SECRET/$B/builds" -H "Content-Type: application/json" --data "$META")
ID=$(printf "%s" "$C" | python3 -c "import sys,json;print(json.load(sys.stdin)[\"buildId\"])")
curl -sS --http1.1 --retry 3 -X PUT "$BASE/api/apps/$SPEEDFLIGHT_SECRET/$B/builds/$ID/app.ipa" --data-binary @build/export/Calternal.ipa >/dev/null
printf "%s" "$C" | python3 -c "import sys,json;print(json.load(sys.stdin)[\"pageUrl\"])"
