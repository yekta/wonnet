#!/usr/bin/env bash
# Builds the desktop app for Linux as an AppImage: apps/gpui/build/Wonnet-linux-x86_64.AppImage.
# Run on x86_64 with the packages of linux-packages.txt installed (Arch) and the version already
# written into the root Cargo.toml (scripts/release-version.sh). The AppImage takes WPE WebKit
# from the system.
set -euo pipefail

ARCH=x86_64
cd "$(dirname "$0")/.."
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' ../../Cargo.toml | head -1)"

echo "▸ Building Wonnet $VERSION for Linux $ARCH"
scripts/patch-crates.sh
cargo build --release

APPDIR="build/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp "${CARGO_TARGET_DIR:-target}/release/wonnet" "$APPDIR/usr/bin/wonnet"
cp assets/icon/wonnet.png "$APPDIR/wonnet.png"
cp assets/icon/wonnet.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/wonnet.png"
cat > "$APPDIR/wonnet.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Wonnet
Comment=Email that feels instant
Exec=wonnet %u
Icon=wonnet
Terminal=false
Categories=Network;Email;
MimeType=x-scheme-handler/wonnet;
StartupWMClass=wonnet
X-AppImage-Version=$VERSION
DESKTOP
cp "$APPDIR/wonnet.desktop" "$APPDIR/usr/share/applications/wonnet.desktop"

cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
exec "$(dirname "$(readlink -f "$0")")/usr/bin/wonnet" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

TOOL="build/appimagetool-$ARCH.AppImage"
if [ ! -x "$TOOL" ]; then
    echo "▸ Downloading appimagetool"
    curl -fsSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
    chmod +x "$TOOL"
fi

OUT="build/Wonnet-linux-$ARCH.AppImage"
rm -f "$OUT"
# Extracted rather than mounted, so the build needs no FUSE.
ARCH="$ARCH" "$TOOL" --appimage-extract-and-run --no-appstream "$APPDIR" "$OUT"
echo "✓ Built $OUT"
