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

pub fn sidebar_viewport(selected: usize, rows: usize, total_items: usize) -> SidebarViewport {
    let lines_per_item = 2;
    let max_items = (rows.saturating_sub(5) / lines_per_item).max(1);
    let start = if selected >= max_items {
        selected - max_items + 1
    } else {
        0
    };

    SidebarViewport {
        start,
        max_items,
        visible_items: total_items.saturating_sub(start).min(max_items),
    }
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

pub fn render_header(w: &mut impl Write, repo_name: &str, cols: usize) {
    let title = if repo_name.is_empty() || repo_name == "zelligent" {
        " zelligent ".to_string()
    } else {
        format!(" zelligent / {repo_name} ")
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
    writeln!(w, "  {DIM}n{RESET}  pick an existing branch").unwrap();
    writeln!(w, "  {DIM}i{RESET}  type a new branch name").unwrap();
}

pub fn render_sidebar_list(
    w: &mut impl Write,
    items: &[SidebarItem],
    agent_statuses: &BTreeMap<String, AgentStatus>,
    repo_name: &str,
    active_tab_name: Option<&str>,
    selected: usize,
    rows: usize,
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
    let viewport = sidebar_viewport(selected, rows, items.len());
    let content_width = cols.saturating_sub(4).max(1);

    writeln!(w).unwrap();
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
    writeln!(w, "  {DIM}{version_line}{RESET}").unwrap();
}

pub fn render_status(w: &mut impl Write, message: &str, is_error: bool) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w).unwrap();
    writeln!(w, "  {color}{message}{RESET}").unwrap();
}
