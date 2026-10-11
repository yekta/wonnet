#!/usr/bin/env bash
# Archives the app for devices and uploads it to TestFlight, with the version in Cargo.toml and a
# build number that counts up by the minute. It is signed with the Apple Distribution certificate
# in the keychain and the given profiles, so no machine makes a certificate of its own.
#
#   APPLE_TEAM_ID=… IOS_BUNDLE_ID=… IOS_PROVISIONING_PROFILE=… IOS_EXTENSION_PROVISIONING_PROFILE=… \
#       APPLE_API_KEY=… APPLE_API_KEY_ID=… APPLE_API_ISSUER_ID=… scripts/testflight.sh
#
# IOS_BUNDLE_ID is the bundle identifier of the app's record in App Store Connect,
# IOS_PROVISIONING_PROFILE its App Store profile in base64, and IOS_EXTENSION_PROVISIONING_PROFILE
# that of its notification extension, `$IOS_BUNDLE_ID.notifications`. Both have the app group
# `group.$IOS_BUNDLE_ID`, and the app's has push notifications. APPLE_API_KEY is an App Store
# Connect API key (the .p8's text) that may upload the app.
set -euo pipefail
cd "$(dirname "$0")/.."

ROOT="$(cd ../.. && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
BUILD="$(date -u +%Y%m%d%H%M)"
KEY="$(mktemp -d)/AuthKey_$APPLE_API_KEY_ID.p8"
echo "$APPLE_API_KEY" > "$KEY"

PROFILES="$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"
mkdir -p "$PROFILES"
# Installs a profile given in base64 and prints its name.
install_profile() {
    local profile
    profile="$(mktemp -d)/profile.mobileprovision"
    echo "$1" | base64 --decode > "$profile"
    cp "$profile" "$PROFILES/$(security cms -D -i "$profile" | plutil -extract UUID raw -o - -).mobileprovision"
    security cms -D -i "$profile" | plutil -extract Name raw -o - -
}
PROFILE_NAME="$(install_profile "$IOS_PROVISIONING_PROFILE")"
EXTENSION_PROFILE_NAME="$(install_profile "$IOS_EXTENSION_PROVISIONING_PROFILE")"

xcodegen generate
xcodebuild archive -project Mail.xcodeproj -scheme Mail -configuration Release \
    -destination 'generic/platform=iOS' -archivePath build/Wonnet.xcarchive -derivedDataPath build/derived \
    DEVELOPMENT_TEAM="$APPLE_TEAM_ID" APP_BUNDLE_ID="$IOS_BUNDLE_ID" APS_ENVIRONMENT=production \
    MARKETING_VERSION="$VERSION" CURRENT_PROJECT_VERSION="$BUILD" \
    CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY="Apple Distribution" IOS_PROFILE_NAME="$PROFILE_NAME" \
    IOS_EXTENSION_PROFILE_NAME="$EXTENSION_PROFILE_NAME"

cat > build/export.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key><string>app-store-connect</string>
    <key>destination</key><string>upload</string>
    <key>teamID</key><string>$APPLE_TEAM_ID</string>
    <key>signingStyle</key><string>manual</string>
    <key>signingCertificate</key><string>Apple Distribution</string>
    <key>provisioningProfiles</key><dict>
        <key>$IOS_BUNDLE_ID</key><string>$PROFILE_NAME</string>
        <key>$IOS_BUNDLE_ID.notifications</key><string>$EXTENSION_PROFILE_NAME</string>
    </dict>
</dict>
</plist>
PLIST
xcodebuild -exportArchive -archivePath build/Wonnet.xcarchive -exportOptionsPlist build/export.plist \
    -exportPath build/export -authenticationKeyPath "$KEY" -authenticationKeyID "$APPLE_API_KEY_ID" \
    -authenticationKeyIssuerID "$APPLE_API_ISSUER_ID"
echo "✓ Uploaded Wonnet $VERSION ($BUILD) to TestFlight"
