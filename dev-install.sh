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
cd ..

# Also install plugin to share dir for `zelligent doctor` compatibility
SHARE_DIR="$HOME/.local/share/zelligent"
mkdir -p "$SHARE_DIR"
cp "plugin/target/wasm32-wasip1/release/zelligent-plugin.wasm" "$SHARE_DIR/zelligent-plugin.wasm"
echo "Installed plugin to $SHARE_DIR/zelligent-plugin.wasm"

# Install Claude Code plugin (marketplace directory) for `zelligent doctor`
PLUGIN_SRC="claude-plugin"
PLUGIN_DST="$SHARE_DIR/claude-plugin"
if [ ! -d "$PLUGIN_SRC" ]; then
  echo "Error: Missing bundled plugin at $PLUGIN_SRC" >&2
  exit 1
fi
rm -rf "$PLUGIN_DST"
cp -R "$PLUGIN_SRC" "$PLUGIN_DST"
echo "Installed Claude plugin to $PLUGIN_DST"

# Build and install Zellij from source (main branch)
# Default to sibling ../zellij directory, override with ZELLIGENT_ZELLIJ_SRC
ZELLIJ_SRC="${ZELLIGENT_ZELLIJ_SRC:?Set ZELLIGENT_ZELLIJ_SRC to your local Zellij checkout}"
if [ ! -d "$ZELLIJ_SRC" ]; then
  echo "Error: Zellij source not found at $ZELLIJ_SRC" >&2
  echo "Either clone https://github.com/zellij-org/zellij next to the zelligent repo," >&2
  echo "or set ZELLIGENT_ZELLIJ_SRC to your local checkout." >&2
  exit 1
fi
# Uninstall Homebrew Zellij to avoid PATH conflicts
if command -v brew >/dev/null 2>&1; then
  if brew list zellij &>/dev/null; then
    echo "Uninstalling Homebrew Zellij to avoid PATH conflicts..."
    brew uninstall zellij
  fi
fi
echo "Building Zellij from $ZELLIJ_SRC..."
cargo build --release --manifest-path "$ZELLIJ_SRC/Cargo.toml"
cp "$ZELLIJ_SRC/target/release/zellij" "$INSTALL_DIR/zellij"
echo "Installed Zellij to $INSTALL_DIR/zellij"

# Update Zellij config to point at the dev plugin path
CONFIG="$HOME/.config/zellij/config.kdl"
if [ -f "$CONFIG" ]; then
  sed -i.bak "s|file:[^ ]*zelligent-plugin\.wasm|file:$SHARE_DIR/zelligent-plugin.wasm|g" "$CONFIG" && rm "$CONFIG.bak"
  echo "Updated $CONFIG to reference $SHARE_DIR/zelligent-plugin.wasm"
fi
