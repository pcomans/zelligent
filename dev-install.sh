#!/bin/bash
set -e

cd "$(dirname "$0")"

SHA=$(git rev-parse HEAD)

# Install spawn-agent script
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
sed "s/__COMMIT_SHA__/$SHA/" spawn-agent.sh > "$INSTALL_DIR/spawn-agent"
chmod +x "$INSTALL_DIR/spawn-agent"
echo "Installed spawn-agent ($SHA) to $INSTALL_DIR/spawn-agent"

# Build and install Zellij plugin
cd plugin
bash build.sh
