#!/bin/sh
# Builds the shell without the login keychain and installs it on a paired iPhone.
# Usage: ./install-on-phone.sh [device-udid]   (default: PHONE_UDID in Local.xcconfig)
set -eu
cd "$(dirname "$0")"
. ./common.sh
BID=$(bundle_id)
DEV=${1:-$(need PHONE_UDID)}
archive_and_export
rm -rf build/ipa && mkdir build/ipa && unzip -q build/export/Calternal.ipa -d build/ipa
xcrun devicectl device install app --device "$DEV" build/ipa/Payload/Calternal.app | grep -iE "installed|error"
xcrun devicectl device process launch --device "$DEV" "$BID" 2>&1 | grep -iE "launched|error" || true
