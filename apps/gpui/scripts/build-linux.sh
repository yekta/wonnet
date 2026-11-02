#!/usr/bin/env bash
# Builds the desktop app for Linux as an AppImage: apps/gpui/build/Wonnet-linux-<arch>.AppImage.
# Run on a machine of that architecture, with the packages of linux-packages.txt installed and
# the version already written into the root Cargo.toml (scripts/release-version.sh).
#
#   scripts/build-linux.sh x86_64|aarch64
set -euo pipefail

ARCH="${1:?usage: build-linux.sh x86_64|aarch64}"
case "$ARCH" in
    x86_64 | aarch64) ;;
    *) echo "Unknown architecture: $ARCH" >&2; exit 1 ;;
esac
cd "$(dirname "$0")/.."
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' ../../Cargo.toml | head -1)"

echo "▸ Building Wonnet $VERSION for Linux $ARCH"
scripts/build-wpe.sh
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

echo "▸ Bundling WPE WebKit"
# WPE WebKit and what it needs come along; what every desktop has comes from the system: glibc,
# the graphics drivers, X11's core, fonts, sound and TLS (whose certificates differ by
# distribution), as linuxdeploy's excludelist has it.
SYSTEM='^(ld-linux.*|libc|libm|libdl|libpthread|librt|libresolv|libutil|libanl|libmvec|libnss_.*|libstdc\+\+|libgcc_s|libGL.*|libEGL|libOpenGL|libglapi|libgbm|libdrm.*|libvulkan|libwayland-.*|libX11|libX11-xcb|libxcb.*|libXau|libXdmcp|libasound|libjack|libpipewire-.*|libfontconfig|libfreetype|libharfbuzz|libfribidi|libthai|libexpat|libz|libuuid|libcom_err|libgpg-error|libgmp|libp11-kit|libgnutls|libICE|libSM|libusb-1\.0)\.so'
TRIPLE="$ARCH-linux-gnu"
LIB="$APPDIR/usr/lib"
HELPERS="$LIB/wpe-webkit-2.0"
MODULES="$LIB/$TRIPLE/gio/modules"
mkdir -p "$MODULES"
# WPE looks for its helpers in wpe-webkit-2.0 beside its library (patches/wpewebkit.patch).
cp -r build/wpe/lib/wpe-webkit-2.0 "$HELPERS"
# GIO's TLS, for the pictures pages load over https.
cp "/usr/lib/$TRIPLE/gio/modules/libgiognutls.so" "$MODULES/"
for file in "$APPDIR/usr/bin/wonnet" "$HELPERS"/WPE* "$HELPERS/injected-bundle"/*.so "$MODULES"/*.so; do
    ldd "$file" | awk '/=> \// { print $3 }' | while read -r library; do
        name="$(basename "$library")"
        if echo "$name" | grep -Eq "$SYSTEM" || [ -e "$LIB/$name" ]; then
            continue
        fi
        cp -L "$library" "$LIB/$name"
    done
done
patchelf --set-rpath '$ORIGIN/../lib' "$APPDIR/usr/bin/wonnet"
for library in "$LIB"/*.so*; do
    patchelf --set-rpath '$ORIGIN' "$library"
done
patchelf --set-rpath '$ORIGIN/..' "$HELPERS"/WPE*
patchelf --set-rpath '$ORIGIN/../..' "$HELPERS/injected-bundle"/*.so
patchelf --set-rpath '$ORIGIN/../../..' "$MODULES"/*.so
chmod -R a+rX "$APPDIR"

# GIO's module folder is compiled into it too, split up on x86_64, so AppRun names it. The app
# leaves it out of what it opens (platform::open).
cat > "$APPDIR/AppRun" <<APPRUN
#!/bin/sh
cd "\$(dirname "\$(readlink -f "\$0")")/usr" || exit 1
export GIO_MODULE_DIR="\$PWD/lib/$TRIPLE/gio/modules"
exec ./bin/wonnet "\$@"
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
