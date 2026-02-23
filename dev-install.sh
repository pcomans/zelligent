#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

VERSION=$(cat VERSION)
SHA=$(git rev-parse --short HEAD)
STAMP="${VERSION}-dev+${SHA}"

# Install zelligent script
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# Ensure zelligent.sh contains the expected placeholder before stamping
if ! grep -q "__COMMIT_SHA__" zelligent.sh; then
  echo "Error: zelligent.sh does not contain the __COMMIT_SHA__ placeholder." >&2
  exit 1
fi

sed "s/__COMMIT_SHA__/$STAMP/" zelligent.sh > "$INSTALL_DIR/zelligent"
chmod +x "$INSTALL_DIR/zelligent"
echo "Installed zelligent ($STAMP) to $INSTALL_DIR/zelligent"

# Build and install Zellij plugin
cd plugin
sed -i.bak "s/version = \"0.0.0-dev\"/version = \"$VERSION\"/" Cargo.toml
if ! grep -q "version = \"$VERSION\"" Cargo.toml; then
  echo "Error: Failed to stamp version into Cargo.toml" >&2
  mv Cargo.toml.bak Cargo.toml
  exit 1
fi
bash build.sh
mv Cargo.toml.bak Cargo.toml

# Also install plugin to share dir for `zelligent doctor` compatibility
SHARE_DIR="$HOME/.local/share/zelligent"
mkdir -p "$SHARE_DIR"
cp "target/wasm32-wasip1/release/zelligent-plugin.wasm" "$SHARE_DIR/zelligent-plugin.wasm"
echo "Installed plugin to $SHARE_DIR/zelligent-plugin.wasm"
