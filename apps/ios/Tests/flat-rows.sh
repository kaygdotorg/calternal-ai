#!/bin/sh
# Simulator test: rows of a windowed section (the Notes app) have no default black background (see flat-rows.swift). Run on the Mac, in apps/ios, after ./sim.sh build.
set -eu
cd "$(dirname "$0")/.."
. ./common.sh
BID=$(bundle_id)
U=$(./sim.sh udid)
xcrun simctl install "$U" build-sim/Build/Products/Debug-iphonesimulator/Calternal.app
xcrun simctl terminate "$U" "$BID" 2>/dev/null || true
xcrun simctl launch "$U" "$BID" -CalternalStartURL "chrome-demo#notes" -CalternalNativeChrome YES -CalternalProbe YES -CalternalProbeEval "store.open=true; publish();" >/dev/null
sleep 9
./sim.sh shot flatrows >/dev/null
swift Tests/flat-rows.swift build/shots/flatrows.png
