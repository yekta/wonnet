#!/usr/bin/env bash
# Builds the core and a release APK at build/Wonnet.apk, signed with the key the variables
# ANDROID_KEYSTORE_PATH, ANDROID_KEYSTORE_PASSWORD, ANDROID_KEY_ALIAS and ANDROID_KEY_PASSWORD
# give, else with the debug key. With app/google-services.json in place, the app gets push.
set -euo pipefail
cd "$(dirname "$0")/.."
./gradlew --no-daemon :app:assembleRelease
mkdir -p build
cp app/build/outputs/apk/release/app-release.apk build/Wonnet.apk
echo "Built $(pwd)/build/Wonnet.apk"
