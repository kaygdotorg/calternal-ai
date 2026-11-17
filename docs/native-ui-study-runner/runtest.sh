#!/bin/sh
# usage: runtest.sh TestName [tag]  -> records video, runs the UI test, copies results into ../res/<tag><name>
cd ~/Developer/lanes/native-ui-study; U=$(cat udid.txt); T=$1; TAG=${2:-}
OUT=res/$TAG$T; rm -rf $OUT; mkdir -p $OUT
rm -rf "$(xcrun simctl get_app_container $U com.apple.Preferences data 2>/dev/null)/tmp/study" 2>/dev/null
xcrun simctl io $U recordVideo --codec=h264 --force $OUT/video.mov >/dev/null 2>&1 &
RP=$!; sleep 2; date +%s.%N > $OUT/video.start
cd proj; xcodebuild test-without-building -project Study.xcodeproj -scheme Host -destination "id=$U" -derivedDataPath ../dd -only-testing:Runner/Study/$T > ../$OUT/xcb.log 2>&1; tail -3 ../$OUT/xcb.log
cd ..; sleep 1; kill -INT $RP; sleep 4
find ~/Library/Developer/CoreSimulator/Devices/$U/data -type d -name study -path "*tmp*" 2>/dev/null | head -1 | while read d; do cp -R "$d"/. $OUT/; rm -rf "$d"; done
ls $OUT | tr '\n' ' '
