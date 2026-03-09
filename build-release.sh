#!/bin/bash
# Create a GitHub release with pre-built binaries

set -e

VERSION=$(cat VERSION)
SHA=$(git rev-parse --short HEAD)

echo "Building zelligent release v${VERSION}..."

# Build the WASM plugin
cd plugin
bash build.sh
cd ..

# Create release directory
mkdir -p release/zelligent-${VERSION}

# Copy files
cp zelligent.sh release/zelligent-${VERSION}/zelligent
cp plugin/target/wasm32-wasip1/release/zelligent-plugin.wasm release/zelligent-${VERSION}/
cp -r claude-plugin release/zelligent-${VERSION}/
cp README.md LICENSE release/zelligent-${VERSION}/

# Create tarball
cd release
tar -czf zelligent-${VERSION}-linux-x86_64.tar.gz zelligent-${VERSION}/

echo "Release tarball created: release/zelligent-${VERSION}-linux-x86_64.tar.gz"
