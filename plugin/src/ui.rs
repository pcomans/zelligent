// ANSI escape helpers
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RESET: &str = "\x1b[0m";
pub const INVERSE: &str = "\x1b[7m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const CYAN: &str = "\x1b[36m";
pub const YELLOW: &str = "\x1b[33m";

use crate::{Mode, Worktree};

pub fn render_header(repo_name: &str, cols: usize) {
    let title = format!(" spawn-agent: {} ", repo_name);
    let pad = cols.saturating_sub(title.len());
    println!("{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad));
}

pub fn render_worktree_list(worktrees: &[Worktree], selected: usize, rows: usize) {
    if worktrees.is_empty() {
        println!();
        println!("  {DIM}No worktrees found.{RESET}");
        println!("  {DIM}Press {RESET}n{DIM} to pick a branch or {RESET}i{DIM} to type one.{RESET}");
        return;
    }

    let max_visible = rows.saturating_sub(5).max(1); // header + footer + margins
    let start = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };

    println!();
    for (idx, wt) in worktrees.iter().enumerate().skip(start).take(max_visible) {
        let cursor = if idx == selected { INVERSE } else { "" };
        let branch_display = &wt.branch;
        println!("  {cursor} {branch_display} {RESET}");
    }
}

pub fn render_branch_list(branches: &[String], selected: usize, rows: usize) {
    if branches.is_empty() {
        println!();
        println!("  {DIM}No branches found.{RESET}");
        return;
    }

    let title = format!("  {BOLD}Select a branch:{RESET}");
    println!();
    println!("{title}");
    println!();

    let max_visible = rows.saturating_sub(7).max(1);
    let start = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };

    for (idx, branch) in branches.iter().enumerate().skip(start).take(max_visible) {
        let cursor = if idx == selected { INVERSE } else { "" };
        println!("  {cursor} {branch} {RESET}");
    }
}

pub fn render_input(input: &str) {
    println!();
    println!("  {BOLD}New branch name:{RESET}");
    println!();
    println!("  > {input}{INVERSE} {RESET}");
}

pub fn render_confirm(branch: &str) {
    println!();
    println!("  {YELLOW}{BOLD}Remove worktree for '{branch}'?{RESET}");
    println!();
    println!("  {DIM}y{RESET} confirm   {DIM}n/Esc{RESET} cancel");
}

pub fn render_footer(mode: &Mode) {
    println!();
    match mode {
        Mode::Loading => {}
        Mode::BrowseWorktrees => {
            println!(
                "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  {DIM}Enter{RESET} open  \
                 {DIM}n{RESET} branch  {DIM}i{RESET} new  {DIM}d{RESET} remove  \
                 {DIM}r{RESET} refresh  {DIM}q{RESET} quit"
            );
        }
        Mode::SelectBranch => {
            println!(
                "  {DIM}↑/k{RESET} up  {DIM}↓/j{RESET} down  \
                 {DIM}Enter{RESET} create  {DIM}Esc{RESET} back"
            );
        }
        Mode::InputBranch => {
            println!("  {DIM}Enter{RESET} create  {DIM}Esc{RESET} back");
        }
        Mode::Confirming => {}
    }
}

pub fn render_status(message: &str, is_error: bool) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    println!();
    println!("  {color}{message}{RESET}");
}
