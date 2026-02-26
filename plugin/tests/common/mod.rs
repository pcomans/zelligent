use std::collections::BTreeSet;
use zelligent_plugin::{Mode, State, Worktree};
use zellij_tile::prelude::*;

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
        Worktree { branch: "feat-a".into() },
        Worktree { branch: "feat-b".into() },
        Worktree { branch: "feat-c".into() },
    ];
    s.branches = vec!["main".into(), "feat-a".into(), "feat-b".into(), "dev".into()];
    s
}
