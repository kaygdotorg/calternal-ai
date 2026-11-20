#!/bin/sh
# Builds and runs the SwiftUI spike in its own Simulator ("drawing-spike", created on first use). Run on the Mac Studio in
# experiments/drawing-spike/apple. Unsigned Simulator build; the bundle identifier is a local placeholder passed at build time.
#   ./run.sh build | ./run.sh run [launch args…] | ./run.sh shot NAME | ./run.sh dark|light
set -eu
cd "$(dirname "$0")"
NAME=drawing-spike; BID=${SPIKE_BUNDLE_ID:-local.spike.drawing}
UDID=$(xcrun simctl list devices | sed -n "s/^ *$NAME (\([0-9A-F-]*\)).*/\1/p" | head -1)
if [ -z "$UDID" ]; then UDID=$(xcrun simctl create "$NAME" "iPhone 17" com.apple.CoreSimulator.SimRuntime.iOS-27-0); fi
case "${1:-}" in
  build)
    cp ../data/declarations.json ../data/fixtures.json DrawingSpike/
    xcodebuild -project DrawingSpike.xcodeproj -scheme DrawingSpike -configuration Debug -destination "id=$UDID" -derivedDataPath build \
      CODE_SIGNING_ALLOWED=NO PRODUCT_BUNDLE_IDENTIFIER="$BID" build -quiet ;;
  run)
    shift
    xcrun simctl boot "$UDID" 2>/dev/null || true; xcrun simctl bootstatus "$UDID" >/dev/null
    xcrun simctl install "$UDID" build/Build/Products/Debug-iphonesimulator/DrawingSpike.app
    xcrun simctl terminate "$UDID" "$BID" 2>/dev/null || true
    xcrun simctl launch "$UDID" "$BID" "$@" ;;
  shot) mkdir -p shots; xcrun simctl io "$UDID" screenshot "shots/$2.png" >/dev/null 2>&1; echo "shots/$2.png" ;;
  dark|light) xcrun simctl ui "$UDID" appearance "$1" ;;
  *) sed -n 2,5p "$0" ;;
esac
