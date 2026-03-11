---
fixture: setup-empty-repo.sh
---

# Sidebar Layout Smoke Test

Verifies that the persistent sidebar renders correctly in the layout.

## Test 1: Sidebar is visible and in browse mode
- Action: Open a new tab with the zelligent sidebar layout (`zellij action new-tab --layout FILE` from within the test-harness session)
- Expected: The terminal shows a split layout with a narrow pane on the left (~24% width) containing the zelligent plugin. The plugin renders in browse mode — listing the session's existing tabs (including "test-harness") as sidebar entries — with footer hints "↑/k up ↓/j down Enter open". No empty-state logo is expected here because the session already has tabs.
- Note: The empty-state logo only appears when zelligent starts a brand-new single-tab session with no existing tabs. That path cannot be tested via the harness (which always adds a tab to an existing session).

## Test 2: No tab bar is present
- Action: Read the full terminal buffer
- Expected: There is NO `Zellij` tab bar at the top of the screen. The sidebar replaces the tab bar.

## Test 3: Status bar is present at bottom
- Action: Read the bottom of the terminal buffer
- Expected: The Zellij status bar is visible at the very bottom of the terminal.

## Test 4: Sidebar shows version
- Action: Read the sidebar pane content
- Expected: A version string appears at the bottom of the sidebar (e.g., "0.0.0-dev" or a semver-like string).

## Test 5: Main pane shows shell prompt
- Action: Read the terminal buffer for the main (right) pane
- Expected: A shell prompt is visible in the main pane, showing `/tmp/zelligent-test-repo` as the cwd.
