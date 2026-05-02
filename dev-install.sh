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

DEFAULT_LAYOUT_SRC="share/default-layout.kdl"
DEFAULT_LAYOUT_DST="$SHARE_DIR/default-layout.kdl"
if [ ! -f "$DEFAULT_LAYOUT_SRC" ]; then
  echo "Error: Missing default layout asset at $DEFAULT_LAYOUT_SRC" >&2
  exit 1
fi
cp "$DEFAULT_LAYOUT_SRC" "$DEFAULT_LAYOUT_DST"
echo "Installed default layout to $DEFAULT_LAYOUT_DST"

USER_LAYOUT_DIR="$HOME/.zelligent"
USER_LAYOUT_DST="$USER_LAYOUT_DIR/layout.kdl"
mkdir -p "$USER_LAYOUT_DIR"
if [ ! -f "$USER_LAYOUT_DST" ]; then
  cp "$DEFAULT_LAYOUT_DST" "$USER_LAYOUT_DST"
  echo "Created user layout at $USER_LAYOUT_DST"
else
  echo "Preserved existing user layout at $USER_LAYOUT_DST"
fi

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

# Optionally build and install a patched Zellij from local source
if [ -n "$ZELLIGENT_ZELLIJ_SRC" ]; then
  if [ ! -d "$ZELLIGENT_ZELLIJ_SRC" ]; then
    echo "Error: ZELLIGENT_ZELLIJ_SRC=$ZELLIGENT_ZELLIJ_SRC does not exist" >&2
    exit 1
  fi
  # Uninstall Homebrew Zellij to avoid PATH conflicts, if Homebrew is available
  if command -v brew >/dev/null 2>&1; then
    if brew list zellij &>/dev/null; then
      echo "Uninstalling Homebrew Zellij to avoid PATH conflicts..."
      brew uninstall zellij
    fi
  fi
  echo "Building Zellij from $ZELLIGENT_ZELLIJ_SRC..."
  cargo build --release --manifest-path "$ZELLIGENT_ZELLIJ_SRC/Cargo.toml"
  cp "$ZELLIGENT_ZELLIJ_SRC/target/release/zellij" "$INSTALL_DIR/zellij"
  # macOS invalidates adhoc code signatures on copy; re-sign to prevent SIGKILL
  if [ "$(uname)" = "Darwin" ]; then
    codesign --force --sign - "$INSTALL_DIR/zellij"
  fi
  echo "Installed patched Zellij to $INSTALL_DIR/zellij"
fi

# Update Zellij config to point at the dev plugin path
CONFIG="$HOME/.config/zellij/config.kdl"
if [ -f "$CONFIG" ]; then
  sed -i.bak "s|file:[^ ]*zelligent-plugin\.wasm|file:$SHARE_DIR/zelligent-plugin.wasm|g" "$CONFIG" && rm "$CONFIG.bak"
  echo "Updated $CONFIG to reference $SHARE_DIR/zelligent-plugin.wasm"
fi

# Grant Zellij plugin permissions for the dev plugin path. Zellij will
# otherwise refuse RunCommands and the sidebar's actions silently fail. If the
# user previously dismissed the in-session prompt, zellij persists an empty
# block (`"…wasm" { }`) — rewrite it idempotently rather than appending.
if [ "$(uname)" = "Darwin" ]; then
  PERM_FILE="$HOME/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl"
else
  PERM_FILE="${XDG_CACHE_HOME:-$HOME/.cache}/zellij/permissions.kdl"
fi
mkdir -p "$(dirname "$PERM_FILE")"
touch "$PERM_FILE"
DEV_PLUGIN_PATH="$SHARE_DIR/zelligent-plugin.wasm"
if grep -qF "\"$DEV_PLUGIN_PATH\"" "$PERM_FILE"; then
  PLUGIN_PATH="$DEV_PLUGIN_PATH" perl -i -0pe '
    my $path = quotemeta($ENV{PLUGIN_PATH});
    my $block = qq{"$ENV{PLUGIN_PATH}" \{\n    ChangeApplicationState\n    ReadApplicationState\n    RunCommands\n    ReadCliPipes\n\}\n};
    if (/"$path"\s*\{([^}]*)\}/s) {
      my $body = $1;
      my $needs = $body !~ /ChangeApplicationState/
               || $body !~ /ReadApplicationState/
               || $body !~ /RunCommands/
               || $body !~ /ReadCliPipes/;
      s/"$path"\s*\{[^}]*\}\s*\n?/$block/s if $needs;
    }
  ' "$PERM_FILE"
else
  cat >> "$PERM_FILE" <<PERMS
"$DEV_PLUGIN_PATH" {
    ChangeApplicationState
    ReadApplicationState
    RunCommands
    ReadCliPipes
}
PERMS
fi
echo "Granted plugin permissions in $PERM_FILE"

# Make sure the dev binary actually wins on PATH. Homebrew installs into
# /opt/homebrew/bin (or /usr/local/bin on Intel) which typically precedes
# ~/.local/bin, so a `brew install pcomans/zelligent/zelligent` will silently
# shadow this dev install. Detect that case and unlink the brew shim — `brew
# link zelligent` reverses it cleanly.
DEV_BIN="$INSTALL_DIR/zelligent"
RESOLVED=$(command -v zelligent 2>/dev/null || true)
if [ -n "$RESOLVED" ] && [ "$RESOLVED" != "$DEV_BIN" ]; then
  REAL_RESOLVED=$(readlink -f "$RESOLVED" 2>/dev/null || echo "$RESOLVED")
  if command -v brew >/dev/null 2>&1 && [[ "$REAL_RESOLVED" == *"/Cellar/zelligent/"* ]]; then
    echo "Unlinking Homebrew zelligent so the dev install takes precedence on PATH..."
    brew unlink zelligent >/dev/null
    echo "  (run 'brew link zelligent' to switch back to the released version.)"
  else
    echo "Warning: '$RESOLVED' shadows the dev install at $DEV_BIN." >&2
    echo "         Adjust your PATH so $INSTALL_DIR comes first, or remove the shadowing binary." >&2
  fi
fi

# Re-resolve after any unlink. The current shell's command hash is stale; tell
# the user to refresh it (we can't do this from a child process).
NEW_RESOLVED=$(PATH="$PATH" command -v zelligent 2>/dev/null || true)
if [ "$NEW_RESOLVED" = "$DEV_BIN" ]; then
  echo "✓ 'zelligent' on PATH now points at the dev install."
  echo "  In an open shell, run 'hash -r' (or open a new terminal) to drop the cached path."
fi

# Heads-up: zellij resurrects sessions by name, so an existing session created
# by an older zelligent will reattach with its old layout. Mention how to
# start fresh.
if command -v zellij >/dev/null 2>&1; then
  EXISTING=$(zellij list-sessions --no-formatting --short 2>/dev/null || true)
  if [ -n "$EXISTING" ]; then
    echo
    echo "Note: zellij resurrects sessions by name. To pick up dev changes in an"
    echo "existing session, delete it first: 'zellij delete-session <name> --force'."
    echo "Active sessions:"
    printf '%s\n' "$EXISTING" | sed 's/^/  /'
  fi
fi
