#!/usr/bin/env bash
set -euo pipefail

BIN_PATH="${HOME}/.local/bin/lantern-native-linux"
LAUNCHER_PATH="${HOME}/.local/bin/lantern"
DESKTOP_PATH="${HOME}/.local/share/applications/sh.lantern.NativeLinux.desktop"
METAINFO_PATH="${HOME}/.local/share/metainfo/sh.lantern.NativeLinux.metainfo.xml"
ICON_32_PATH="${HOME}/.local/share/icons/hicolor/32x32/apps/sh.lantern.NativeLinux.png"
ICON_128_PATH="${HOME}/.local/share/icons/hicolor/128x128/apps/sh.lantern.NativeLinux.png"
ICON_256_PATH="${HOME}/.local/share/icons/hicolor/256x256/apps/sh.lantern.NativeLinux.png"

rm -f \
  "${BIN_PATH}" \
  "${LAUNCHER_PATH}" \
  "${DESKTOP_PATH}" \
  "${METAINFO_PATH}" \
  "${ICON_32_PATH}" \
  "${ICON_128_PATH}" \
  "${ICON_256_PATH}"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
fi

printf 'Removed Lantern Native from %s, %s, %s, %s, %s, %s, and %s\n' \
  "${BIN_PATH}" \
  "${LAUNCHER_PATH}" \
  "${DESKTOP_PATH}" \
  "${METAINFO_PATH}" \
  "${ICON_32_PATH}" \
  "${ICON_128_PATH}" \
  "${ICON_256_PATH}"
