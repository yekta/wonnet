#!/usr/bin/env bash
# Builds the Rust core for Android as libwonnet_core.so, into app/build/jniLibs/<abi>, which
# Gradle packs into the app. Gradle runs it before it builds the app.
#
#   WONNET_ABIS=arm64-v8a scripts/build-core.sh
#
# WONNET_ABIS lists the ABIs, arm64-v8a and x86_64 without it. Needs cargo-ndk, an Android NDK
# (ANDROID_NDK_HOME, else the newest under ANDROID_HOME) and Rust's Android targets.
set -euo pipefail
cd "$(dirname "$0")/.."
ANDROID="$(pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)"
    export ANDROID_NDK_HOME
fi
# A target folder of its own, so this build doesn't wait on the others for Cargo's lock.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ANDROID/../../target/android}"

ABIS="$(echo "${WONNET_ABIS:-arm64-v8a,x86_64}" | tr ',' ' ')"
targets=()
for abi in $ABIS; do targets+=(-t "$abi"); done
cd ../..
cargo ndk "${targets[@]}" --platform 29 build --release -p mail-core-android

# Only the core: cargo-ndk would copy every library the build made.
rm -rf "$ANDROID/app/build/jniLibs"
for abi in $ABIS; do
    case "$abi" in
        arm64-v8a) triple=aarch64-linux-android ;;
        x86_64) triple=x86_64-linux-android ;;
        *) echo "Unknown ABI $abi" >&2; exit 1 ;;
    esac
    mkdir -p "$ANDROID/app/build/jniLibs/$abi"
    cp "$CARGO_TARGET_DIR/$triple/release/libwonnet_core.so" "$ANDROID/app/build/jniLibs/$abi/"
done
