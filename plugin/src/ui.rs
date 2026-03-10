// ANSI escape helpers
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RESET: &str = "\x1b[0m";
pub const INVERSE: &str = "\x1b[7m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const CYAN: &str = "\x1b[36m";
pub const YELLOW: &str = "\x1b[33m";
pub const BG_CYAN: &str = "\x1b[46m";
pub const FG_BLACK: &str = "\x1b[30m";

use std::collections::BTreeMap;
use std::io::Write;

use unicode_width::UnicodeWidthStr;

use crate::{AgentStatus, Mode, SidebarItem, Worktree};

/// Sanitize a branch name to match the shell's tab/session name logic:
/// replace `/` with `-`, then strip anything outside `[A-Za-z0-9_-]`.
pub fn sanitize_tab_name(branch: &str) -> String {
    branch
        .replace('/', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

/// Visible width of a string (accounts for wide/emoji chars).
pub fn visible_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Clip a string to fit within `max_width` visible columns, adding "…" if truncated.
pub fn clip_to_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + cw > max_width.saturating_sub(1) {
            // Need room for ellipsis
            if width < max_width {
                result.push('…');
            }
            return result;
        }
        result.push(ch);
        width += cw;
    }
    result
}

/// Pad or truncate text to exactly `width` visible columns.
pub fn fit_text(s: &str, width: usize) -> String {
    let vis = visible_width(s);
    if vis <= width {
        format!("{}{}", s, " ".repeat(width - vis))
    } else {
        clip_to_width(s, width)
    }
}

fn status_indicator(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Idle => "  ",
        AgentStatus::Working => "● ",
        AgentStatus::Awaiting => "● ",
        AgentStatus::Done => "✓ ",
    }
}

fn status_color(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Idle => "",
        AgentStatus::Working => GREEN,
        AgentStatus::Awaiting => YELLOW,
        AgentStatus::Done => GREEN,
    }
}

pub fn render_header(w: &mut impl Write, repo_name: &str, cols: usize) {
    let title = format!(" {} ", repo_name);
    let pad = cols.saturating_sub(visible_width(&title));
    writeln!(w, "{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad)).unwrap();
}

/// Render the sidebar list from sidebar_items (two-line rows: name + subtitle).
pub fn render_sidebar_list(
    w: &mut impl Write,
    items: &[SidebarItem],
    agent_statuses: &BTreeMap<String, AgentStatus>,
    selected: usize,
    rows: usize,
    cols: usize,
) {
    if items.is_empty() {
        writeln!(w).unwrap();
        writeln!(w, "  {CYAN}  ▄▄▄▄▄▄▄▄      ▄▄ ▄▄{RESET}").unwrap();
        writeln!(w, "  {CYAN} █▀▀▀▀▀██▀       ██ ██                      █▄{RESET}").unwrap();
        writeln!(w, "  {CYAN}      ▄█▀        ██ ██ ▀▀    ▄▄       ▄    ▄██▄{RESET}").unwrap();
        writeln!(w, "  {CYAN}    ▄█▀    ▄█▀█▄ ██ ██ ██ ▄████ ▄█▀█▄ ████▄ ██{RESET}").unwrap();
        writeln!(w, "  {CYAN}  ▄█▀    ▄ ██▄█▀ ██ ██ ██ ██ ██ ██▄█▀ ██ ██ ██{RESET}").unwrap();
        writeln!(w, "  {CYAN} ████████▀▄▀█▄▄▄▄██▄██▄██▄▀████▄▀█▄▄▄▄██ ▀█▄██{RESET}").unwrap();
        writeln!(w, "  {CYAN}                             ██{RESET}").unwrap();
        writeln!(w, "  {CYAN}                           ▀▀▀{RESET}").unwrap();
        writeln!(w).unwrap();
        writeln!(w, "  {DIM}n{RESET}  pick an existing branch").unwrap();
        writeln!(w, "  {DIM}i{RESET}  type a new branch name").unwrap();
        return;
    }

    // Two lines per item, reserve space for header, footer, status
    let lines_per_item = 2;
    let max_items = rows.saturating_sub(5).max(1) / lines_per_item;
    let start = if selected >= max_items {
        selected - max_items + 1
    } else {
        0
    };

    let content_width = cols.saturating_sub(4); // 2 left margin + 2 right

    writeln!(w).unwrap();
    for (idx, item) in items.iter().enumerate().skip(start).take(max_items) {
        let is_selected = idx == selected;
        let status = agent_statuses
            .get(&item.tab_name)
            .unwrap_or(&AgentStatus::Idle);
        let ind = status_indicator(status);
        let sc = status_color(status);

        // Line 1: status indicator + display name
        let name = fit_text(&item.display_name, content_width.saturating_sub(2));
        if is_selected {
            writeln!(w, "  {sc}{ind}{RESET}{INVERSE} {name} {RESET}").unwrap();
        } else {
            writeln!(w, "  {sc}{ind}{RESET} {name} ", ).unwrap();
        }

        // Line 2: subtitle (branch or "user tab")
        let subtitle = match &item.matched_branch {
            Some(branch) => format!("branch: {branch}"),
            None => "user tab".to_string(),
        };
        let subtitle = fit_text(&subtitle, content_width);
        if is_selected {
            writeln!(w, "    {DIM}{INVERSE} {subtitle} {RESET}").unwrap();
        } else {
            writeln!(w, "    {DIM} {subtitle} {RESET}").unwrap();
        }
    }
}

/// Render worktree list (legacy, used when sidebar_items are empty).
pub fn render_worktree_list(w: &mut impl Write, worktrees: &[Worktree], agent_statuses: &BTreeMap<String, AgentStatus>, selected: usize, rows: usize) {
    if worktrees.is_empty() {
        writeln!(w).unwrap();
        writeln!(w, "  {CYAN}  ▄▄▄▄▄▄▄▄      ▄▄ ▄▄{RESET}").unwrap();
        writeln!(w, "  {CYAN} █▀▀▀▀▀██▀       ██ ██                      █▄{RESET}").unwrap();
        writeln!(w, "  {CYAN}      ▄█▀        ██ ██ ▀▀    ▄▄       ▄    ▄██▄{RESET}").unwrap();
        writeln!(w, "  {CYAN}    ▄█▀    ▄█▀█▄ ██ ██ ██ ▄████ ▄█▀█▄ ████▄ ██{RESET}").unwrap();
        writeln!(w, "  {CYAN}  ▄█▀    ▄ ██▄█▀ ██ ██ ██ ██ ██ ██▄█▀ ██ ██ ██{RESET}").unwrap();
        writeln!(w, "  {CYAN} ████████▀▄▀█▄▄▄▄██▄██▄██▄▀████▄▀█▄▄▄▄██ ▀█▄██{RESET}").unwrap();
        writeln!(w, "  {CYAN}                             ██{RESET}").unwrap();
        writeln!(w, "  {CYAN}                           ▀▀▀{RESET}").unwrap();
        writeln!(w).unwrap();
        writeln!(w, "  {DIM}n{RESET}  pick an existing branch").unwrap();
        writeln!(w, "  {DIM}i{RESET}  type a new branch name").unwrap();
        return;
    }

    let max_visible = rows.saturating_sub(5).max(1); // header + footer + margins
    let start = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };

    writeln!(w).unwrap();
    for (idx, wt) in worktrees.iter().enumerate().skip(start).take(max_visible) {
        let cursor = if idx == selected { INVERSE } else { "" };
        let tab_name = sanitize_tab_name(&wt.branch);
        let status = agent_statuses.get(&tab_name).unwrap_or(&AgentStatus::Idle);
        let ind = status_indicator(status);
        let sc = status_color(status);
        if wt.dir != wt.branch {
            writeln!(w, "  {sc}{ind}{RESET}{cursor} {dir} {RESET}  {DIM}({branch}){RESET}", dir = wt.dir, branch = wt.branch).unwrap();
        } else {
            writeln!(w, "  {sc}{ind}{RESET}{cursor} {dir} {RESET}", dir = wt.dir).unwrap();
        }
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
        let cursor = if idx == selected { INVERSE } else { "" };
        writeln!(w, "  {cursor} {branch} {RESET}").unwrap();
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

pub fn render_confirm(w: &mut impl Write, branch: &str) {
    writeln!(w).unwrap();
    writeln!(w, "  {YELLOW}{BOLD}Remove worktree for '{branch}'?{RESET}").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "  {DIM}y{RESET} confirm   {DIM}n/Esc{RESET} cancel").unwrap();
}

pub fn render_footer(w: &mut impl Write, mode: &Mode, version: &str, cols: usize) {
    writeln!(w).unwrap();
    match mode {
        Mode::Loading => {}
        Mode::BrowseWorktrees => {
            writeln!(
                w,
                "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} open  \
                 {DIM}n{RESET} branch  {DIM}i{RESET} new  {DIM}d{RESET} remove  \
                 {DIM}r{RESET} refresh"
            )
            .unwrap();
        }
        Mode::SelectBranch => {
            writeln!(
                w,
                "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  \
                 {DIM}Enter{RESET} create  {DIM}Esc{RESET} back"
            )
            .unwrap();
        }
        Mode::InputBranch => {
            writeln!(w, "  {DIM}Enter{RESET} create  {DIM}Esc{RESET} back").unwrap();
        }
        Mode::NotGitRepo | Mode::Confirming => {}
    }
    let ver_line = fit_text(version, cols.saturating_sub(2));
    writeln!(w, "  {DIM}{ver_line}{RESET}").unwrap();
}

pub fn render_status(w: &mut impl Write, message: &str, is_error: bool) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w).unwrap();
    writeln!(w, "  {color}{message}{RESET}").unwrap();
}
