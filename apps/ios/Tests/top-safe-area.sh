#!/bin/sh
# Simulator test: the native chrome must not paint a band in the top safe area, closed or with the sidebar open. Run on the Mac, in apps/ios, after ./sim.sh build.
# Uses the bundled chrome-demo page; the safe-area height is read from the device (62 pt x 3 = 186 px on the iPhone 18 Pro Max).
set -eu
cd "$(dirname "$0")/.."
. ./common.sh
BID=$(bundle_id)
U=$(./sim.sh udid)
SAFE=${SAFE_TOP_PX:-186}
xcrun simctl install "$U" build-sim/Build/Products/Debug-iphonesimulator/Calternal.app
fail=0
for state in closed open; do
  EVAL=""; [ "$state" = open ] && EVAL="store.open=true; publish();"
  xcrun simctl terminate "$U" $BID 2>/dev/null || true
  xcrun simctl launch "$U" $BID -CalternalStartURL chrome-demo -CalternalNativeChrome YES -CalternalProbe YES -CalternalProbeEval "$EVAL" >/dev/null
  sleep 8
  ./sim.sh shot "topband-$state" >/dev/null
  echo "== $state"; swift Tests/top-safe-area.swift "build/shots/topband-$state.png" "$SAFE" || fail=1
done
exit $fail
