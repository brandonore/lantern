#!/usr/bin/env bash
set -euo pipefail

missing_commands=()
missing_packages=()

require_command() {
  local command_name="$1"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    missing_commands+=("${command_name}")
  fi
}

require_pkg_config_package() {
  local package_name="$1"
  if ! pkg-config --exists "${package_name}"; then
    missing_packages+=("${package_name}")
  fi
}

require_command cargo
require_command pkg-config

if command -v pkg-config >/dev/null 2>&1; then
  require_pkg_config_package gtk4
  require_pkg_config_package libadwaita-1
  require_pkg_config_package libsoup-3.0
  require_pkg_config_package vte-2.91-gtk4
fi

if (( ${#missing_commands[@]} == 0 && ${#missing_packages[@]} == 0 )); then
  exit 0
fi

if (( ${#missing_commands[@]} > 0 )); then
  printf 'Missing required commands: %s\n' "${missing_commands[*]}" >&2
fi

if (( ${#missing_packages[@]} > 0 )); then
  printf 'Missing required native packages for Lantern Native: %s\n' "${missing_packages[*]}" >&2
  printf 'On Debian/Ubuntu, install: libadwaita-1-dev libgtk-4-dev libsoup-3.0-dev libvte-2.91-gtk4-dev\n' >&2
fi

exit 1
