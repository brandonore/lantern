#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(uname -s)" == "Linux" ]]; then
  bash "${ROOT_DIR}/apps/lantern-native-linux/packaging/package.sh"
else
  cargo tauri build --manifest-path "${ROOT_DIR}/src-tauri/Cargo.toml"
fi
