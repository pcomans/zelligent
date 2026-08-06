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
    /// Whether the ` <repo> ` header banner line is drawn this frame. Since
    /// #218 the sidebar pane is `borderless=true`, so this in-pane banner is
    /// the sidebar's sole title bar (there is no Zellij frame title above it);
    /// it carries the repo name only (#156).
    pub show_header: bool,
    /// Whether a blank separator line is drawn between the header (or the
    /// pane top, if the header itself was dropped) and the first item.
    pub show_separator: bool,
    /// Physical rows the persistent `stale · retrying` marker occupies this
    /// frame (issue #216): 1 when the worktree cache is flagged stale, else
    /// 0. Drawn between the header and the separator, so it counts toward
    /// `leading_lines()` and the mouse-click item offset.
    pub stale_lines: usize,
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
        self.show_header as usize + self.stale_lines + self.show_separator as usize
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

/// How many physical rows a status message will occupy (its content line
/// only, not the leading blank line before it), accounting for wrap at the
/// real pane width. `render_status` prints `"  {message}"`, so the wrap
/// width is the message's visible width plus the 2-space prefix.
fn status_wrap_rows(message: &str, cols: usize) -> usize {
    if cols == 0 {
        return 1;
    }
    let width = 2 + visible_width(message);
    width.div_ceil(cols).max(1)
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
    stale: bool,
) -> SidebarLayout {
    let status_lines = status_height(status_message, cols);
    // The persistent stale marker (issue #216) is reserved up front, ahead
    // of the header/separator degradation decision: staleness is more
    // important to convey than the repo-name banner, so on a short pane the
    // marker survives while the header/separator are sacrificed first.
    let stale_lines = usize::from(stale);

    // Footer degradation for undersized panes (issue #216 finding 5). The
    // full footer (blank + hints + version) is 3–4 rows; on a pane too short
    // to also fit the status, the marker, and at least one two-row item, it
    // collapses to the single version line so those never overflow `rows`
    // and force the header-swallowing scroll (#135/#136). One line is always
    // kept as the terminal's bottom anchor (see `render_footer`), so the
    // final write stays a newline-less `write!`.
    let full_footer = if cols >= 55 { 3 } else { 4 };
    let overhead = status_lines + stale_lines;
    let footer_lines = if rows >= overhead + 2 + full_footer {
        full_footer
    } else {
        1
    };

    let content_budget = rows.saturating_sub(overhead + footer_lines);
    let (show_header, show_separator) = if content_budget >= MIN_ROWS_HEADER_AND_SEPARATOR {
        (true, true)
    } else if content_budget >= MIN_ROWS_HEADER_ONLY {
        (true, false)
    } else {
        (false, false)
    };

    let leading = show_header as usize + show_separator as usize;
    let item_rows_budget = content_budget.saturating_sub(leading);
    // No `.max(1)` here (finding 5): on a truly tiny pane the item viewport
    // may legitimately shrink to zero rather than overflow. The full-footer
    // guard above still reserves room for one item on any normally-sized
    // pane, so the frozen "an item row is never sacrificed" rule (#135/#136)
    // holds for every case a user actually sees.
    let max_items = item_rows_budget / 2;
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
        stale_lines,
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

// This in-pane header is the sidebar's ONLY title bar. The sidebar pane is
// `borderless=true` in the layout (#218), so Zellij draws no pane frame and
// hence no frame title — earlier there were TWO stacked title bars, the
// "zelligent" frame title directly above this header (the "double header").
// #156 tried to differentiate them by dropping this line's brand prefix (so
// the frame said "zelligent" and this said the repo name), but two ruled
// title bars stacked still read as doubled; #218 removes the frame instead.
// The header carries the repo name; the brand lives in the footer next to
// the version. An empty repo name (state not loaded yet; error arms pass
// their own label) falls back to the brand so there is always a title.
pub fn render_header(w: &mut impl Write, repo_name: &str, cols: usize) {
    let title = if repo_name.is_empty() {
        " zelligent ".to_string()
    } else {
        format!(" {repo_name} ")
    };
    let pad = cols.saturating_sub(visible_width(&title));
    writeln!(w, "{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad)).unwrap();
}

/// Persistent `stale · retrying` marker (issue #216). Drawn between the
/// header and the item list whenever `State::is_stale()` holds. Yellow (a
/// warning, not the red alarm of a hard error) and carries the short `reason`
/// inline. When `has_details` (a failed refresh has full error text
/// recoverable via the `e` key) it advertises `e: details`; a bare
/// unsatisfied-invalidation marker omits the hint since there's nothing extra
/// to show. Exactly one physical line: the whole string is clipped to the
/// pane width (prefix included) so it can never wrap and throw off the layout
/// arithmetic. The hint is dropped first when width is tight, so the reason
/// survives on a narrow pane.
pub fn render_stale_marker(w: &mut impl Write, reason: &str, has_details: bool, cols: usize) {
    let base = format!("⚠ stale · retrying — {reason}");
    let content = if has_details {
        let with_hint = format!("{base} · e: details");
        if visible_width(&with_hint) + 2 <= cols {
            with_hint
        } else {
            base
        }
    } else {
        base
    };
    let clipped = clip_to_width(&content, cols.saturating_sub(2));
    writeln!(w, "  {YELLOW}{clipped}{RESET}").unwrap();
}

/// The version-only footer for an undersized pane (issue #216 finding 5):
/// the single bottom-anchor line `render_footer` always ends with, without
/// the leading blank or the command hints there's no room for. Newline-less
/// `write!` like `render_footer`, so a frame that fills `rows` exactly does
/// not scroll (#135/#136).
pub fn render_version_only(w: &mut impl Write, version: &str, cols: usize) {
    let version_line = fit_text(version, cols.saturating_sub(2));
    write!(w, "  {DIM}{version_line}{RESET}").unwrap();
}

/// Render the empty-state body within a `budget_rows` PHYSICAL-row budget at
/// `cols` width, returning how many physical rows it actually wrote (issue
/// #216 finding 5, third pass). Each line's physical height accounts for
/// wrapping at `cols` (a wide instruction line is two rows at a narrow width),
/// so the count the caller pads against equals what the terminal draws — the
/// earlier logical-line count silently overflowed. When everything fits, the
/// full six-line body (blank separators included) renders unchanged. When it
/// doesn't, the blank separators are dropped first and the meaningful content
/// lines are kept in visual order, each included only if its wrapped height
/// still fits — so "No managed worktrees yet." never disappears while a blank
/// survives (the non-monotonic bug), and the headline is preferred over the
/// lower hints.
pub fn render_empty_state(w: &mut impl Write, budget_rows: usize, cols: usize) -> usize {
    // (visible text for width, styled line, is_blank) in visual order.
    let items: [(&str, String, bool); 6] = [
        ("", String::new(), true),
        ("  No managed worktrees yet.", format!("  {BOLD}No managed worktrees yet.{RESET}"), false),
        (
            "  Pick a branch or type a new one to get started.",
            format!("  {DIM}Pick a branch or type a new one to get started.{RESET}"),
            false,
        ),
        ("", String::new(), true),
        ("  n  pick an existing branch", format!("  {DIM}n{RESET}  pick an existing branch"), false),
        ("  i  type a new branch name", format!("  {DIM}i{RESET}  type a new branch name"), false),
    ];
    let height = |plain: &str| -> usize {
        if cols == 0 {
            1
        } else {
            visible_width(plain).max(1).div_ceil(cols)
        }
    };

    let total: usize = items.iter().map(|(plain, _, _)| height(plain)).sum();
    let mut written = 0;
    if total <= budget_rows {
        // Everything fits — render the full body verbatim.
        for (plain, styled, _) in &items {
            writeln!(w, "{styled}").unwrap();
            written += height(plain);
        }
    } else {
        // Tight: drop the blank separators, keep meaningful lines in visual
        // order, each only if its wrapped height still fits the budget.
        for (plain, styled, is_blank) in &items {
            if *is_blank {
                continue;
            }
            let h = height(plain);
            if written + h > budget_rows {
                continue;
            }
            writeln!(w, "{styled}").unwrap();
            written += h;
        }
    }
    written
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

pub fn render_branch_list(w: &mut impl Write, branches: &[String], selected: usize, rows: usize) {
    if branches.is_empty() {
        writeln!(w).unwrap();
        writeln!(w, "  {DIM}No branches found.{RESET}").unwrap();
        return;
    }

    let title = format!("  {BOLD}Select a branch:{RESET}");
    writeln!(w).unwrap();
    writeln!(w, "{title}").unwrap();
    writeln!(w).unwrap();

    let max_visible = rows.saturating_sub(7).max(1);
    let start = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };

    for (idx, branch) in branches.iter().enumerate().skip(start).take(max_visible) {
        let cursor_gutter = if idx == selected {
            format!("{CYAN}▌{RESET} ")
        } else {
            "  ".to_string()
        };
        writeln!(w, "{cursor_gutter}{branch}").unwrap();
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

pub fn render_confirm(w: &mut impl Write, branch: &str, cols: usize) {
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
    writeln!(w).unwrap();
    writeln!(w, "  {DIM}y{RESET} confirm   {DIM}n/Esc{RESET} cancel").unwrap();
}

pub fn render_footer(w: &mut impl Write, mode: &Mode, version: &str, cols: usize) {
    writeln!(w).unwrap();
    match mode {
        Mode::Loading => {}
        Mode::BrowseWorktrees => {
            if cols >= 55 {
                writeln!(
                    w,
                    "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} open  \
                     {DIM}n{RESET} branch  {DIM}i{RESET} new  {DIM}d{RESET} remove  \
                     {DIM}r{RESET} refresh"
                )
                .unwrap();
            } else {
                writeln!(
                    w,
                    "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} open"
                )
                .unwrap();
                writeln!(
                    w,
                    "  {DIM}n{RESET} branch  {DIM}i{RESET} new  {DIM}d{RESET} del  {DIM}r{RESET} ↻"
                )
                .unwrap();
            }
        }
        Mode::SelectBranch => {
            if cols >= 44 {
                writeln!(
                    w,
                    "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  \
                     {DIM}Enter{RESET} create  {DIM}Esc{RESET} back"
                )
                .unwrap();
            } else {
                writeln!(
                    w,
                    "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} create"
                )
                .unwrap();
                writeln!(w, "  {DIM}Esc{RESET} back").unwrap();
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

pub fn render_status(w: &mut impl Write, message: &str, is_error: bool) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w).unwrap();
    writeln!(w, "  {color}{message}{RESET}").unwrap();
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
        // #156/#218: no brand prefix — this is the sole (borderless) header,
        // carrying the repo name; the brand lives in the footer.
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
}
