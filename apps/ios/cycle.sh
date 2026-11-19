#!/bin/sh
# Development loop on the Mac: rebuild, relaunch in the lane's Simulator with the probe on, take a screenshot.
# Usage: ./cycle.sh <start-url|demo> [seconds-to-wait] [screenshot-name]   (output: probe lines in /tmp/calternal-ios-probe.log)
cd "$(dirname "$0")"
./sim.sh build 2>&1 | grep -E "error" | head -20
U=$(./sim.sh udid)
xcrun simctl terminate "$U" cloud.calternal.app 2>/dev/null
xcrun simctl install "$U" build-sim/Build/Products/Debug-iphonesimulator/Calternal.app
: > /tmp/calternal-ios-probe.log
(xcrun simctl launch --console "$U" cloud.calternal.app -CalternalStartURL "$1" -CalternalProbe YES $EXTRA > /tmp/calternal-ios-probe.log 2>&1 &)
sleep "${2:-7}"
./sim.sh shot "${3:-cycle}" >/dev/null
grep PROBE /tmp/calternal-ios-probe.log | tail -1 | sed 's/.*underPage":"\([^"]*\)".*/underPage=\1/'
