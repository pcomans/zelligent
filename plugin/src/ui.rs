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

use crate::{AgentStatus, Mode, Worktree};

/// Sanitize a branch name to match the shell's tab/session name logic:
/// replace `/` with `-`, then strip anything outside `[A-Za-z0-9_-]`.
pub fn sanitize_tab_name(branch: &str) -> String {
    branch
        .replace('/', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

fn status_indicator(status: &AgentStatus) -> String {
    match status {
        AgentStatus::Idle => "  ".to_string(),
        AgentStatus::Working => format!("{GREEN}● {RESET}"),
        AgentStatus::NeedsInput => format!("{YELLOW}● {RESET}"),
        AgentStatus::Done => format!("{GREEN}✓ {RESET}"),
    }
}

pub fn render_header(w: &mut impl Write, repo_name: &str, cols: usize) {
    let title = format!(" zelligent: {} ", repo_name);
    let pad = cols.saturating_sub(title.len());
    writeln!(w, "{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad)).unwrap();
}

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
        let indicator = status_indicator(
            agent_statuses.get(&tab_name).unwrap_or(&AgentStatus::Idle),
        );
        if wt.dir != wt.branch {
            writeln!(w, "  {indicator}{cursor} {dir} {RESET}  {DIM}({branch}){RESET}", dir = wt.dir, branch = wt.branch).unwrap();
        } else {
            writeln!(w, "  {indicator}{cursor} {dir} {RESET}", dir = wt.dir).unwrap();
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
    writeln!(w, "  {DIM}q/Esc{RESET}  ignored (persistent sidebar)").unwrap();
}

pub fn render_confirm(w: &mut impl Write, branch: &str) {
    writeln!(w).unwrap();
    writeln!(w, "  {YELLOW}{BOLD}Remove worktree for '{branch}'?{RESET}").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "  {DIM}y{RESET} confirm   {DIM}n/Esc{RESET} cancel").unwrap();
}

pub fn render_footer(w: &mut impl Write, mode: &Mode, version: &str) {
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
    writeln!(w, "  {DIM}{version}{RESET}").unwrap();
}

pub fn render_status(w: &mut impl Write, message: &str, is_error: bool) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w).unwrap();
    writeln!(w, "  {color}{message}{RESET}").unwrap();
}
