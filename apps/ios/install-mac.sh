#!/bin/sh
# Builds the Mac app signed through Apple cloud signing (no login keychain), so it can carry the associated-domains entitlement that passkeys in the web card
# need, then copies it to ~/Applications/Calternal.app and opens it. The App Store Connect key in ~/.config/asc is passed to xcodebuild and never printed.
# Usage: ./install-mac.sh [developer-id|development]   (default developer-id: runs on any Mac without registering it)
set -eu
cd "$(dirname "$0")"
. ./common.sh
METHOD=${1:-developer-id}
team=$(need DEVELOPMENT_TEAM)
P=$(ls ~/.config/asc/AuthKey_*.p8 | head -1); K=$(basename "$P" .p8 | sed 's/AuthKey_//'); I=$(tr -d '[:space:]' < ~/.config/asc/issuer-id.txt)
mkdir -p build
cat > build/ExportOptionsMac.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>method</key><string>$METHOD</string>
<key>signingStyle</key><string>automatic</string>
<key>teamID</key><string>$team</string>
</dict></plist>
PLIST
rm -rf build/Mac.xcarchive build/export-mac
xcodebuild -project Calternal.xcodeproj -scheme CalternalMac -configuration Release -destination "generic/platform=macOS" -archivePath build/Mac.xcarchive \
  CODE_SIGNING_ALLOWED=NO CODE_SIGN_ENTITLEMENTS=CalternalMac.passkeys.entitlements archive -quiet
A=build/Mac.xcarchive/Products/Applications/Calternal.app
codesign -s - --force --deep --entitlements CalternalMac.passkeys.entitlements "$A" 2>/dev/null
xcodebuild -exportArchive -archivePath build/Mac.xcarchive -exportPath build/export-mac -exportOptionsPlist build/ExportOptionsMac.plist -allowProvisioningUpdates \
  -authenticationKeyID "$K" -authenticationKeyIssuerID "$I" -authenticationKeyPath "$P" -quiet
mkdir -p ~/Applications && rm -rf ~/Applications/Calternal.app && cp -R build/export-mac/Calternal.app ~/Applications/Calternal.app
open ~/Applications/Calternal.app
