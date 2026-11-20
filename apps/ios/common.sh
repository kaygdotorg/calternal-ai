# Shared by install-on-phone.sh and share-build.sh (sourced, not run).
# Signing needs no login keychain: the archive is built unsigned, signed ad hoc with the
# entitlements, then exported with Apple cloud signing through the App Store Connect key in
# ~/.config/asc. The key's path and ids are passed to xcodebuild and never printed or copied.
# Local values (team, phone) come from Local.xcconfig, which is untracked.

# cfg KEY: the value of KEY in Local.xcconfig, or nothing.
cfg() { sed -n "s|^[[:space:]]*$1[[:space:]]*=[[:space:]]*\([^/[:space:]]*\).*|\1|p" Local.xcconfig 2>/dev/null | head -1; }

# need KEY: the value of KEY, or a plain-language stop.
need() {
  v=$(cfg "$1")
  [ -n "$v" ] || { echo "Local.xcconfig has no $1: copy Local.xcconfig.example to Local.xcconfig and fill it in." >&2; exit 1; }
  printf '%s' "$v"
}

# bundle_id: the app's bundle identifier, from $CALTERNAL_BUNDLE_ID or PRODUCT_BUNDLE_IDENTIFIER in Local.xcconfig (never written in a tracked file).
bundle_id() {
  v=${CALTERNAL_BUNDLE_ID:-$(cfg PRODUCT_BUNDLE_IDENTIFIER)}
  printf '%s' "${v:-com.example.calternal}"   # the placeholder Shell.xcconfig builds with when Local.xcconfig is absent
}

# archive_and_export [BUILD_NUMBER]: builds Release for iOS and leaves build/export/Calternal.ipa.
archive_and_export() {
  team=$(need DEVELOPMENT_TEAM)
  P=$(ls ~/.config/asc/AuthKey_*.p8 | head -1)
  K=$(basename "$P" .p8 | sed 's/AuthKey_//')
  I=$(tr -d '[:space:]' < ~/.config/asc/issuer-id.txt)
  mkdir -p build
  cat > build/ExportOptions.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>method</key><string>release-testing</string>
<key>signingStyle</key><string>automatic</string>
<key>teamID</key><string>$team</string>
<key>thinning</key><string>&lt;none&gt;</string>
</dict></plist>
PLIST
  rm -rf build/App.xcarchive build/export
  xcodebuild -project Calternal.xcodeproj -scheme Calternal -configuration Release \
    -destination "generic/platform=iOS" -archivePath build/App.xcarchive \
    CODE_SIGNING_ALLOWED=NO ${1:+CURRENT_PROJECT_VERSION="$1"} archive -quiet
  A=build/App.xcarchive/Products/Applications/Calternal.app
  find "$A" \( -name "*.dylib" -o -name "*.framework" \) -exec codesign -s - --force {} \; 2>/dev/null
  codesign -s - --force --entitlements Calternal/Calternal.entitlements "$A" 2>/dev/null
  xcodebuild -exportArchive -archivePath build/App.xcarchive -exportPath build/export \
    -exportOptionsPlist build/ExportOptions.plist -allowProvisioningUpdates \
    -authenticationKeyID "$K" -authenticationKeyIssuerID "$I" -authenticationKeyPath "$P" -quiet
}
