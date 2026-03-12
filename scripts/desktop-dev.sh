#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(uname -s)" == "Linux" ]]; then
  bash "${ROOT_DIR}/apps/lantern-native-linux/packaging/check-deps.sh"
  cargo run -p lantern-native-linux --manifest-path "${ROOT_DIR}/Cargo.toml"
else
  cargo tauri dev --manifest-path "${ROOT_DIR}/src-tauri/Cargo.toml"
fi
