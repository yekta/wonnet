#!/usr/bin/env bash
# Builds WPE WebKit, which paints a message's body on Linux, into build/wpe. Ubuntu packages no
# WPE, so the app brings its own. It takes an hour or more, and does nothing when build/wpe
# already holds this version with this patch and these options. Needs the packages of
# linux-packages.txt.
#
#   scripts/build-wpe.sh
set -euo pipefail

VERSION=2.54.2
SHA256=f3284e19e47fcb5ac8c63c2e1c113dbd65b2423eb8a545f5b345707829004f56

cd "$(dirname "$0")/.."
PREFIX="$PWD/build/wpe"
STAMP="$VERSION $(cat patches/wpewebkit.patch scripts/build-wpe.sh | sha256sum | cut -d' ' -f1)"
if [ "$(cat "$PREFIX/stamp" 2>/dev/null)" = "$STAMP" ]; then
    exit 0
fi

SOURCE="build/wpewebkit-$VERSION"
if [ ! -f "$SOURCE/patched" ]; then
    echo "▸ Downloading WPE WebKit $VERSION"
    archive="build/wpewebkit-$VERSION.tar.xz"
    mkdir -p build
    curl -fsSL "https://wpewebkit.org/releases/wpewebkit-$VERSION.tar.xz" -o "$archive"
    if [ "$(sha256sum "$archive" | cut -d' ' -f1)" != "$SHA256" ]; then
        echo "$archive does not match its checksum." >&2
        exit 1
    fi
    rm -rf "$SOURCE"
    tar -xf "$archive" -C build
    rm "$archive"
    patch -d "$SOURCE" -p1 < patches/wpewebkit.patch
    touch "$SOURCE/patched"
fi

echo "▸ Building WPE WebKit $VERSION"
# Built with bubblewrap, the 2.0 API sandboxes every page, and a system without bwrap shows none.
cmake -S "$SOURCE" -B "$SOURCE/build" -G Ninja \
    -DPORT=WPE \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=lld \
    -DCMAKE_SHARED_LINKER_FLAGS=-fuse-ld=lld \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_INSTALL_LIBDIR=lib \
    -DCMAKE_INSTALL_RPATH='$ORIGIN:$ORIGIN/..:$ORIGIN/../..' \
    -DLIBEXEC_INSTALL_DIR="$PREFIX/lib/wpe-webkit-2.0" \
    -DENABLE_WPE_PLATFORM=ON \
    -DENABLE_WPE_PLATFORM_DRM=OFF \
    -DENABLE_WPE_PLATFORM_HEADLESS=OFF \
    -DENABLE_WPE_PLATFORM_WAYLAND=OFF \
    -DENABLE_WPE_LEGACY_API=OFF \
    -DENABLE_WPE_QT_API=OFF \
    -DENABLE_BUBBLEWRAP_SANDBOX=OFF \
    -DENABLE_DOCUMENTATION=OFF \
    -DENABLE_INTROSPECTION=OFF \
    -DENABLE_JOURNALD_LOG=OFF \
    -DENABLE_GAMEPAD=OFF \
    -DENABLE_GPU_PROCESS=OFF \
    -DENABLE_PDFJS=OFF \
    -DENABLE_SPELLCHECK=OFF \
    -DENABLE_WEBDRIVER=OFF \
    -DENABLE_XSLT=OFF \
    -DUSE_GSTREAMER=OFF \
    -DUSE_GBM=OFF \
    -DUSE_LIBDRM=OFF \
    -DUSE_ATK=OFF \
    -DUSE_AVIF=OFF \
    -DUSE_JPEGXL=OFF \
    -DUSE_LCMS=OFF \
    -DUSE_LIBBACKTRACE=OFF \
    -DUSE_LIBHYPHEN=OFF \
    -DUSE_SYSPROF_CAPTURE=OFF
cmake --build "$SOURCE/build"
rm -rf "$PREFIX"
cmake --install "$SOURCE/build" --strip
echo "$STAMP" > "$PREFIX/stamp"
rm -rf "$SOURCE"
echo "✓ Built WPE WebKit $VERSION into $PREFIX"
