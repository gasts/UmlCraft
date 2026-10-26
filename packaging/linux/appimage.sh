#!/usr/bin/env bash
# Builds the AppImage, e.g. `appimage.sh umlcraft dist/UmlCraft-0.5.0-linux-x64.AppImage`.
# Needs rsvg-convert, appimagetool on the PATH and its runtime file in APPIMAGE_RUNTIME.
#
# Only the binary goes in: the libraries it needs (xcb, FreeType, xkbcommon, Vulkan, Wayland,
# Fontconfig) are part of every graphical desktop, and bundled graphics libraries would clash
# with the system's drivers.
set -euo pipefail

binary=$1
output=$2
here=$(dirname "$0")
id=com.umlcraft.app
appdir="$(dirname "$output")/UmlCraft.AppDir"

rm -rf "$appdir"
bash "$here/install-desktop-files.sh" "$binary" "$appdir/usr"
ln -s usr/bin/umlcraft "$appdir/AppRun"
ln -s "usr/share/applications/$id.desktop" "$appdir/$id.desktop"
ln -s "usr/share/icons/hicolor/256x256/apps/$id.png" "$appdir/$id.png"
ln -s "$id.png" "$appdir/.DirIcon"
# appimagetool looks for the metadata under its older name.
ln -s "$id.metainfo.xml" "$appdir/usr/share/metainfo/$id.appdata.xml"
appimagetool --runtime-file "$APPIMAGE_RUNTIME" "$appdir" "$output"
