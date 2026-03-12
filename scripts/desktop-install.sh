#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(uname -s)" == "Linux" ]]; then
  bash "${ROOT_DIR}/apps/lantern-native-linux/packaging/install.sh"
else
  printf 'Desktop install automation is only implemented for the native Linux client.\n' >&2
  exit 1
fi
