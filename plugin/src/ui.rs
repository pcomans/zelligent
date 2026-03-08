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
use unicode_width::UnicodeWidthChar;

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

fn status_indicator(status: &AgentStatus) -> String {
    match status {
        AgentStatus::Idle => "  ".to_string(),
        AgentStatus::Working => format!("{GREEN}●{RESET} "),
        AgentStatus::Awaiting => format!("{YELLOW}●{RESET} "),
        AgentStatus::Done => format!("{GREEN}✓{RESET} "),
    }
}

fn subtitle_for_item(item: &SidebarItem) -> String {
    match &item.matched_branch {
        Some(branch) => format!("branch: {branch}"),
        None => "user tab".to_string(),
    }
}

fn kind_icon(item: &SidebarItem) -> String {
    if item.matched_branch.is_some() {
        format!("{CYAN}⎇{RESET} ")
    } else {
        format!("{DIM}□{RESET} ")
    }
}

fn clip_to_width(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    fn consume_ansi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
        out.push(chars.next().unwrap_or('\x1b'));
        if !matches!(chars.peek(), Some('[')) {
            return;
        }
        out.push(chars.next().unwrap());
        for c in chars.by_ref() {
            out.push(c);
            if ('@'..='~').contains(&c) {
                break;
            }
        }
    }

    let mut out = String::new();
    let mut used = 0usize;
    let mut chars = text.chars().peekable();
    let mut truncated = false;
    while let Some(c) = chars.peek().copied() {
        if c == '\x1b' {
            consume_ansi(&mut chars, &mut out);
            continue;
        }
        let c = chars.next().unwrap();
        let char_width = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + char_width > max_chars {
            truncated = true;
            break;
        }
        out.push(c);
        used += char_width;
    }
    if truncated && out.contains('\x1b') && !out.ends_with(RESET) {
        out.push_str(RESET);
    }
    out
}

fn visible_width(text: &str) -> usize {
    let mut width = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.peek().copied() {
        if c == '\x1b' {
            chars.next();
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for csi in chars.by_ref() {
                    if ('@'..='~').contains(&csi) {
                        break;
                    }
                }
            }
            continue;
        }
        let c = chars.next().unwrap();
        width += UnicodeWidthChar::width(c).unwrap_or(0);
    }
    width
}

fn fit_text(text: &str, width: usize) -> String {
    let truncated = clip_to_width(text, width);
    let pad = width.saturating_sub(visible_width(truncated.as_str()));
    format!("{truncated}{}", " ".repeat(pad))
}

pub fn render_header(w: &mut impl Write, repo_name: &str, tab_count: Option<usize>, cols: usize) {
    let repo_name = if repo_name.is_empty() {
        "repo"
    } else {
        repo_name
    };
    let title = match tab_count {
        Some(count) => format!(" zelligent · {} ({count}) ", repo_name),
        None => format!(" zelligent · {} ", repo_name),
    };
    let pad = cols.saturating_sub(title.len());
    writeln!(w, "{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad)).unwrap();
}

pub fn render_sidebar_list(
    w: &mut impl Write,
    items: &[SidebarItem],
    agent_statuses: &BTreeMap<String, AgentStatus>,
    selected: usize,
    rows: usize,
    cols: usize,
) {
    // Keep browse-mode footer pinned to the bottom:
    // header(1) + list_budget + status_strip(1) + footer(3) = total rows
    let list_line_budget = rows.saturating_sub(5);
    if list_line_budget == 0 {
        return;
    }

    if items.is_empty() {
        let mut lines_written = 0usize;
        let width = cols.saturating_sub(2);
        writeln!(w).unwrap();
        lines_written += 1;
        writeln!(
            w,
            "  {DIM}{}{RESET}",
            fit_text("No tabs in this session.", width)
        )
        .unwrap();
        lines_written += 1;
        writeln!(w, "  {DIM}{}{RESET}", fit_text("n: pick branch", width)).unwrap();
        lines_written += 1;
        writeln!(w, "  {DIM}{}{RESET}", fit_text("i: create branch", width)).unwrap();
        lines_written += 1;
        while lines_written < list_line_budget {
            writeln!(w).unwrap();
            lines_written += 1;
        }
        return;
    }

    let max_visible = list_line_budget.saturating_sub(1).saturating_div(2).max(1);
    let start = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };

    // "  " + left-accent char + row content
    let line_width = cols.saturating_sub(3);
    let row1_label_width = line_width.saturating_sub(4);
    let row2_subtitle_width = line_width.saturating_sub(4);

    let mut lines_written = 0usize;
    writeln!(w).unwrap();
    lines_written += 1;
    for (idx, item) in items.iter().enumerate().skip(start).take(max_visible) {
        let is_selected = idx == selected;
        let left = if is_selected { "▌" } else { " " };
        let wrap_start = if is_selected { INVERSE } else { "" };
        let wrap_end = if is_selected { RESET } else { "" };
        let indicator = status_indicator(
            agent_statuses
                .get(&item.tab_name)
                .unwrap_or(&AgentStatus::Idle),
        );
        let kind = kind_icon(item);
        let line1 = format!(
            "{indicator}{kind}{}",
            fit_text(&item.display_name, row1_label_width)
        );
        let line2 = format!(
            "    {}",
            fit_text(&subtitle_for_item(item), row2_subtitle_width)
        );
        writeln!(w, "  {left}{wrap_start}{line1}{wrap_end}",).unwrap();
        lines_written += 1;
        if is_selected {
            writeln!(w, "  {left}{wrap_start}{DIM}{line2}{RESET}{wrap_end}",).unwrap();
        } else {
            writeln!(w, "  {left}{DIM}{line2}{RESET}",).unwrap();
        }
        lines_written += 1;
    }
    while lines_written < list_line_budget {
        writeln!(w).unwrap();
        lines_written += 1;
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
    let width = cols.saturating_sub(2);
    match mode {
        Mode::Loading => {}
        Mode::BrowseWorktrees => {
            writeln!(
                w,
                "  {DIM}{}{RESET}",
                fit_text(
                    "jk move · Enter switch · n branch · i new · d remove · r refresh · q quit",
                    width
                )
            )
            .unwrap();
        }
        Mode::SelectBranch => {
            writeln!(
                w,
                "  {DIM}{}{RESET}",
                fit_text("j/k move  Enter create  Esc back", width)
            )
            .unwrap();
        }
        Mode::InputBranch => {
            writeln!(
                w,
                "  {DIM}{}{RESET}",
                fit_text("Enter create  Esc back", width)
            )
            .unwrap();
        }
        Mode::NotGitRepo | Mode::Confirming => {}
    }
    writeln!(w, "  {DIM}{}{RESET}", fit_text(version, width)).unwrap();
}

pub fn render_status(w: &mut impl Write, message: &str, is_error: bool) {
    if message.is_empty() {
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w).unwrap();
    writeln!(w, "  {color}{message}{RESET}").unwrap();
}

pub fn render_status_strip(w: &mut impl Write, message: &str, is_error: bool, cols: usize) {
    let width = cols.saturating_sub(2);
    if message.is_empty() {
        writeln!(w, "  {DIM}{}{RESET}", fit_text("ready", width)).unwrap();
        return;
    }
    let color = if is_error { RED } else { GREEN };
    writeln!(w, "  {color}{}{RESET}", fit_text(message, width)).unwrap();
}

#[cfg(test)]
mod tests {
    use super::{clip_to_width, fit_text, visible_width, CYAN, RESET};

    #[test]
    fn clip_to_width_respects_ascii_width() {
        assert_eq!(clip_to_width("feature-super-long-branch", 7), "feature");
    }

    #[test]
    fn clip_to_width_respects_terminal_cell_width() {
        assert_eq!(clip_to_width("ab漢cd", 4), "ab漢");
    }

    #[test]
    fn fit_text_pads_to_requested_width() {
        let fitted = fit_text("abc", 6);
        assert_eq!(fitted, "abc   ");
    }

    #[test]
    fn fit_text_ignores_ansi_sequences_for_padding() {
        let styled = format!("{CYAN}abc{RESET}");
        let fitted = fit_text(&styled, 6);
        assert_eq!(visible_width(&fitted), 6);
    }

    #[test]
    fn clip_to_width_preserves_ansi_sequence_integrity() {
        let styled = format!("{CYAN}abcdef{RESET}");
        let clipped = clip_to_width(&styled, 3);
        assert!(clipped.contains(CYAN));
        assert!(clipped.contains(RESET));
        assert_eq!(visible_width(&clipped), 3);
    }
}
