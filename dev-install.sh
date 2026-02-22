#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

SHA=$(git rev-parse HEAD)

# Install spawn-agent script
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# Ensure spawn-agent.sh contains the expected placeholder before stamping
if ! grep -q "__COMMIT_SHA__" spawn-agent.sh; then
  echo "Error: spawn-agent.sh does not contain the __COMMIT_SHA__ placeholder." >&2
  exit 1
fi

sed "s/__COMMIT_SHA__/$SHA/" spawn-agent.sh > "$INSTALL_DIR/spawn-agent"

# Verify that the stamped script contains the expected SHA
if ! grep -q "$SHA" "$INSTALL_DIR/spawn-agent"; then
  echo "Error: Failed to stamp spawn-agent with commit SHA $SHA." >&2
  exit 1
fi
chmod +x "$INSTALL_DIR/spawn-agent"
echo "Installed spawn-agent ($SHA) to $INSTALL_DIR/spawn-agent"

# Build and install Zellij plugin
cd plugin
bash build.sh
