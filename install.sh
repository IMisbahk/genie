#!/usr/bin/env bash
set -euo pipefail

REPO_OWNER="imisbahk"
REPO_NAME="genie"
BIN_NAME="genie"
INSTALL_DIR="/usr/local/bin"
FALLBACK_INSTALL_DIR="$HOME/.local/bin"
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map arch names
case "$ARCH" in
  x86_64|amd64) ARCH=amd64 ;;
  arm64|aarch64) ARCH=arm64 ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
 esac

# Choose asset suffix (adjust if you publish different formats)
ASSET_SUFFIX="$OS-$ARCH.zip"
API_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"

need_sudo() {
  [ -w "$INSTALL_DIR" ] || command -v sudo >/dev/null 2>&1
}

install_path() {
  if [ -w "$INSTALL_DIR" ]; then
    echo "$INSTALL_DIR/$BIN_NAME"
  else
    mkdir -p "$FALLBACK_INSTALL_DIR"
    echo "$FALLBACK_INSTALL_DIR/$BIN_NAME"
  fi
}

install_from_release() {
  echo "Fetching latest release metadata..."
  TAG=$(curl -fsSL "$API_URL" | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name"\s*:\s*"([^"]+)".*/\1/')
  if [ -z "${TAG:-}" ]; then
    echo "Could not determine latest release tag"; return 1
  fi

  # Find asset URL matching OS/ARCH suffix
  ASSET_URL=$(curl -fsSL "$API_URL" | \
    grep -E '"browser_download_url"\s*:\s*".*' | \
    sed -E 's/.*"browser_download_url"\s*:\s*"([^"]+)".*/\1/' | \
    grep "$ASSET_SUFFIX" | head -n1)
  if [ -z "${ASSET_URL:-}" ]; then
    echo "No matching asset found for $OS-$ARCH (suffix $ASSET_SUFFIX)"; return 1
  fi

  TMPDIR=$(mktemp -d)
  echo "Downloading $ASSET_URL ..."
  curl -fsSL "$ASSET_URL" -o "$TMPDIR/$BIN_NAME.zip"
  echo "Extracting..."
  unzip -q "$TMPDIR/$BIN_NAME.zip" -d "$TMPDIR"

  TARGET=$(install_path)
  echo "Installing to $TARGET"
  if [[ "$TARGET" == "$INSTALL_DIR/$BIN_NAME" ]] && ! [ -w "$INSTALL_DIR" ]; then
    sudo cp "$TMPDIR/$BIN_NAME" "$TARGET"
    sudo chmod +x "$TARGET"
  else
    cp "$TMPDIR/$BIN_NAME" "$TARGET"
    chmod +x "$TARGET"
  fi

  rm -rf "$TMPDIR"
  echo "Installed $BIN_NAME to $TARGET"
  echo "Ensure the directory is on your PATH."

  echo
  echo "Running post-install..."
  "$TARGET" welcome || true
  # Try to open docs (opens browser on macOS/Linux); non-fatal if it fails
  "$TARGET" docs || true
  echo
  echo "Genie help:\n"
  "$TARGET" --help || true
}

install_from_release
