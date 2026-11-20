#!/bin/sh
# Simulator test: chrome controls are neutral (see neutral-chrome.swift). Run on the Mac, in apps/ios, after ./sim.sh build.
set -eu
cd "$(dirname "$0")/.."
. ./common.sh
BID=$(bundle_id)
U=$(./sim.sh udid)
xcrun simctl install "$U" build-sim/Build/Products/Debug-iphonesimulator/Calternal.app
xcrun simctl terminate "$U" "$BID" 2>/dev/null || true
xcrun simctl launch "$U" "$BID" -CalternalStartURL chrome-demo -CalternalNativeChrome YES -CalternalProbe YES -CalternalProbeEval "store.open=true; publish();" >/dev/null
sleep 8
./sim.sh shot neutral >/dev/null
swift Tests/neutral-chrome.swift build/shots/neutral.png
