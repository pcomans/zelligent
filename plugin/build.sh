#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "Building zelligent-plugin..."
cargo build --release

WASM="target/wasm32-wasip1/release/zelligent-plugin.wasm"
if [ ! -f "$WASM" ]; then
  echo "Error: $WASM not found"
  exit 1
fi

DEST="$HOME/.config/zellij/plugins"
mkdir -p "$DEST"
cp "$WASM" "$DEST/zelligent-plugin.wasm"
echo "Installed to $DEST/zelligent-plugin.wasm"
