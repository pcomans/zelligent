# Sidebar Mouse Mapping and Header-Line Fix (#135/#136)

Fixes two interlocked bugs found by the 2026-07 real-input UI audit
(`tests/harness/plans/ui-audit-06-repro-verification.md` R1-R3):

- **Z-1 / #135** — clicking a sidebar item's subtitle line (`branch: X`)
  selected the *next* item, and the last item's subtitle mapped past the
  end of the list (dead zone).
- **Z-2 / #136** — the in-pane header (` zelligent / <repo> `) never
  displayed in a live session, and the number of leading (non-item) lines
  shifted at runtime whenever a status message wrapped to two physical
  rows, which in turn shifted the Z-1 offset dynamically.

## Root cause

`plugin/src/lib.rs`'s `render_to` computes, for every `Mode`, a `padding`
count so that `header + content + padding + status + footer` equals
exactly `rows` lines, and writes every one of those lines with
`writeln!` — including the very last line of the frame (the version
line in `ui::render_footer`).

Zellij's plugin pane is a real terminal grid. After the per-frame
`\x1b[H` (cursor home) + `\x1b[2J` (clear), the cursor sits at row 0.
Writing exactly `rows` newline-terminated lines advances the cursor
through `rows` row transitions — one past the last valid row index
(`rows - 1`) — which forces the grid to scroll by one line, silently
discarding row 0 (the header). This happens **on every render**, not
just under some edge condition, because the code deliberately targets
`total lines == rows`.

When a status message is longer than the pane's `cols`, the terminal's
own auto-wrap consumes an extra physical row that the old code never
counted (`status_height` was a fixed `2`, whatever the message's real
width), pushing the same mechanism to swallow the blank separator line
too — this is what made the leading-line count (and therefore the Z-1
click offset) shift at runtime.

Verified by direct tmux/SGR-mouse diagnosis against a live session at
three pane heights (tall, short, and with a wrapped status message);
see the implementor's report for the annotated captures.

## The fix

1. **`ui::render_footer`'s last line no longer ends in `\n`.** Since
   `render_footer` is always the last thing every `Mode` arm of
   `render_to` writes, this alone makes a frame that fills `rows`
   exactly land the cursor on the last valid row instead of past it —
   no more universal 1-line scroll, so the header renders in every
   `Mode`, at every pane size that fits its own minimum content.

2. **`ui::sidebar_layout(rows, cols, item_count, selected,
   status_message) -> SidebarLayout`** is now the single function that
   computes the sidebar's entire vertical layout for one frame:

   ```rust
   pub struct SidebarLayout {
       pub show_header: bool,
       pub show_separator: bool,
       pub viewport: SidebarViewport,
       pub status_lines: usize,   // wrap-aware: blank + ceil(width/cols)
       pub footer_lines: usize,   // 3 (cols>=55) or 4
   }
   ```

   Both `render_to`'s `BrowseWorktrees` arm (drawing) and
   `State::sidebar_index_at_line` (mouse hit-testing) call this same
   function with the same inputs (`last_rows`/`last_cols` captured at
   render time, plus the current `selected_index` and
   `status_message`), so a click can never resolve to a different row
   than what was actually drawn. The old standalone `ui::sidebar_viewport`
   (which used a blind `rows.saturating_sub(5)` heuristic, oblivious to
   `cols`, the footer's real height, or status wrap) has been removed —
   there is now exactly one place that does the *sidebar viewport*
   arithmetic.

   A follow-up review found the unification was incomplete: three other
   `render_to` arms (`NotGitRepo`, `BrowseWorktrees`'s empty-state branch,
   and `InputBranch`) still computed their own `status_height` as a fixed
   `if status_message.is_empty() { 0 } else { 2 }`, independently of
   `sidebar_layout` — undercounting a wrapped status message and
   re-triggering the exact header-swallowing scroll this fix was written
   to eliminate, just outside the populated sidebar-list path. The wrap
   math was pulled out of `sidebar_layout` into `ui::status_height(status_message,
   cols) -> usize`, which `sidebar_layout` now calls too, and all three
   arms were switched to call it instead of the fixed guess. There is now
   exactly one place that computes wrap-aware status height, shared by
   every render arm that reserves space for `render_status`.

3. **Graceful degradation**, in this order, so an item row is never the
   first thing sacrificed on a too-short pane:
   - `content_budget = rows - (status_lines + footer_lines)`
   - `content_budget >= 4`: header + separator + >=1 item
   - `content_budget == 3`: header only (separator dropped)
   - `content_budget < 3`: neither header nor separator

   The item viewport's row budget is carved out of `content_budget`
   *after* leading lines are decided, using the same arithmetic that
   used to live in `ui::sidebar_viewport`.

4. **`State::sidebar_index_at_line(&self, line: usize) -> Option<usize>`**
   no longer takes a `rows` parameter — it reads `self.last_rows` and
   the new `self.last_cols` (added alongside `last_rows`, set in
   `ZellijPlugin::render`) and recomputes the identical `SidebarLayout`.
   Hit-testing semantics: a click on an item's title OR subtitle line
   selects that item; a click on any other line (header, separator,
   footer, status, or past the last visible item) is a strict no-op.

## Known limitation (not in scope)

Footer/version rows are not degraded. On a pane so short that even one
forced-minimum item (2 rows) plus the footer (3-4 rows) don't jointly
fit `rows` — far below anything in the audited range (smallest tested:
220x24) — the residual 1-line overflow the header/blank fix eliminated
elsewhere can still reappear. Fixing this would require designing
footer degradation, which the frozen design for #135/#136 didn't ask
for.
