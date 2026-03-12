#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(uname -s)" == "Linux" ]]; then
  bash "${ROOT_DIR}/apps/lantern-native-linux/packaging/uninstall.sh"
else
  printf 'Desktop uninstall automation is only implemented for the native Linux client.\n' >&2
  exit 1
fi
