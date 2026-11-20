#!/bin/sh
# On the Mac Studio, in experiments/ios-keyboard: build for this lane's Simulator, install,
# launch with the test field focused, and take a screenshot to build/shots/<name>.png.
# Usage: ./sim.sh [name]
set -eu
cd "$(dirname "$0")"
U=$(cat build/sim-udid)
xcodebuild -project KeyboardSpike.xcodeproj -scheme KBSpike -configuration Debug \
  -destination "generic/platform=iOS Simulator" -derivedDataPath build/sim build -quiet
xcrun simctl install "$U" build/sim/Build/Products/Debug-iphonesimulator/KBSpike.app
BID=$(/usr/libexec/PlistBuddy -c "Print CFBundleIdentifier" build/sim/Build/Products/Debug-iphonesimulator/KBSpike.app/Info.plist)
xcrun simctl terminate "$U" "$BID" 2>/dev/null || true
xcrun simctl launch "$U" "$BID" -SpikeFocus YES >/dev/null
sleep 5
mkdir -p build/shots
xcrun simctl io "$U" screenshot "build/shots/${1:-now}.png" 2>/dev/null
