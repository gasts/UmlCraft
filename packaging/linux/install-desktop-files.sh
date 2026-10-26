#!/usr/bin/env bash
# Installs the binary and the desktop integration (launcher, icons, AppStream metadata, the
# .ucraft file type) below a prefix, e.g. `install-desktop-files.sh umlcraft dist/AppDir/usr`.
# Every Linux package is built from this tree. Needs rsvg-convert for the PNG icons.
set -euo pipefail

binary=$1
prefix=$2
here=$(dirname "$0")
id=com.umlcraft.app

install -Dm755 "$binary" "$prefix/bin/umlcraft"
install -Dm644 "$here/$id.desktop" "$prefix/share/applications/$id.desktop"
install -Dm644 "$here/$id.metainfo.xml" "$prefix/share/metainfo/$id.metainfo.xml"
install -Dm644 "$here/$id.mime.xml" "$prefix/share/mime/packages/$id.xml"
install -Dm644 "$here/$id.svg" "$prefix/share/icons/hicolor/scalable/apps/$id.svg"
for size in 128 256; do
    mkdir -p "$prefix/share/icons/hicolor/${size}x$size/apps"
    rsvg-convert -w "$size" -h "$size" "$here/$id.svg" \
        -o "$prefix/share/icons/hicolor/${size}x$size/apps/$id.png"
done
