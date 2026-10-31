#!/usr/bin/env bash
# Builds the desktop app for Windows, portable, as apps/gpui/build/Wonnet-windows-x86_64.zip
# holding Wonnet.exe (its icon and version info embedded by build.rs). Runs in Git Bash on a
# Windows machine with Rust's MSVC toolchain, the version already written into the root
# Cargo.toml (scripts/release-version.sh).
#
#   scripts/build-windows.sh
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' ../../Cargo.toml | head -1)"

echo "▸ Building Wonnet $VERSION for Windows x86_64"
scripts/patch-crates.sh
cargo build --release --target x86_64-pc-windows-msvc

STAGE="build/windows"
OUT="build/Wonnet-windows-x86_64.zip"
rm -rf "$STAGE" "$OUT"
mkdir -p "$STAGE"
cp target/x86_64-pc-windows-msvc/release/wonnet.exe "$STAGE/Wonnet.exe"

if command -v 7z >/dev/null 2>&1; then
    (cd "$STAGE" && 7z a -tzip "../Wonnet-windows-x86_64.zip" Wonnet.exe >/dev/null)
else
    powershell.exe -NoProfile -Command "Compress-Archive -Path '$STAGE/Wonnet.exe' -DestinationPath '$OUT' -Force"
fi
echo "✓ Built $OUT"
