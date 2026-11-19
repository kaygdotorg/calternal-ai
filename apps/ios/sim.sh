#!/bin/sh
# Runs the shell in this lane's own iPhone Simulator ("calternal-ios", created once with
# `xcrun simctl create`; never touches another device). Run on the Mac Studio, in apps/ios.
#   ./sim.sh build                    build for the Simulator (unsigned) into build-sim/
#   ./sim.sh run [start-url|demo] [extra launch args...]
#                                     (re)install and launch; the start address defaults to the
#                                     production site; extra args are passed to the app, e.g.
#                                     -CalternalProbe YES -CalternalNativeBar YES
#   ./sim.sh shot NAME                screenshot to build/shots/NAME.png
#   ./sim.sh logs                     the last minute of the app's log
# Screenshots are evidence for the lane's Forgejo issue, never for the repository (build/ is ignored).
set -eu
cd "$(dirname "$0")"
NAME=calternal-ios
UDID=$(xcrun simctl list devices | sed -n "s/.*$NAME (\([0-9A-F-]*\)).*/\1/p" | head -1)
[ -n "$UDID" ] || { echo "no Simulator named $NAME: xcrun simctl create $NAME <device type> <runtime>" >&2; exit 1; }
APP=build-sim/Build/Products/Debug-iphonesimulator/Calternal.app
case "${1:-}" in
  build)
    xcodebuild -project Calternal.xcodeproj -scheme Calternal -configuration Debug -destination "id=$UDID" \
      -derivedDataPath build-sim CODE_SIGNING_ALLOWED=NO build -quiet ;;
  run)
    shift; START=${1:-}; [ $# -gt 0 ] && shift
    xcrun simctl boot "$UDID" 2>/dev/null || true
    xcrun simctl bootstatus "$UDID" >/dev/null
    xcrun simctl install "$UDID" "$APP"
    xcrun simctl terminate "$UDID" cloud.calternal.app 2>/dev/null || true
    if [ -n "$START" ]; then
      xcrun simctl launch "$UDID" cloud.calternal.app -CalternalStartURL "$START" "$@"
    else
      xcrun simctl launch "$UDID" cloud.calternal.app "$@"
    fi ;;
  shot)
    mkdir -p build/shots; xcrun simctl io "$UDID" screenshot "build/shots/${2:?name}.png" 2>&1 | tail -1; echo "build/shots/$2.png" ;;
  logs)
    xcrun simctl spawn "$UDID" log show --last 1m --predicate 'subsystem == "cloud.calternal.app"' --style compact ;;
  udid) echo "$UDID" ;;
  *) sed -n 2,12p "$0" ;;
esac
