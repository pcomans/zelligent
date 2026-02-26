mod common;

use common::{key, render_to_string, state_with_worktrees};
use zelligent_plugin::{Mode, State, Worktree};
use zellij_tile::prelude::*;

fn state_with_branches() -> State {
    let mut s = state_with_worktrees();
    s.mode = Mode::SelectBranch;
    s.filtered_branches = vec![
        "main".into(),
        "feat-a".into(),
        "feat-b".into(),
        "dev".into(),
    ];
    s.selected_index = 0;
    s
}

// --- Render snapshot tests ---

#[test]
fn render_loading_mode() {
    let s = State::default();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_loading_with_error() {
    let s = State {
        status_message: "Permission denied".into(),
        status_is_error: true,
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_with_worktrees() {
    let s = state_with_worktrees();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_empty() {
    let s = State { mode: Mode::BrowseWorktrees, ..Default::default() };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_with_status_message() {
    let mut s = state_with_worktrees();
    s.status_message = "Spawned worktree for feat-d".into();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_with_error_message() {
    let mut s = state_with_worktrees();
    s.status_message = "Failed to spawn worktree".into();
    s.status_is_error = true;
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_select_branch() {
    let s = state_with_branches();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_select_branch_empty() {
    let s = State {
        mode: Mode::SelectBranch,
        filtered_branches: vec![],
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_input_branch_empty() {
    let s = State { mode: Mode::InputBranch, ..Default::default() };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_input_branch_with_text() {
    let s = State {
        mode: Mode::InputBranch,
        input_buffer: "feat/my-feature".into(),
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_confirming() {
    let mut s = state_with_worktrees();
    s.mode = Mode::Confirming;
    s.selected_index = 1;
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_worktree_list_scrolling() {
    let s = State {
        mode: Mode::BrowseWorktrees,
        worktrees: (0..20)
            .map(|i| Worktree { dir: format!("branch-{i}"), branch: format!("branch-{i}") })
            .collect(),
        selected_index: 15,
        ..Default::default()
    };
    // Small viewport forces scrolling
    insta::assert_snapshot!(render_to_string(&s, 10, 80));
}

#[test]
fn render_browse_mixed_dir_branch_names() {
    let s = State {
        mode: Mode::BrowseWorktrees,
        repo_name: "myrepo".into(),
        worktrees: vec![
            Worktree { dir: "autonomy".into(), branch: "plugin-snapshot-tests".into() },
            Worktree { dir: "competition".into(), branch: "competition".into() },
            Worktree { dir: "ding".into(), branch: "feat/ding-dong".into() },
        ],
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

// --- Interaction flow tests ---

#[test]
fn flow_browse_navigate_and_render() {
    let mut s = state_with_worktrees();
    let initial = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_browse_initial", initial);

    // Navigate down
    s.handle_key_browse(&key(BareKey::Char('j')));
    let after_j = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_browse_after_j", after_j);

    // Navigate down again
    s.handle_key_browse(&key(BareKey::Char('j')));
    let after_jj = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_browse_after_jj", after_jj);
}

#[test]
fn flow_browse_to_branch_picker() {
    let mut s = state_with_worktrees();
    s.filtered_branches = s.branches.clone();

    // Press 'n' to enter branch picker
    s.handle_key_browse(&key(BareKey::Char('n')));
    assert_eq!(s.mode, Mode::SelectBranch);
    let picker = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_branch_picker", picker);

    // Navigate down in picker
    s.handle_key_select_branch(&key(BareKey::Char('j')));
    let picker_moved = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_branch_picker_after_j", picker_moved);

    // Escape back
    s.handle_key_select_branch(&key(BareKey::Esc));
    assert_eq!(s.mode, Mode::BrowseWorktrees);
}

#[test]
fn flow_browse_to_input_branch() {
    let mut s = state_with_worktrees();

    // Press 'i' to enter input mode
    s.handle_key_browse(&key(BareKey::Char('i')));
    assert_eq!(s.mode, Mode::InputBranch);
    let empty_input = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_input_empty", empty_input);

    // Type a branch name
    s.handle_key_input_branch(&key(BareKey::Char('f')));
    s.handle_key_input_branch(&key(BareKey::Char('i')));
    s.handle_key_input_branch(&key(BareKey::Char('x')));
    let typed = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_input_typed", typed);

    // Backspace
    s.handle_key_input_branch(&key(BareKey::Backspace));
    let after_bs = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_input_after_backspace", after_bs);
}

#[test]
fn flow_browse_to_confirm_delete() {
    let mut s = state_with_worktrees();
    s.selected_index = 1; // feat-b

    // Press 'd' to confirm delete
    s.handle_key_browse(&key(BareKey::Char('d')));
    assert_eq!(s.mode, Mode::Confirming);
    let confirm = render_to_string(&s, 20, 80);
    insta::assert_snapshot!("flow_confirm_delete", confirm);

    // Press Esc to cancel
    s.handle_key_confirming(&key(BareKey::Esc));
    assert_eq!(s.mode, Mode::BrowseWorktrees);
}
