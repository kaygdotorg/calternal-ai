#!/bin/sh
# On the Mac Studio, in experiments/ios-keyboard: archives, signs for the registered iPhone
# (release-testing export through Apple cloud signing with the App Store Connect key, whose path
# is passed to xcodebuild and never printed) and installs on the phone over the network.
# The export registers the two bundle identifiers if they are missing.
# Usage: ./install-on-phone.sh [device-udid]   (default: PHONE_UDID in Local.xcconfig)
set -eu
cd "$(dirname "$0")"
cfg() { sed -n "s|^[[:space:]]*$1[[:space:]]*=[[:space:]]*\([^/[:space:]]*\).*|\1|p" Local.xcconfig | head -1; }
TEAM=$(cfg DEVELOPMENT_TEAM); BID=$(cfg SPIKE_BUNDLE_ID)
DEV=${1:-$(cfg PHONE_UDID)}
[ -n "$TEAM" ] && [ -n "$BID" ] && [ -n "$DEV" ] || { echo "Local.xcconfig needs DEVELOPMENT_TEAM, SPIKE_BUNDLE_ID and PHONE_UDID." >&2; exit 1; }
./archive.sh "$(date +%Y%m%d%H%M)"
P=$(ls ~/.config/asc/AuthKey_*.p8 | head -1)
K=$(basename "$P" .p8 | sed 's/AuthKey_//')
I=$(tr -d '[:space:]' < ~/.config/asc/issuer-id.txt)
A=build/KBSpike.xcarchive/Products/Applications/KBSpike.app
for kind in Keyboard App; do sed "s|\$(SPIKE_KEYCHAIN_GROUP)|$TEAM.$BID.shared|" Config/$kind.entitlements > build/$kind.xcent; done
codesign -s - --force --entitlements build/Keyboard.xcent "$A/PlugIns/Keyboard.appex"
codesign -s - --force --entitlements build/App.xcent "$A"
cat > build/ExportOptions-device.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>method</key><string>release-testing</string>
<key>signingStyle</key><string>automatic</string>
<key>teamID</key><string>$TEAM</string>
<key>thinning</key><string>&lt;none&gt;</string>
</dict></plist>
PLIST
rm -rf build/export build/ipa
xcodebuild -exportArchive -archivePath build/KBSpike.xcarchive -exportPath build/export \
  -exportOptionsPlist build/ExportOptions-device.plist -allowProvisioningUpdates \
  -authenticationKeyID "$K" -authenticationKeyIssuerID "$I" -authenticationKeyPath "$P" -quiet
mkdir build/ipa && unzip -q build/export/KBSpike.ipa -d build/ipa
xcrun devicectl device install app --device "$DEV" build/ipa/Payload/KBSpike.app | grep -iE "installed|error" || true
xcrun devicectl device process launch --device "$DEV" "$BID" 2>&1 | grep -iE "launched|error" || true
