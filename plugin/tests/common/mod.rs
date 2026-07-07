use std::collections::{BTreeMap, BTreeSet};
use zelligent_plugin::{Mode, State, Worktree};
use zellij_tile::prelude::*;

#[allow(dead_code)]
pub fn make_pipe_msg(name: &str, args: &[(&str, &str)]) -> PipeMessage {
    let mut map = BTreeMap::new();
    for (k, v) in args {
        map.insert(k.to_string(), v.to_string());
    }
    PipeMessage {
        source: PipeSource::Cli("test".into()),
        name: name.to_string(),
        payload: None,
        args: map,
        is_private: false,
    }
}

#[allow(dead_code)]
pub fn make_tab_info(name: &str, active: bool) -> TabInfo {
    TabInfo {
        position: 0,
        name: name.to_string(),
        active,
        panes_to_hide: 0,
        is_fullscreen_active: false,
        is_sync_panes_active: false,
        are_floating_panes_visible: false,
        other_focused_clients: vec![],
        active_swap_layout_name: None,
        is_swap_layout_dirty: false,
        viewport_rows: 0,
        viewport_columns: 0,
        display_area_rows: 0,
        display_area_columns: 0,
        selectable_tiled_panes_count: 0,
        selectable_floating_panes_count: 0,
    }
}

pub fn key(bare: BareKey) -> KeyWithModifier {
    KeyWithModifier { bare_key: bare, key_modifiers: BTreeSet::new() }
}

pub fn render_to_string(state: &State, rows: usize, cols: usize) -> String {
    let mut buf = Vec::new();
    state.render_to(&mut buf, rows, cols);
    let output = String::from_utf8(buf).unwrap();
    // Replace the build-specific version string so snapshots don't change on every commit.
    output.replace(zelligent_plugin::VERSION, "VERSION")
}

/// Strip ANSI CSI escape sequences (`ESC '[' <params> <final-byte>`, the
/// only kind this codebase's `ui` module emits) so a rendered line's
/// on-screen width can be measured.
#[allow(dead_code)]
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
            }
            for next in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&next) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Count the physical terminal rows a rendered frame occupies, the way a
/// real terminal grid would: each newline-delimited line contributes
/// `ceil(visible_width / cols).max(1)` rows, since a too-wide line
/// soft-wraps instead of scrolling. `render_to`'s very last write is
/// deliberately `write!`, not `writeln!` (see `render_footer`'s doc
/// comment), so a frame that exactly fills `rows` has no trailing newline —
/// drop the trailing empty split that a terminating `\n` would otherwise
/// produce so it isn't miscounted as an extra blank row.
///
/// This is the regression check for #135/#136: every render arm must
/// account for a wrapped status message (via `ui::status_height`) when
/// budgeting its padding, or the frame emits more physical rows than
/// `rows`, forcing a scroll that discards row 0 (the header).
#[allow(dead_code)]
pub fn physical_rows(output: &str, cols: usize) -> usize {
    let mut lines: Vec<&str> = output.split('\n').collect();
    if output.ends_with('\n') {
        lines.pop();
    }
    lines
        .iter()
        .map(|line| {
            let width = zelligent_plugin::ui::visible_width(&strip_ansi(line));
            width.div_ceil(cols.max(1)).max(1)
        })
        .sum()
}

pub fn state_with_worktrees() -> State {
    let mut s = State::default();
    s.mode = Mode::BrowseWorktrees;
    s.worktrees = vec![
        Worktree { dir: "feat-a".into(), branch: "feat-a".into() },
        Worktree { dir: "feat-b".into(), branch: "feat-b".into() },
        Worktree { dir: "feat-c".into(), branch: "feat-c".into() },
    ];
    s.tabs = vec![
        make_tab_info("feat-a", true),
        make_tab_info("feat-b", false),
        make_tab_info("feat-c", false),
    ];
    s.branches = vec!["main".into(), "feat-a".into(), "feat-b".into(), "dev".into()];
    s.recompute_sidebar_items();
    s
}
