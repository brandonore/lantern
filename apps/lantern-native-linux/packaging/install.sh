#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BASH_APP_ROOT="${ROOT_DIR}/apps/lantern-native-linux"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
METAINFO_DIR="${HOME}/.local/share/metainfo"
ICON_DIR="${HOME}/.local/share/icons/hicolor"
BIN_PATH="${BIN_DIR}/lantern-native-linux"
LAUNCHER_PATH="${BIN_DIR}/lantern"
DESKTOP_PATH="${APP_DIR}/sh.lantern.NativeLinux.desktop"
DESKTOP_TEMPLATE="${ROOT_DIR}/apps/lantern-native-linux/packaging/sh.lantern.NativeLinux.desktop.in"
METAINFO_SOURCE="${ROOT_DIR}/apps/lantern-native-linux/packaging/sh.lantern.NativeLinux.metainfo.xml"
METAINFO_PATH="${METAINFO_DIR}/sh.lantern.NativeLinux.metainfo.xml"
ICON_SOURCE_DIR="${ROOT_DIR}/src-tauri/icons"
ICON_32_PATH="${ICON_DIR}/32x32/apps/sh.lantern.NativeLinux.png"
ICON_128_PATH="${ICON_DIR}/128x128/apps/sh.lantern.NativeLinux.png"
ICON_256_PATH="${ICON_DIR}/256x256/apps/sh.lantern.NativeLinux.png"

bash "${BASH_APP_ROOT}/packaging/check-deps.sh"

mkdir -p \
  "${BIN_DIR}" \
  "${APP_DIR}" \
  "${METAINFO_DIR}" \
  "${ICON_DIR}/32x32/apps" \
  "${ICON_DIR}/128x128/apps" \
  "${ICON_DIR}/256x256/apps"

cargo build --release -p lantern-native-linux --manifest-path "${ROOT_DIR}/Cargo.toml"
install -m 0755 "${ROOT_DIR}/target/release/lantern-native-linux" "${BIN_PATH}"
ln -sf "${BIN_PATH}" "${LAUNCHER_PATH}"
sed "s|__BIN_PATH__|${LAUNCHER_PATH}|g" "${DESKTOP_TEMPLATE}" > "${DESKTOP_PATH}"
install -m 0644 "${METAINFO_SOURCE}" "${METAINFO_PATH}"
install -m 0644 "${ICON_SOURCE_DIR}/32x32.png" "${ICON_32_PATH}"
install -m 0644 "${ICON_SOURCE_DIR}/128x128.png" "${ICON_128_PATH}"
install -m 0644 "${ICON_SOURCE_DIR}/128x128@2x.png" "${ICON_256_PATH}"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${APP_DIR}" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
fi

if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli validate "${METAINFO_PATH}" >/dev/null 2>&1 || true
fi

printf 'Installed Lantern Native to %s\n' "${BIN_PATH}"
printf 'Linux launcher linked at %s\n' "${LAUNCHER_PATH}"
printf 'Desktop entry written to %s\n' "${DESKTOP_PATH}"
printf 'App metadata written to %s\n' "${METAINFO_PATH}"
printf 'Icons written to %s, %s, and %s\n' "${ICON_32_PATH}" "${ICON_128_PATH}" "${ICON_256_PATH}"
