#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

SHA=$(git rev-parse HEAD)

# Install zelligent script
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# Ensure zelligent.sh contains the expected placeholder before stamping
if ! grep -q "__COMMIT_SHA__" zelligent.sh; then
  echo "Error: zelligent.sh does not contain the __COMMIT_SHA__ placeholder." >&2
  exit 1
fi

sed "s/__COMMIT_SHA__/$SHA/" zelligent.sh > "$INSTALL_DIR/zelligent"

# Verify that the stamped script contains the expected SHA
if ! grep -q "$SHA" "$INSTALL_DIR/zelligent"; then
  echo "Error: Failed to stamp zelligent with commit SHA $SHA." >&2
  exit 1
fi
chmod +x "$INSTALL_DIR/zelligent"
echo "Installed zelligent ($SHA) to $INSTALL_DIR/zelligent"

# Build and install Zellij plugin
cd plugin
bash build.sh
