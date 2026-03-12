#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP_ROOT="${ROOT_DIR}/apps/lantern-native-linux"
STAGE_DIR="${APP_ROOT}/dist/lantern-native-linux-$(uname -m)"
VERIFY_HOME="/tmp/lantern-native-bundle-verify"

if [[ ! -x "${STAGE_DIR}/install.sh" ]]; then
  printf 'Missing staged bundle installer at %s. Run npm run native:package first.\n' "${STAGE_DIR}/install.sh" >&2
  exit 1
fi

rm -rf "${VERIFY_HOME}"
mkdir -p "${VERIFY_HOME}"

HOME="${VERIFY_HOME}" bash "${STAGE_DIR}/install.sh"

test -x "${VERIFY_HOME}/.local/bin/lantern-native-linux"
test -L "${VERIFY_HOME}/.local/bin/lantern"
test -f "${VERIFY_HOME}/.local/share/applications/sh.lantern.NativeLinux.desktop"
test -f "${VERIFY_HOME}/.local/share/metainfo/sh.lantern.NativeLinux.metainfo.xml"
test -f "${VERIFY_HOME}/.local/share/icons/hicolor/32x32/apps/sh.lantern.NativeLinux.png"
test -f "${VERIFY_HOME}/.local/share/icons/hicolor/128x128/apps/sh.lantern.NativeLinux.png"
test -f "${VERIFY_HOME}/.local/share/icons/hicolor/256x256/apps/sh.lantern.NativeLinux.png"

HOME="${VERIFY_HOME}" bash "${STAGE_DIR}/uninstall.sh"

test ! -e "${VERIFY_HOME}/.local/bin/lantern-native-linux"
test ! -e "${VERIFY_HOME}/.local/bin/lantern"
test ! -e "${VERIFY_HOME}/.local/share/applications/sh.lantern.NativeLinux.desktop"
test ! -e "${VERIFY_HOME}/.local/share/metainfo/sh.lantern.NativeLinux.metainfo.xml"
test ! -e "${VERIFY_HOME}/.local/share/icons/hicolor/32x32/apps/sh.lantern.NativeLinux.png"
test ! -e "${VERIFY_HOME}/.local/share/icons/hicolor/128x128/apps/sh.lantern.NativeLinux.png"
test ! -e "${VERIFY_HOME}/.local/share/icons/hicolor/256x256/apps/sh.lantern.NativeLinux.png"

printf 'Verified staged native bundle install/uninstall using HOME=%s\n' "${VERIFY_HOME}"
