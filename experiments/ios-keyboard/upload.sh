#!/bin/sh
# On the Mac Studio, in experiments/ios-keyboard, AFTER ./archive.sh and only once the app
# record, bundle identifiers and App Group may be created (coordinator's go-ahead).
# Signs the archive ad hoc with the entitlements, then exports with Apple cloud signing through
# the App Store Connect key (path passed to xcodebuild, never printed) and uploads to App Store
# Connect for TestFlight. -allowProvisioningUpdates registers the two bundle identifiers and the
# App Group if they do not exist yet, and creates the distribution profiles.
set -eu
cd "$(dirname "$0")"
cfg() { sed -n "s|^[[:space:]]*$1[[:space:]]*=[[:space:]]*\([^/[:space:]]*\).*|\1|p" Local.xcconfig | head -1; }
TEAM=$(cfg DEVELOPMENT_TEAM); GROUP=$(cfg SPIKE_APP_GROUP)
[ -n "$TEAM" ] && [ -n "$GROUP" ] || { echo "Local.xcconfig needs DEVELOPMENT_TEAM and SPIKE_APP_GROUP." >&2; exit 1; }
P=$(ls ~/.config/asc/AuthKey_*.p8 | head -1)
K=$(basename "$P" .p8 | sed 's/AuthKey_//')
I=$(tr -d '[:space:]' < ~/.config/asc/issuer-id.txt)
A=build/KBSpike.xcarchive/Products/Applications/KBSpike.app
for kind in Keyboard App; do sed "s|\$(SPIKE_APP_GROUP)|$GROUP|" Config/$kind.entitlements > build/$kind.xcent; done
codesign -s - --force --entitlements build/Keyboard.xcent "$A/PlugIns/Keyboard.appex"
codesign -s - --force --entitlements build/App.xcent "$A"
cat > build/ExportOptions.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>method</key><string>app-store-connect</string>
<key>destination</key><string>upload</string>
<key>signingStyle</key><string>automatic</string>
<key>teamID</key><string>$TEAM</string>
<key>manageAppVersionAndBuildNumber</key><false/>
<key>testFlightInternalTestingOnly</key><true/>
</dict></plist>
PLIST
xcodebuild -exportArchive -archivePath build/KBSpike.xcarchive -exportPath build/export \
  -exportOptionsPlist build/ExportOptions.plist -allowProvisioningUpdates \
  -authenticationKeyID "$K" -authenticationKeyIssuerID "$I" -authenticationKeyPath "$P"
