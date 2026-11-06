#!/bin/bash
# Installs or updates TextActions on macOS from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/huypham-dev/text-actions/main/install.sh | bash
#
# Optional environment variables:
#   TEXTACTIONS_INSTALL_DIR  Install here instead of /Applications.
#   TEXTACTIONS_NO_OPEN=1    Do not open the app after installing.

# Everything runs from main, called on the last line, so a download that is cut
# off part-way cannot run a partial script.
set -euo pipefail

REPO_URL="https://github.com/huypham-dev/text-actions"
APP_NAME="TextActions.app"

fail() {
  echo "Error: $1" >&2
  exit 1
}

latest_version() {
  local url
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$REPO_URL/releases/latest")" ||
    fail "could not reach GitHub. Check your internet connection and try again."
  case "$url" in
    */releases/tag/v*) echo "${url##*/releases/tag/v}" ;;
    *) fail "could not find the latest release at $REPO_URL/releases" ;;
  esac
}

# Reads the hardware rather than `uname -m`, which reports x86_64 on an Apple
# silicon Mac when the shell runs under Rosetta.
asset_name() {
  local version="$1"
  if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = "1" ]; then
    echo "TextActions-$version-arm64.dmg"
  else
    echo "TextActions-$version.dmg"
  fi
}

install_dir() {
  if [ -n "${TEXTACTIONS_INSTALL_DIR:-}" ]; then
    mkdir -p "$TEXTACTIONS_INSTALL_DIR"
    echo "$TEXTACTIONS_INSTALL_DIR"
  elif [ -w /Applications ]; then
    echo /Applications
  else
    mkdir -p "$HOME/Applications"
    echo "$HOME/Applications"
  fi
}

stop_running_app() {
  local binary="$1/Contents/MacOS/TextActions"
  pgrep -f "$binary" >/dev/null 2>&1 || return 0
  echo "Closing the running copy of TextActions..."
  pkill -f "$binary" || true
  local waited=0
  while pgrep -f "$binary" >/dev/null 2>&1; do
    [ "$waited" -lt 10 ] || fail "could not close TextActions. Quit it from the menu bar and try again."
    sleep 1
    waited=$((waited + 1))
  done
}

# Global, not local: the EXIT trap runs after main has returned.
work=""
mount=""
staged=""

cleanup() {
  if [ -n "$mount" ]; then
    hdiutil detach "$mount" -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$work" "$staged"
}

main() {
  [ "$(uname -s)" = "Darwin" ] || fail "this script is for macOS. On Windows, see $REPO_URL#install"

  local version asset dir target
  version="$(latest_version)"
  asset="$(asset_name "$version")"
  dir="$(install_dir)"
  target="$dir/$APP_NAME"
  staged="$dir/.$APP_NAME.new"
  work="$(mktemp -d)"
  mount="$work/mount"

  trap cleanup EXIT

  echo "Downloading TextActions $version..."
  curl -fL --progress-bar -o "$work/$asset" "$REPO_URL/releases/download/v$version/$asset" ||
    fail "could not download $asset"

  echo "Installing to $dir..."
  mkdir -p "$mount"
  hdiutil attach "$work/$asset" -nobrowse -readonly -quiet -mountpoint "$mount" ||
    fail "could not open $asset"
  [ -d "$mount/$APP_NAME" ] || fail "$asset does not contain $APP_NAME"

  # Copy first and swap last, so a failed copy leaves any existing app in place.
  rm -rf "$staged"
  ditto "$mount/$APP_NAME" "$staged"
  stop_running_app "$target"
  rm -rf "$target"
  mv "$staged" "$target"

  echo
  echo "TextActions $version is installed at $target"
  echo "It runs from the menu bar and has no Dock icon."
  echo "When it asks, turn it on under System Settings > Privacy & Security > Accessibility."

  if [ ! -x "$HOME/.local/bin/claude" ] && ! command -v claude >/dev/null 2>&1; then
    echo
    echo "TextActions also needs the Claude Code CLI, which was not found on this Mac."
    echo "Install it and log in: https://claude.com/product/claude-code"
  fi

  if [ "${TEXTACTIONS_NO_OPEN:-}" != "1" ]; then
    open "$target"
  fi
}

main "$@"
