#!/bin/sh
# On the Mac Studio, in experiments/ios-keyboard: builds a Release archive for iPhone without
# signing (build/KBSpike.xcarchive). Bundle identifiers, App Group and team come from
# Local.xcconfig (untracked). Usage: ./archive.sh [build-number]
set -eu
cd "$(dirname "$0")"
[ -f Local.xcconfig ] || { echo "No Local.xcconfig: copy Local.xcconfig.example and fill it in." >&2; exit 1; }
N=${1:-$(date +%Y%m%d%H%M)}
rm -rf build/KBSpike.xcarchive
xcodebuild -project KeyboardSpike.xcodeproj -scheme KBSpike -configuration Release \
  -destination "generic/platform=iOS" -archivePath build/KBSpike.xcarchive -derivedDataPath build/release \
  CODE_SIGNING_ALLOWED=NO CURRENT_PROJECT_VERSION="$N" archive -quiet
echo "Archive ready: build/KBSpike.xcarchive (build $N)"
