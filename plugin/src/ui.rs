// ANSI escape helpers
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RESET: &str = "\x1b[0m";
pub const INVERSE: &str = "\x1b[7m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const CYAN: &str = "\x1b[36m";
pub const YELLOW: &str = "\x1b[33m";

use std::collections::BTreeMap;
use std::io::Write;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{AgentStatus, Mode, SidebarItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarViewport {
    pub start: usize,
    pub max_items: usize,
    pub visible_items: usize,
}

/// Single source of truth for the BrowseWorktrees sidebar's vertical layout.
/// Computed once from `(rows, cols, item_count, selected, status_message)`
/// and consumed by BOTH the renderer (`render_to` / `render_sidebar_list`)
/// and the mouse-click mapper (`State::sidebar_index_at_line`), so a click
/// can never resolve to a different row than what was actually drawn.
/// See #135/#136 — both bugs were a direct consequence of the render path
/// and the hit-test path each guessing this layout independently, and
/// disagreeing whenever the guesses drifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarLayout {
    /// Whether the ` <repo> ` header banner line is drawn this frame (#156:
    /// repo name only — the pane frame title carries the tool name).
    pub show_header: bool,
    /// Whether a blank separator line is drawn between the header (or the
    /// pane top, if the header itself was dropped) and the first item.
    pub show_separator: bool,
    /// The scrollable item viewport: which items are visible, and where.
    pub viewport: SidebarViewport,
    /// Physical rows the status message will occupy this frame, including
    /// its leading blank line and any wrap at the real pane width. 0 when
    /// there is no status message.
    pub status_lines: usize,
    /// Physical rows the footer (blank + command hints + version) occupies.
    pub footer_lines: usize,
}

impl SidebarLayout {
    /// Rows before the first item row: 0, 1 (header only), or 2 (header +
    /// blank separator). This is exactly the offset mouse-click line
    /// numbers need before dividing by 2 to find an item index.
    pub fn leading_lines(&self) -> usize {
        self.show_header as usize + self.show_separator as usize
    }
}

/// Degradation thresholds for `sidebar_layout`, in rows reserved for
/// `[header, separator, >=1 two-line item]` — i.e. `content_budget` below,
/// after status/footer rows are already carved out of `rows`. Order of
/// sacrifice on a too-short pane: blank separator first, then the header
/// itself — an item row is never sacrificed (frozen design, #135/#136):
///   content_budget >= 4: header + separator + >=1 item
///   content_budget == 3: header only (no separator) + >=1 item
///   content_budget <  3: neither header nor separator (bare item list)
const MIN_ROWS_HEADER_AND_SEPARATOR: usize = 4;
const MIN_ROWS_HEADER_ONLY: usize = 3;

/// The two-space hanging indent every status line — first line and
/// continuations alike — is rendered with. See `wrap_status`.
const STATUS_INDENT: &str = "  ";

/// Word-wrap `message` into display lines ready to print as-is: each line
/// already carries the `STATUS_INDENT` hanging indent, so a multi-line
/// status reads as one message instead of a first line that's indented and
/// continuations flush against the pane edge (#187). Wrapping breaks at
/// word boundaries; a single word wider than the available width still
/// hard-breaks character-by-character so this can never overflow the pane
/// or loop forever. This is the ONLY place that performs this wrap — both
/// `status_wrap_rows` (line-count math feeding `sidebar_layout`, and from
/// there `State::sidebar_index_at_line`'s click mapping) and `render_status`
/// (actual bytes on screen) route through it, so the row a click maps to
/// can never disagree with the row the render actually drew. See
/// #135/#136 for the original render/mapper-drift bug this pattern guards
/// against; #187 is a second instance of the same hazard.
pub fn wrap_status(message: &str, cols: usize) -> Vec<String> {
    if message.is_empty() {
        return Vec::new();
    }
    let indent_width = visible_width(STATUS_INDENT);
    let avail = cols.saturating_sub(indent_width).max(1);

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in message.split_whitespace() {
        let word_width = visible_width(word);
        if word_width > avail {
            // Word alone doesn't fit even on an empty line: hard-break it
            // character-by-character rather than overflow or loop forever.
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            for ch in word.chars() {
                let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                if current_width + ch_width > avail && !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    current_width = 0;
                }
                current.push(ch);
                current_width += ch_width;
            }
            continue;
        }

        if current.is_empty() {
            current = word.to_string();
            current_width = word_width;
        } else if current_width + 1 + word_width <= avail {
            current.push(' ');
            current.push_str(word);
            current_width += 1 + word_width;
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
            current_width = word_width;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        // `message` was non-empty but all-whitespace.
        lines.push(String::new());
    }

    lines
        .into_iter()
        .map(|line| format!("{STATUS_INDENT}{line}"))
        .collect()
}

/// How many physical rows a status message will occupy (its content lines
/// only, not the leading blank line before it), accounting for word-wrap at
/// the real pane width. Delegates to `wrap_status` — see its doc comment
/// for why this must be the only place that computes this.
fn status_wrap_rows(message: &str, cols: usize) -> usize {
    wrap_status(message, cols).len().max(1)
}

/// Physical rows `render_status` will occupy this frame: 0 when there is no
/// message, else its leading blank line plus however many rows the message
/// wraps to at `cols` width. This is the single source of truth for that
/// arithmetic — `sidebar_layout` calls it, and so must every other render
/// arm in `lib.rs` that reserves space for a status message, so a wrapped
/// status can never re-trigger the header-swallowing scroll bug (#135/#136)
/// in a path that isn't the populated BrowseWorktrees list.
pub fn status_height(status_message: &str, cols: usize) -> usize {
    if status_message.is_empty() {
        0
    } else {
        1 + status_wrap_rows(status_message, cols)
    }
}

/// Compute the sidebar's full vertical layout for one frame. See
/// `SidebarLayout` for field meanings and the module-level doc comment for
/// why this must be the only place that does this arithmetic.
pub fn sidebar_layout(
    rows: usize,
    cols: usize,
    item_count: usize,
    selected: usize,
    status_message: &str,
) -> SidebarLayout {
    // #192: the populated-list footer is always two hint lines (nav+open on
    // one, create+remove on the other) — blank + line 1 + line 2 + version
    // — regardless of pane width, now that both lines fit comfortably down
    // to the narrowest tested sidebar width (30 cols). This used to vary
    // with `cols` (3 lines when a single combined line fit at cols>=55,
    // else 4); that split is gone along with the single-line variant.
    let footer_lines = 4;
    let status_lines = status_height(status_message, cols);

    let content_budget = rows.saturating_sub(status_lines + footer_lines);
    let (show_header, show_separator) = if content_budget >= MIN_ROWS_HEADER_AND_SEPARATOR {
        (true, true)
    } else if content_budget >= MIN_ROWS_HEADER_ONLY {
        (true, false)
    } else {
        (false, false)
    };

    let leading = show_header as usize + show_separator as usize;
    let item_rows_budget = content_budget.saturating_sub(leading);
    let max_items = (item_rows_budget / 2).max(1);
    let start = if selected >= max_items {
        selected - max_items + 1
    } else {
        0
    };
    let viewport = SidebarViewport {
        start,
        max_items,
        visible_items: item_count.saturating_sub(start).min(max_items),
    };

    SidebarLayout {
        show_header,
        show_separator,
        viewport,
        status_lines,
        footer_lines,
    }
}

/// Sanitize a branch name to match the shell's tab/session name logic:
/// replace `/` with `-`, then strip anything outside `[A-Za-z0-9_-]`.
pub fn sanitize_tab_name(branch: &str) -> String {
    branch
        .replace('/', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

pub fn visible_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn clip_to_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if visible_width(s) <= max_width {
        return s.to_string();
    }

    let mut width = 0;
    let mut out = String::new();
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}

pub fn fit_text(s: &str, width: usize) -> String {
    let clipped = clip_to_width(s, width);
    let padding = width.saturating_sub(visible_width(&clipped));
    format!("{clipped}{}", " ".repeat(padding))
}

fn status_color(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Idle => "",
        AgentStatus::Working => GREEN,
        AgentStatus::NeedsInput => YELLOW,
        AgentStatus::Done => GREEN,
    }
}

fn status_symbol(status: &AgentStatus) -> Option<&'static str> {
    match status {
        AgentStatus::Idle => None,
        AgentStatus::Working | AgentStatus::NeedsInput => Some("●"),
        AgentStatus::Done => Some("✓"),
    }
}

// The header carries ONLY the repo name (#156): the pane frame title already
// says "zelligent" (the lazygit convention — every pane's frame names its
// tool), so a brand prefix here reads as a stutter directly under the frame.
// The brand also lives in the footer next to the version. An empty repo name
// (state not loaded yet; error arms pass their own label) falls back to the
// tool name rather than an empty rule.
pub fn render_header(w: &mut impl Write, repo_name: &str, cols: usize) {
    let title = if repo_name.is_empty() {
        " zelligent ".to_string()
    } else {
        format!(" {repo_name} ")
    };
    let pad = cols.saturating_sub(visible_width(&title));
    writeln!(w, "{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad)).unwrap();
}

pub fn render_empty_state(w: &mut impl Write) {
    writeln!(w).unwrap();
    writeln!(w, "  {BOLD}No managed worktrees yet.{RESET}").unwrap();
    writeln!(
        w,
        "  {DIM}Pick a branch or type a new one to get started.{RESET}"
    )
    .unwrap();
    writeln!(w).unwrap();
    // `i` first (#185): it's the universally-valid first action in a fresh
    // repo, whereas `n`'s picker only has something to offer once other
    // branches exist — and the main repo's own checked-out branch is never
    // one of them (see `BranchAnnotation` / list-branches suppression).
    writeln!(w, "  {DIM}i{RESET}  type a new branch name").unwrap();
    writeln!(w, "  {DIM}n{RESET}  pick an existing branch").unwrap();
}

pub fn render_sidebar_list(
    w: &mut impl Write,
    items: &[SidebarItem],
    agent_statuses: &BTreeMap<String, AgentStatus>,
    repo_name: &str,
    active_tab_name: Option<&str>,
    selected: usize,
    layout: &SidebarLayout,
    cols: usize,
) {
    if items.is_empty() {
        writeln!(w).unwrap();
        writeln!(w, "  {DIM}Waiting for tabs...{RESET}").unwrap();
        return;
    }

    // Two pieces of state live on different visual axes so they can't compete:
    //   left gutter (▌ / blank) = navigation cursor
    //   title color/weight       = active tab in Zellij (bold cyan)
    //   right gutter (●/✓ / blank) = agent status
    //
    // The ▌ gutter intentionally spans BOTH lines of the selected item
    // (title + subtitle) — a continuous 2-row bar, not just a mark on the
    // title line. Confirmed intended.
    //
    // The cursor re-syncs to the active tab whenever this pane is revealed
    // (`State::handle_visible` — hidden instances receive no TabUpdates, so
    // reveal is the only reliable switch signal) or the active tab changes
    // within a delivered snapshot (`State::handle_tab_update`); see #151.
    // It tracks tab switches (sidebar click, Enter, or a native Ctrl-t
    // switch) but never fights j/k browsing within a tab, since neither
    // trigger fires during same-tab browsing.
    //
    // `layout` (computed once by `sidebar_layout`) is the only source of the
    // viewport and separator visibility — never recomputed here — so this
    // render can never draw a different set of rows than what
    // `State::sidebar_index_at_line` used to map the last click. See
    // #135/#136.
    let viewport = layout.viewport;
    let content_width = cols.saturating_sub(4).max(1);

    if layout.show_separator {
        writeln!(w).unwrap();
    }
    for (idx, item) in items
        .iter()
        .enumerate()
        .skip(viewport.start)
        .take(viewport.max_items)
    {
        let selected_row = idx == selected;
        let active_row = active_tab_name.is_some_and(|name| name == item.tab_name);
        let status = agent_statuses
            .get(&item.tab_name)
            .unwrap_or(&AgentStatus::Idle);
        let indicator = if item.matched_branch.is_some() {
            status_symbol(status)
        } else {
            None
        };
        let status_gutter = match indicator {
            Some(glyph) => format!(" {}{glyph}{RESET}", status_color(status)),
            None => "  ".to_string(),
        };
        let cursor_gutter = if selected_row {
            format!("{CYAN}▌{RESET} ")
        } else {
            "  ".to_string()
        };

        let title_text = fit_text(&item.display_name, content_width);
        let title = if active_row {
            format!("{BOLD}{CYAN}{title_text}{RESET}")
        } else {
            title_text
        };
        let subtitle = match item.matched_branch.as_deref() {
            Some(branch) => fit_text(&format!("branch: {branch}"), content_width),
            None if !repo_name.is_empty() && item.tab_name == repo_name => {
                fit_text("current repo", content_width)
            }
            None => fit_text("user tab", content_width),
        };

        writeln!(w, "{cursor_gutter}{title}{status_gutter}").unwrap();
        writeln!(w, "{cursor_gutter}{DIM}{subtitle}{RESET}").unwrap();
    }
}

/// What Enter will do for a branch-picker row (#185): the picker used to
/// give no hint whether selecting a branch spawns something new or jumps to
/// an existing tab. Classified by the caller (`State`, which already knows
/// about tabs and worktrees) and rendered as a dim suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchAnnotation {
    /// No tab and no worktree — Enter creates a fresh worktree and tab.
    Plain,
    /// A tab is already open for this branch — Enter jumps to it.
    Open,
    /// A worktree exists but has no tab yet — Enter reopens a tab for it.
    Worktree,
}

impl BranchAnnotation {
    fn suffix(self) -> &'static str {
        match self {
            BranchAnnotation::Plain => "",
            BranchAnnotation::Open => " (open)",
            BranchAnnotation::Worktree => " (worktree)",
        }
    }
}

pub fn render_branch_list(
    w: &mut impl Write,
    branches: &[String],
    annotations: &[BranchAnnotation],
    selected: usize,
    rows: usize,
    cols: usize,
    query: &str,
) {
    if branches.is_empty() {
        writeln!(w).unwrap();
        if query.is_empty() {
            writeln!(
                w,
                "  {DIM}No other branches — use i to create one.{RESET}"
            )
            .unwrap();
        } else {
            writeln!(w, "  {DIM}No branches match '{query}'.{RESET}").unwrap();
        }
        return;
    }

    // #196: the query rides on the title line, styled like InputBranch's
    // buffer display (`render_input`) — plain text plus an inverse-video
    // cursor block. Empty query renders exactly as before filtering existed.
    let title = if query.is_empty() {
        format!("  {BOLD}Select a branch:{RESET}")
    } else {
        format!("  {BOLD}Select a branch:{RESET} {query}{INVERSE} {RESET}")
    };
    writeln!(w).unwrap();
    writeln!(w, "{title}").unwrap();
    writeln!(w).unwrap();

    let max_visible = rows.saturating_sub(7).max(1);
    let start = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };
    // "  " cursor gutter, same width whether or not this row is selected.
    let available = cols.saturating_sub(2);

    for (idx, branch) in branches.iter().enumerate().skip(start).take(max_visible) {
        let cursor_gutter = if idx == selected {
            format!("{CYAN}▌{RESET} ")
        } else {
            "  ".to_string()
        };
        let suffix = annotations
            .get(idx)
            .copied()
            .unwrap_or(BranchAnnotation::Plain)
            .suffix();
        // Priority: full row, then drop the annotation, then clip the name
        // itself — never let a long annotated branch name wrap or overflow.
        let row = if !suffix.is_empty() && visible_width(branch) + visible_width(suffix) <= available
        {
            format!("{branch}{DIM}{suffix}{RESET}")
        } else if visible_width(branch) <= available {
            branch.clone()
        } else {
            clip_to_width(branch, available)
        };
        writeln!(w, "{cursor_gutter}{row}").unwrap();
    }
}

pub fn render_input(w: &mut impl Write, input: &str) {
    writeln!(w).unwrap();
    writeln!(w, "  {BOLD}New branch name:{RESET}").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "  > {input}{INVERSE} {RESET}").unwrap();
}

pub fn render_not_git_repo(w: &mut impl Write, cwd: &str) {
    writeln!(w).unwrap();
    writeln!(w, "  {BOLD}{RED}Not a git repository.{RESET}").unwrap();
    writeln!(w, "  {DIM}Current directory: {cwd}{RESET}").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "  {DIM}d{RESET}  save layout to disk").unwrap();
    writeln!(w, "  {DIM}x{RESET}  nuke session & start fresh").unwrap();
    writeln!(w, "  {DIM}q{RESET}  close plugin").unwrap();
}

/// The confirm dialog's optional disclosure line, drawn only when the
/// branch has an open tab (removal closes it): `  closes its tab`, with
/// ` (agent running)` appended when that tab's agent is actively working
/// (#188 — the plugin already knows both facts at confirm time, so this
/// reuses existing state rather than issuing new git calls). Narrow panes
/// drop the agent-running qualifier first, then the whole line, rather than
/// wrapping — kept in its own function so `confirm_dialog_lines` (used for
/// padding math) can never disagree with what `render_confirm` actually
/// draws.
fn confirm_tab_note(closes_tab: bool, agent_running: bool, cols: usize) -> Option<&'static str> {
    if !closes_tab {
        return None;
    }
    const FULL: &str = "  closes its tab (agent running)";
    const SHORT: &str = "  closes its tab";
    if agent_running && FULL.chars().count() <= cols {
        Some(FULL)
    } else if SHORT.chars().count() <= cols {
        Some(SHORT)
    } else {
        None
    }
}

/// Number of physical lines `render_confirm` draws when a branch is
/// selected — 4 plus 1 more if `confirm_tab_note` has something to show.
/// Callers computing footer padding must use this instead of a hardcoded
/// constant, so the layout never drifts from the actual render.
pub fn confirm_dialog_lines(closes_tab: bool, agent_running: bool, cols: usize) -> usize {
    4 + confirm_tab_note(closes_tab, agent_running, cols).is_some() as usize
}

pub fn render_confirm(w: &mut impl Write, branch: &str, cols: usize, closes_tab: bool, agent_running: bool) {
    // The prompt is `  Remove worktree for '<branch>'?` — 25 fixed chars plus
    // the branch name. At narrow widths the closing `'?` would otherwise
    // tumble onto a second line and read like garbage. Drop the prefix on
    // narrow panes; clip the branch with `…` if even that doesn't fit.
    writeln!(w).unwrap();
    let fixed = "  Remove worktree for ''?";
    let available = cols.saturating_sub(fixed.chars().count());
    if cols >= fixed.chars().count() + branch.chars().count().min(8) {
        let clipped = clip_to_width(branch, available);
        writeln!(
            w,
            "  {YELLOW}{BOLD}Remove worktree for '{clipped}'?{RESET}"
        )
        .unwrap();
    } else {
        let short_prefix = "  Remove '";
        let short_avail = cols.saturating_sub(short_prefix.chars().count() + 2);
        let clipped = clip_to_width(branch, short_avail);
        writeln!(w, "  {YELLOW}{BOLD}Remove '{clipped}'?{RESET}").unwrap();
    }
    if let Some(note) = confirm_tab_note(closes_tab, agent_running, cols) {
        writeln!(w, "{DIM}{note}{RESET}").unwrap();
    }
    writeln!(w).unwrap();
    writeln!(w, "  {DIM}y{RESET} confirm   {DIM}n/Esc{RESET} cancel").unwrap();
}

/// `browse_empty` and `browse_can_remove` are consulted only when `mode` is
/// `Mode::BrowseWorktrees`; every other mode ignores them (pass `false,
/// false`). `browse_empty` mirrors `State::should_render_empty_state()` —
/// the empty state's body already explains `i`/`n`, so the footer drops
/// down to just `r refresh` (#192: previously the full nav/create/remove
/// footer was shown even with nothing to navigate, open, or delete).
/// `browse_can_remove` mirrors `selected_sidebar_branch().is_some()`, the
/// same condition `handle_key_browse`'s `d` arm checks — the `d remove`
/// hint would otherwise promise an action that errors on the selected row
/// (e.g. a plain user tab with no matched branch).
pub fn render_footer(
    w: &mut impl Write,
    mode: &Mode,
    version: &str,
    cols: usize,
    browse_empty: bool,
    browse_can_remove: bool,
) {
    writeln!(w).unwrap();
    match mode {
        Mode::Loading => {}
        Mode::BrowseWorktrees => {
            if browse_empty {
                writeln!(w, "  {DIM}r{RESET} refresh").unwrap();
            } else {
                // #192: j/k still work (handle_key_browse never dropped
                // them) but the footer spends no characters on them —
                // arrows are the discoverable hint, same rationale #196
                // applied to the branch picker's j/k-as-filter-chars change
                // below. Two lines, both comfortably under the narrowest
                // tested sidebar width (30 cols: see
                // render_browse_with_wrapped_status_message), so unlike
                // SelectBranch's footer this one doesn't need a cols-gated
                // full/narrow wording split.
                writeln!(
                    w,
                    "  {DIM}↑/↓{RESET}  {DIM}Enter{RESET} open  {DIM}r{RESET} refresh"
                )
                .unwrap();
                if browse_can_remove {
                    writeln!(
                        w,
                        "  {DIM}n{RESET} pick  {DIM}i{RESET} new  {DIM}d{RESET} remove"
                    )
                    .unwrap();
                } else {
                    writeln!(w, "  {DIM}n{RESET} pick  {DIM}i{RESET} new").unwrap();
                }
            }
        }
        Mode::SelectBranch => {
            // #196: j/k are now filter characters, not navigation, so the
            // footer drops the vi-key hints in favor of arrows plus a
            // typing hint.
            if cols >= 50 {
                writeln!(
                    w,
                    "  {DIM}↑/↓{RESET} move  type to filter  \
                     {DIM}Enter{RESET} create  {DIM}Esc{RESET} back"
                )
                .unwrap();
            } else {
                writeln!(w, "  {DIM}↑/↓{RESET} move  type to filter").unwrap();
                writeln!(w, "  {DIM}Enter{RESET} create  {DIM}Esc{RESET} back").unwrap();
            }
        }
        Mode::InputBranch => {
            writeln!(w, "  {DIM}Enter{RESET} create  {DIM}Esc{RESET} back").unwrap();
        }
        Mode::NotGitRepo | Mode::Confirming => {}
    }
    let version_line = fit_text(version, cols.saturating_sub(2));
    // No trailing `\n` here: `render_footer` is always the very last thing
    // `render_to` writes for any Mode. Zellij's plugin pane is a real
    // terminal grid — after `\x1b[H` positions the cursor at row 0, writing
    // exactly `rows` newline-terminated lines advances the cursor through
    // `rows` row transitions, one past the last valid row, which forces a
    // scroll that silently discards row 0 (the header). This is the
    // verified root cause of #136 ("header never displays"): every prior
    // frame emitted exactly `rows` lines, ALL `writeln!`-terminated,
    // guaranteeing that scroll on every single render. Omitting the final
    // newline keeps the cursor on the last row instead of pushing it past
    // the bottom, so a frame that fills `rows` exactly no longer scrolls.
    write!(w, "  {DIM}{version_line}{RESET}").unwrap();
}

pub fn render_status(w: &mut impl Write, message: &str, is_error: bool, cols: usize) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w).unwrap();
    for line in wrap_status(message, cols) {
        writeln!(w, "{color}{line}{RESET}").unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_string(repo_name: &str, cols: usize) -> String {
        let mut buf = Vec::new();
        render_header(&mut buf, repo_name, cols);
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn header_shows_repo_name_only() {
        // #156: no brand prefix — the pane frame title carries the tool name.
        let h = header_string("my-service", 40);
        assert!(h.contains(" my-service "));
        assert!(!h.contains("zelligent"));
    }

    #[test]
    fn header_empty_name_falls_back_to_brand() {
        // Loading / not-a-repo arms pass "" — the fallback must be the brand,
        // never an empty rule (retro-review of #166).
        assert!(header_string("", 40).contains(" zelligent "));
    }

    #[test]
    fn header_repo_named_zelligent_gets_no_special_treatment() {
        // The old repo_name == "zelligent" special case is gone; pinned so a
        // future edit can't quietly reintroduce it (it would be invisible in
        // snapshots that use other repo names).
        assert!(header_string("zelligent", 40).contains(" zelligent "));
    }

    // --- wrap_status (#187: word-boundary wrap + hanging indent) ---

    // The two #187 issue examples were captured at a pane 34 characters of
    // content wide (the harness plan describes this as "36 cols" counting
    // the tmux capture's two border characters; the verbatim captured rows
    // — "  Only worktree tabs can be remove" / "d" and "here first, or
    // spawn a different b" / "ranch." — are each exactly 34 characters).

    #[test]
    fn wrap_status_breaks_on_word_boundaries_issue_example_one() {
        // Verbatim issue example: char-level wrap used to split
        // "removed" into "remove" / "d". Word-boundary wrap must keep
        // "removed" whole and give every line the same 2-space indent.
        let lines = wrap_status("Only worktree tabs can be removed", 34);
        assert_eq!(lines, vec!["  Only worktree tabs can be", "  removed"]);
        for line in &lines {
            assert!(line.starts_with("  "), "every line keeps the hanging indent");
        }
        assert!(
            lines.iter().all(|l| !l.trim().is_empty()),
            "no blank continuation line"
        );
    }

    #[test]
    fn wrap_status_breaks_on_word_boundaries_issue_example_two() {
        // Verbatim issue example: char-level wrap used to split "branch."
        // into "b" / "ranch." across the wrap point. This is the real
        // runtime message (zelligent.sh's "already checked out" refusal);
        // word-boundary wrap must never split "branch." either.
        let lines = wrap_status(
            "zelligent only opens isolated worktrees — check out another \
             branch there first, or spawn a different branch.",
            34,
        );
        assert_eq!(
            lines,
            vec![
                "  zelligent only opens isolated",
                "  worktrees — check out another",
                "  branch there first, or spawn a",
                "  different branch.",
            ]
        );
        for line in &lines {
            assert!(line.starts_with("  "), "every line keeps the hanging indent");
        }
        assert!(
            !lines.iter().any(|l| l.trim() == "ranch."),
            "'branch.' must never be split mid-word (#187)"
        );
    }

    #[test]
    fn wrap_status_hard_breaks_a_word_wider_than_the_available_width() {
        // A single word longer than the wrap width must still terminate
        // (no infinite loop) and never overflow a line past `cols`.
        let word = "supercalifragilisticexpialidocious";
        let lines = wrap_status(word, 10);
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(
                visible_width(line) <= 10,
                "line {line:?} overflows the 10-col width"
            );
        }
        // Hard-broken pieces reassemble into the original word losslessly.
        let rejoined: String = lines.iter().map(|l| l.trim_start()).collect();
        assert_eq!(rejoined, word);
    }

    #[test]
    fn wrap_status_never_loops_or_panics_on_degenerate_widths() {
        // cols smaller than the 2-space indent must not panic or hang.
        for cols in [0, 1, 2] {
            let lines = wrap_status("hello world", cols);
            assert!(!lines.is_empty());
        }
    }

    #[test]
    fn wrap_status_respects_unicode_display_width() {
        // Wide (double-column) CJK characters must be counted by display
        // width, not `char` count, consistent with `visible_width`
        // elsewhere in this module (e.g. `clip_to_width`).
        let message = "文字幅テスト文字幅テスト"; // 12 chars, each width 2 = 24 cols
        let lines = wrap_status(message, 10);
        // avail = 10 - 2 = 8 display columns => 4 wide chars per line.
        for line in &lines {
            assert!(
                visible_width(line) <= 10,
                "line {line:?} (width {}) overflows 10 cols",
                visible_width(line)
            );
        }
        assert!(lines.len() > 1, "message is wider than one line at cols=10");
    }

    #[test]
    fn wrap_status_empty_message_yields_no_lines() {
        assert_eq!(wrap_status("", 40), Vec::<String>::new());
    }

    #[test]
    fn status_wrap_rows_agrees_with_wrap_status_line_count() {
        // `status_wrap_rows` (feeding `status_height`, and from there
        // `sidebar_layout`) must always report the same row count
        // `wrap_status` actually produces — this is the #187 twin of the
        // #135/#136 render/mapper-agreement invariant.
        let message = "Only worktree tabs can be removed";
        for cols in [10, 18, 30, 34, 36, 80] {
            assert_eq!(status_wrap_rows(message, cols), wrap_status(message, cols).len());
        }
    }

    fn confirm_string(branch: &str, cols: usize, closes_tab: bool, agent_running: bool) -> String {
        let mut buf = Vec::new();
        render_confirm(&mut buf, branch, cols, closes_tab, agent_running);
        String::from_utf8(buf).unwrap()
    }

    // --- render_confirm / confirm_dialog_lines (#188) ---

    #[test]
    fn confirm_detached_worktree_has_no_tab_note() {
        // No open tab: today's plain two-line dialog, unchanged.
        let s = confirm_string("feat-a", 80, false, false);
        assert!(!s.contains("closes its tab"));
        assert_eq!(confirm_dialog_lines(false, false, 80), 4);
    }

    #[test]
    fn confirm_open_tab_discloses_it_closes() {
        let s = confirm_string("feat-a", 80, true, false);
        assert!(s.contains("closes its tab"));
        assert!(!s.contains("agent running"));
        assert_eq!(confirm_dialog_lines(true, false, 80), 5);
    }

    #[test]
    fn confirm_open_tab_with_working_agent_says_so() {
        let s = confirm_string("feat-a", 80, true, true);
        assert!(s.contains("closes its tab (agent running)"));
        assert_eq!(confirm_dialog_lines(true, true, 80), 5);
    }

    #[test]
    fn confirm_agent_running_ignored_without_open_tab() {
        // agent_running is meaningless without closes_tab (can't happen in
        // practice — no tab means no live agent — but the note must never
        // appear on its own).
        let s = confirm_string("feat-a", 80, false, true);
        assert!(!s.contains("closes its tab"));
        assert_eq!(confirm_dialog_lines(false, true, 80), 4);
    }

    #[test]
    fn confirm_narrow_pane_drops_agent_running_qualifier_before_the_whole_line() {
        // "  closes its tab (agent running)" is 33 chars; "  closes its
        // tab" is 17. At a width that fits the short form but not the long
        // one, drop the qualifier rather than wrapping or hiding the line
        // outright.
        let s = confirm_string("feat-a", 20, true, true);
        assert!(s.contains("closes its tab"));
        assert!(!s.contains("agent running"));
        assert_eq!(confirm_dialog_lines(true, true, 20), 5);
    }

    #[test]
    fn confirm_extremely_narrow_pane_drops_tab_note_entirely() {
        let s = confirm_string("feat-a", 10, true, true);
        assert!(!s.contains("closes its tab"));
        assert_eq!(confirm_dialog_lines(true, true, 10), 4);
    }

    // --- render_branch_list / BranchAnnotation (#185) ---

    fn branch_list_string(
        branches: &[String],
        annotations: &[BranchAnnotation],
        selected: usize,
        rows: usize,
        cols: usize,
    ) -> String {
        branch_list_string_with_query(branches, annotations, selected, rows, cols, "")
    }

    fn branch_list_string_with_query(
        branches: &[String],
        annotations: &[BranchAnnotation],
        selected: usize,
        rows: usize,
        cols: usize,
        query: &str,
    ) -> String {
        let mut buf = Vec::new();
        render_branch_list(&mut buf, branches, annotations, selected, rows, cols, query);
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn branch_list_empty_shows_helpful_hint_not_blank() {
        // A fresh repo's picker is empty once the checked-out branch is
        // suppressed (#185's CLI-side change) — a blank list reads like a
        // bug, so this must always say something actionable.
        let s = branch_list_string(&[], &[], 0, 20, 80);
        assert!(s.contains("No other branches"));
        assert!(s.contains('i'));
        assert!(!s.contains("No branches found"));
    }

    #[test]
    fn branch_list_plain_row_has_no_suffix() {
        let branches = vec!["feat-a".to_string()];
        let s = branch_list_string(&branches, &[BranchAnnotation::Plain], 0, 20, 80);
        assert!(s.contains("feat-a"));
        assert!(!s.contains('('));
    }

    #[test]
    fn branch_list_open_row_shows_open_suffix() {
        // The suffix is DIM-wrapped independently of the branch name, so the
        // ANSI code sits between them — assert each substring separately
        // rather than the concatenated "feat-a (open)".
        let branches = vec!["feat-a".to_string()];
        let s = branch_list_string(&branches, &[BranchAnnotation::Open], 0, 20, 80);
        assert!(s.contains("feat-a"));
        assert!(s.contains(" (open)"));
    }

    #[test]
    fn branch_list_worktree_row_shows_worktree_suffix() {
        let branches = vec!["feat-a".to_string()];
        let s = branch_list_string(&branches, &[BranchAnnotation::Worktree], 0, 20, 80);
        assert!(s.contains("feat-a"));
        assert!(s.contains(" (worktree)"));
    }

    #[test]
    fn branch_list_missing_annotation_defaults_to_plain() {
        // Defensive: a shorter annotations slice than branches (shouldn't
        // happen in practice, since lib.rs always zips them 1:1) must not
        // panic and must render as unannotated.
        let branches = vec!["feat-a".to_string()];
        let s = branch_list_string(&branches, &[], 0, 20, 80);
        assert!(s.contains("feat-a"));
        assert!(!s.contains('('));
    }

    // --- #196: filter query rendering ---

    #[test]
    fn branch_list_query_appears_on_title_line() {
        let branches = vec!["feat-a".to_string(), "feat-b".to_string()];
        let s = branch_list_string_with_query(
            &branches,
            &[BranchAnnotation::Plain, BranchAnnotation::Plain],
            0,
            20,
            80,
            "fe",
        );
        assert!(s.contains("Select a branch:"));
        assert!(s.contains("fe"));
    }

    #[test]
    fn branch_list_zero_matches_with_query_shows_no_branches_match() {
        // Distinct from the "no other branches at all" empty state (#185) —
        // this is a live query that narrowed the list to nothing.
        let s = branch_list_string_with_query(&[], &[], 0, 20, 80, "zzz");
        assert!(s.contains("No branches match 'zzz'"));
        assert!(!s.contains("No other branches"));
    }

    #[test]
    fn branch_list_empty_query_keeps_no_other_branches_hint() {
        // The truly-empty-repo state must still read "No other branches —
        // use i to create one." even though it now flows through the same
        // query-aware branch of `render_branch_list`.
        let s = branch_list_string_with_query(&[], &[], 0, 20, 80, "");
        assert!(s.contains("No other branches"));
    }

    #[test]
    fn branch_list_narrow_pane_drops_annotation_before_clipping_name() {
        // "feat-a (worktree)" is 18 chars; at a width that fits the bare
        // name but not the annotated form, drop the suffix rather than
        // truncating the name (mirrors confirm_tab_note's priority).
        let branches = vec!["feat-a".to_string()];
        let s = branch_list_string(&branches, &[BranchAnnotation::Worktree], 0, 20, 10);
        assert!(s.contains("feat-a"));
        assert!(!s.contains("worktree"));
        assert!(!s.contains('…'));
    }

    #[test]
    fn branch_list_extremely_narrow_pane_clips_name_with_ellipsis() {
        let branches = vec!["feature-really-long-branch-name".to_string()];
        let s = branch_list_string(&branches, &[BranchAnnotation::Open], 0, 20, 10);
        assert!(!s.contains("open"));
        assert!(s.contains('…'));
    }

    // --- render_footer (#192: state-aware browse footer) ---

    fn footer_string(mode: &Mode, cols: usize, browse_empty: bool, browse_can_remove: bool) -> String {
        let mut buf = Vec::new();
        render_footer(&mut buf, mode, "0.0.0-test", cols, browse_empty, browse_can_remove);
        String::from_utf8(buf).unwrap()
    }

    /// Strip ANSI SGR sequences (color/reset codes) so `visible_width` sees
    /// only the glyphs a terminal would actually lay out — otherwise the
    /// escape bytes themselves (e.g. the printable `2`/`m` in `\x1b[2m`)
    /// inflate the width `UnicodeWidthStr` reports.
    fn strip_ansi(line: &str) -> String {
        let mut out = String::new();
        let mut in_escape = false;
        for ch in line.chars() {
            if ch == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if ch.is_ascii_alphabetic() {
                    in_escape = false;
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// Every visible line of every browse-footer variant, at the narrowest
    /// sidebar width this file already exercises elsewhere for real renders
    /// (30 cols — see `render_browse_with_wrapped_status_message` in
    /// render_snapshots.rs), must fit without soft-wrapping. A line that
    /// overflows would silently desync the frame's row budget from what
    /// `sidebar_layout`/the empty-state arm actually reserve for it — the
    /// same class of bug #135/#136/#187 already fixed for other lines.
    #[test]
    fn browse_footer_variants_fit_narrow_sidebar_width() {
        const NARROW: usize = 30;
        for (empty, can_remove) in [(true, false), (false, false), (false, true)] {
            let s = footer_string(&Mode::BrowseWorktrees, NARROW, empty, can_remove);
            for line in s.lines().filter(|l| !l.is_empty()) {
                let width = visible_width(&strip_ansi(line));
                assert!(
                    width <= NARROW,
                    "line {line:?} (width {width}) overflows {NARROW} cols \
                     (empty={empty}, can_remove={can_remove})"
                );
            }
        }
    }

    #[test]
    fn browse_footer_empty_state_shows_only_refresh_hint() {
        // #192a: nothing to navigate/open/delete in the empty state — the
        // body already explains `i`/`n`, so the footer drops down to just
        // the refresh hint, no nav/Enter/create/delete hints.
        let s = footer_string(&Mode::BrowseWorktrees, 80, true, false);
        assert!(s.contains('r') && s.contains("refresh"));
        assert!(!s.contains("Enter"), "empty state has nothing to open");
        assert!(!s.contains('n'), "empty state's own body already covers branch picking");
        assert!(!s.contains('d'), "empty state has nothing to remove");
    }

    #[test]
    fn browse_footer_with_items_shows_nav_and_creation_hints() {
        let s = footer_string(&Mode::BrowseWorktrees, 80, false, false);
        assert!(s.contains("Enter"), "populated list must hint how to open an item");
        assert!(s.contains('r') && s.contains("refresh"));
        assert!(s.contains('n'), "must hint how to pick an existing branch");
        assert!(s.contains('i'), "must hint how to create a new branch");
    }

    #[test]
    fn browse_footer_shows_remove_hint_only_when_selection_is_removable() {
        // Mirrors the exact condition `handle_key_browse`'s 'd' arm checks
        // (`selected_sidebar_branch().is_some()`) — the hint must never
        // promise an action that would error ("Only worktree tabs can be
        // removed") on the currently selected row.
        let removable = footer_string(&Mode::BrowseWorktrees, 80, false, true);
        assert!(removable.contains("remove"), "removable selection: d remove must be hinted");

        let not_removable = footer_string(&Mode::BrowseWorktrees, 80, false, false);
        assert!(
            !not_removable.contains("remove"),
            "non-removable selection (e.g. a plain user tab): d remove must be hidden"
        );
    }
}
