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

use crate::{AgentStatus, Mode, SidebarItem};

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
    // Check if the string already fits
    if visible_width(s) <= max_width {
        return s.to_string();
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
    active_tab_name: Option<&str>,
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
        let is_active = active_tab_name.map_or(false, |name| name == item.tab_name);
        
        let status = agent_statuses
            .get(&item.tab_name)
            .unwrap_or(&AgentStatus::Idle);
        let ind = status_indicator(status);
        let sc = status_color(status);

        // Line 1: status indicator + display name with Powerline arrow
        let name_width = content_width.saturating_sub(4);
        let name = fit_text(&item.display_name, name_width);
        let is_user_tab = item.matched_branch.is_none();
        let ind_str = if is_user_tab { "  " } else { status_indicator(status) };
        
        if is_active {
            if is_selected {
                // High contrast active/selected
                write!(w, "  {BG_CYAN}{FG_BLACK}{ind_str}{name} {RESET}{CYAN}{RESET}").unwrap();
            } else {
                // Active but not focused: Cyan text with solid caret
                write!(w, "  {BOLD}{CYAN}{ind_str}{name} {RESET}{CYAN}{RESET}").unwrap();
            }
        } else if is_selected {
            // Selected but not active: Inverted background with solid caret
            write!(w, "  {INVERSE}{ind_str}{name} {RESET}{RESET}").unwrap();
        } else {
            // Idle
            let sc = status_color(status);
            write!(w, "  {sc}{ind_str}{RESET}{name} ").unwrap();
        }
        writeln!(w).unwrap();

        // Line 2: subtitle (branch or "user tab")
        // Line 2 is NEVER highlighted, just colored to match the state
        let subtitle_text = match &item.matched_branch {
            Some(branch) => format!("branch: {branch}"),
            None => "user tab".to_string(),
        };
        let subtitle = fit_text(&subtitle_text, content_width);
        
        if is_active {
            writeln!(w, "    {CYAN}{subtitle}{RESET}").unwrap();
        } else {
            writeln!(w, "    {DIM}{subtitle}{RESET}").unwrap();
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
            // Two-line footer for narrow sidebar widths
            if cols >= 55 {
                writeln!(
                    w,
                    "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} open  \
                     {DIM}n{RESET} branch  {DIM}i{RESET} new  {DIM}d{RESET} remove  \
                     {DIM}r{RESET} refresh"
                )
                .unwrap();
            } else {
                writeln!(w, "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} open").unwrap();
                writeln!(w, "  {DIM}n{RESET} branch  {DIM}i{RESET} new  {DIM}d{RESET} del  {DIM}r{RESET} refresh").unwrap();
            }
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
