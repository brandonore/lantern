#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP_ROOT="${ROOT_DIR}/apps/lantern-native-linux"
DIST_DIR="${APP_ROOT}/dist"
ARCH="$(uname -m)"
PACKAGE_NAME="lantern-native-linux-${ARCH}"
STAGE_DIR="${DIST_DIR}/${PACKAGE_NAME}"
ARCHIVE_PATH="${DIST_DIR}/${PACKAGE_NAME}.tar.gz"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"
ICON_SOURCE_DIR="${ROOT_DIR}/src-tauri/icons"

bash "${APP_ROOT}/packaging/check-deps.sh"

mkdir -p "${DIST_DIR}"
rm -rf "${STAGE_DIR}"
rm -f "${ARCHIVE_PATH}" "${CHECKSUM_PATH}"
mkdir -p \
  "${STAGE_DIR}/bin" \
  "${STAGE_DIR}/share/applications" \
  "${STAGE_DIR}/share/metainfo" \
  "${STAGE_DIR}/share/icons/hicolor/32x32/apps" \
  "${STAGE_DIR}/share/icons/hicolor/128x128/apps" \
  "${STAGE_DIR}/share/icons/hicolor/256x256/apps"

cargo build --release -p lantern-native-linux --manifest-path "${ROOT_DIR}/Cargo.toml"

install -m 0755 "${ROOT_DIR}/target/release/lantern-native-linux" "${STAGE_DIR}/bin/lantern-native-linux"
ln -sf lantern-native-linux "${STAGE_DIR}/bin/lantern"
install -m 0644 "${APP_ROOT}/packaging/sh.lantern.NativeLinux.desktop.in" "${STAGE_DIR}/share/applications/sh.lantern.NativeLinux.desktop.in"
install -m 0644 "${APP_ROOT}/packaging/sh.lantern.NativeLinux.metainfo.xml" "${STAGE_DIR}/share/metainfo/sh.lantern.NativeLinux.metainfo.xml"
install -m 0644 "${ICON_SOURCE_DIR}/32x32.png" "${STAGE_DIR}/share/icons/hicolor/32x32/apps/sh.lantern.NativeLinux.png"
install -m 0644 "${ICON_SOURCE_DIR}/128x128.png" "${STAGE_DIR}/share/icons/hicolor/128x128/apps/sh.lantern.NativeLinux.png"
install -m 0644 "${ICON_SOURCE_DIR}/128x128@2x.png" "${STAGE_DIR}/share/icons/hicolor/256x256/apps/sh.lantern.NativeLinux.png"
install -m 0755 "${APP_ROOT}/packaging/install-bundle.sh" "${STAGE_DIR}/install.sh"
install -m 0755 "${APP_ROOT}/packaging/uninstall.sh" "${STAGE_DIR}/uninstall.sh"

tar -czf "${ARCHIVE_PATH}" -C "${DIST_DIR}" "${PACKAGE_NAME}"
sha256sum "${ARCHIVE_PATH}" > "${CHECKSUM_PATH}"

printf 'Staged native Linux bundle in %s\n' "${STAGE_DIR}"
printf 'Archive written to %s\n' "${ARCHIVE_PATH}"
printf 'Checksum written to %s\n' "${CHECKSUM_PATH}"
