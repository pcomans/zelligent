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

pub fn state_with_worktrees() -> State {
    let mut s = State::default();
    s.mode = Mode::BrowseWorktrees;
    s.worktrees = vec![
        Worktree { dir: "feat-a".into(), branch: "feat-a".into() },
        Worktree { dir: "feat-b".into(), branch: "feat-b".into() },
        Worktree { dir: "feat-c".into(), branch: "feat-c".into() },
    ];
    s.branches = vec!["main".into(), "feat-a".into(), "feat-b".into(), "dev".into()];
    // Populate tabs so sidebar_items are computed (matches real runtime behavior)
    s.tabs = vec![
        make_tab_info("feat-a", true),
        make_tab_info("feat-b", false),
        make_tab_info("feat-c", false),
    ];
    s.recompute_sidebar_items();
    s
}
