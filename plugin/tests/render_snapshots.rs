mod common;

use common::{key, make_tab_info, physical_rows, render_to_string, state_with_worktrees};
use zelligent_plugin::{AgentStatus, Mode, SidebarItem, State, Worktree};
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

/// #216: a failed refresh keeps the last known worktree list on screen and
/// adds a persistent `stale · retrying` marker with a short reason plus the
/// `e: details` on-demand hint. The marker occupies exactly one budgeted
/// row, so the frame still fits `rows` (no #135/#136 scroll).
#[test]
fn render_browse_stale_marker() {
    let mut s = state_with_worktrees();
    s.refresh_error =
        Some("Failed to list worktrees: git: Too many open files (os error 24)".into());
    let output = render_to_string(&s, 20, 80);
    assert_eq!(
        physical_rows(&output, 80),
        20,
        "the stale marker must be budgeted, not overflow the frame"
    );
    assert!(output.contains("stale"), "persistent staleness marker is shown");
    assert!(output.contains("too many open files"), "short reason is inline");
    assert!(output.contains("e: details"), "wide pane advertises the on-demand full error");
    assert!(output.contains("feat-a"), "the last known list stays visible");
    insta::assert_snapshot!(output);
}

/// #216: on a narrow pane the marker drops the `e: details` hint first so the
/// reason survives, and still fits `rows` exactly.
#[test]
fn render_browse_stale_marker_narrow_drops_hint() {
    let mut s = state_with_worktrees();
    s.refresh_error =
        Some("Failed to list worktrees: git: Too many open files (os error 24)".into());
    let output = render_to_string(&s, 20, 30);
    assert_eq!(physical_rows(&output, 30), 20, "narrow marker must not wrap past `rows`");
    assert!(output.contains("stale"));
    insta::assert_snapshot!(output);
}

/// #216: even with no worktrees to list (the first refresh itself failed),
/// the empty state still surfaces the staleness marker.
#[test]
fn render_browse_empty_with_stale_marker() {
    let mut s = State { mode: Mode::BrowseWorktrees, ..Default::default() };
    s.refresh_error = Some("Failed to list worktrees: boom".into());
    let output = render_to_string(&s, 20, 80);
    assert_eq!(physical_rows(&output, 80), 20, "empty-state stale marker must be budgeted");
    assert!(output.contains("stale"));
    insta::assert_snapshot!(output);
}

/// #216 finding 4: an unsatisfied invalidation with no refresh in flight
/// (`cache_dirty`, no `refresh_error`) shows the generic `changes pending`
/// marker — no `e: details` hint, since there's no error text to recover.
#[test]
fn render_browse_stale_marker_cache_dirty_generic() {
    let mut s = state_with_worktrees();
    s.cache_dirty = true; // known-but-unsatisfied invalidation, not in flight
    let output = render_to_string(&s, 20, 80);
    assert_eq!(physical_rows(&output, 80), 20);
    assert!(output.contains("stale"), "cache_dirty alone flags staleness");
    assert!(output.contains("changes pending"), "generic reason for a bare invalidation");
    assert!(!output.contains("e: details"), "no on-demand error when there's no error text");
    insta::assert_snapshot!(output);
}

/// #216 finding 5: an undersized pane (rows=5) carrying BOTH a persistent
/// stale marker AND an active wrapped error status must degrade (footer
/// collapses to the version line, item viewport may shrink to zero) rather
/// than overflow `rows` and scroll the frame (#135/#136).
#[test]
fn render_browse_stale_marker_tiny_pane_5_rows_no_overflow() {
    let mut s = state_with_worktrees();
    s.refresh_error =
        Some("Failed to list worktrees: git: Too many open files (os error 24)".into());
    s.status_message = "Failed to list worktrees: too many open files".into();
    s.status_is_error = true;
    let output = render_to_string(&s, 5, 40);
    assert_eq!(physical_rows(&output, 40), 5, "rows=5 with wrapped error must not overflow");
    assert!(output.contains("stale"), "the marker survives even at rows=5");
    insta::assert_snapshot!(output);
}

/// #216 finding 5: rows=6 boundary — one more row than the tightest case,
/// so a single item can reappear while still not overflowing.
#[test]
fn render_browse_stale_marker_tiny_pane_6_rows_no_overflow() {
    let mut s = state_with_worktrees();
    s.refresh_error =
        Some("Failed to list worktrees: git: Too many open files (os error 24)".into());
    s.status_message = "Failed to list worktrees: too many open files".into();
    s.status_is_error = true;
    let output = render_to_string(&s, 6, 40);
    assert_eq!(physical_rows(&output, 40), 6, "rows=6 with wrapped error must not overflow");
    assert!(output.contains("stale"));
    insta::assert_snapshot!(output);
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
fn render_sidebar_list_scrolling() {
    let mut s = State {
        mode: Mode::BrowseWorktrees,
        worktrees: (0..20)
            .map(|i| Worktree { dir: format!("branch-{i}"), branch: format!("branch-{i}") })
            .collect(),
        tabs: (0..20)
            .map(|i| make_tab_info(&format!("branch-{i}"), i == 15))
            .collect(),
        selected_index: 15,
        ..Default::default()
    };
    s.recompute_sidebar_items();
    insta::assert_snapshot!(render_to_string(&s, 10, 80));
}

#[test]
fn render_sidebar_list_short_pane_still_shows_one_item() {
    let mut s = State {
        mode: Mode::BrowseWorktrees,
        worktrees: (0..3)
            .map(|i| Worktree { dir: format!("branch-{i}"), branch: format!("branch-{i}") })
            .collect(),
        tabs: (0..3)
            .map(|i| make_tab_info(&format!("branch-{i}"), i == 2))
            .collect(),
        selected_index: 2,
        ..Default::default()
    };
    s.recompute_sidebar_items();
    // rows=5, cols=80 -> footer_lines=3, content_budget=2 -> below
    // MIN_ROWS_HEADER_ONLY (3), so BOTH header and separator degrade away.
    // The item row is never sacrificed: it's still the very first content
    // line. See #135/#136.
    insta::assert_snapshot!(render_to_string(&s, 5, 80));
}

/// Middle degradation tier: enough room for the header but not the blank
/// separator. rows=6, cols=80 -> footer_lines=3, content_budget=3 ==
/// MIN_ROWS_HEADER_ONLY -> header shows, separator does not.
#[test]
fn render_sidebar_list_medium_pane_drops_separator_keeps_header() {
    let mut s = State {
        mode: Mode::BrowseWorktrees,
        worktrees: (0..3)
            .map(|i| Worktree { dir: format!("branch-{i}"), branch: format!("branch-{i}") })
            .collect(),
        tabs: (0..3)
            .map(|i| make_tab_info(&format!("branch-{i}"), i == 0))
            .collect(),
        selected_index: 0,
        ..Default::default()
    };
    s.recompute_sidebar_items();
    insta::assert_snapshot!(render_to_string(&s, 6, 80));
}

/// A status message that's wider than the pane wraps to multiple physical
/// terminal rows. `sidebar_layout` accounts for the wrap when carving the
/// status budget out of `content_budget`, so the header/separator/item
/// rows above it stay in their normal positions and the frame still fits
/// `rows` exactly (no scroll, no swallowed header) — this is the #136
/// "dynamic offset" bug, fixed. cols=30: the ~34-char message (36 chars
/// with its 2-space prefix) wraps to 2 physical rows.
#[test]
fn render_browse_with_wrapped_status_message() {
    let mut s = state_with_worktrees();
    s.status_message = "Only worktree tabs can be removed".into();
    s.status_is_error = true;
    let output = render_to_string(&s, 20, 30);
    assert_eq!(
        physical_rows(&output, 30),
        20,
        "wrapped status must not push the frame past `rows` (that's the #136 scroll bug)"
    );
    assert!(output.contains("zelligent"), "header must survive a wrapped status message");
    insta::assert_snapshot!(output);
}

/// A status message long enough to wrap at cols=50 without also wrapping
/// any of the arm's own fixed content lines (this arm's longest fixed line,
/// the `x  nuke session & start fresh` hint, is 31 wide — comfortably under
/// 50 — which keeps this test isolated to the status-wrap bug rather than
/// tripping an unrelated one).
///
/// Companion to the populated-list case above: `Mode::NotGitRepo` computed
/// its status budget from a fixed `{ 0 } else { 2 }` guess (review of
/// #135/#136) instead of `ui::status_height`, so a wrapped message here
/// would silently overflow `rows` and scroll the header off just like the
/// bug this branch fixed for the sidebar list.
#[test]
fn render_not_git_repo_with_wrapped_status() {
    let s = State {
        mode: Mode::NotGitRepo,
        status_message: "Only worktree tabs can be removed from the sidebar".into(),
        status_is_error: true,
        initial_cwd: std::path::PathBuf::from("/tmp/foo"),
        ..Default::default()
    };
    let output = render_to_string(&s, 20, 50);
    assert_eq!(
        physical_rows(&output, 50),
        20,
        "wrapped status must not push the frame past `rows`"
    );
    assert!(output.contains(" zelligent "), "header must survive a wrapped status message");
    insta::assert_snapshot!(output);
}

/// Same status message and width as above; cols=50 also comfortably fits
/// this arm's longest fixed line (`Pick a branch or type a new one to get
/// started.`, 49 wide) so only the status wrap is under test.
///
/// Companion to the populated-list case above: the `BrowseWorktrees`
/// empty-state branch (`should_render_empty_state()`) had the same fixed
/// `{ 0 } else { 2 }` status guess as `NotGitRepo`/`InputBranch`.
#[test]
fn render_browse_empty_with_wrapped_status() {
    let s = State {
        mode: Mode::BrowseWorktrees,
        status_message: "Only worktree tabs can be removed from the sidebar".into(),
        status_is_error: true,
        ..Default::default()
    };
    let output = render_to_string(&s, 20, 50);
    assert_eq!(
        physical_rows(&output, 50),
        20,
        "wrapped status must not push the frame past `rows`"
    );
    assert!(output.contains("zelligent"), "header must survive a wrapped status message");
    insta::assert_snapshot!(output);
}

/// Companion to the populated-list case above: `Mode::InputBranch` had the
/// same fixed `{ 0 } else { 2 }` status guess.
#[test]
fn render_input_branch_with_wrapped_status() {
    let s = State {
        mode: Mode::InputBranch,
        status_message: "Only worktree tabs can be removed from the sidebar".into(),
        status_is_error: true,
        ..Default::default()
    };
    let output = render_to_string(&s, 20, 50);
    assert_eq!(
        physical_rows(&output, 50),
        20,
        "wrapped status must not push the frame past `rows`"
    );
    assert!(output.contains("New branch name"), "header/content must survive a wrapped status message");
    insta::assert_snapshot!(output);
}

#[test]
fn render_browse_mixed_dir_branch_names() {
    let mut s = State {
        mode: Mode::BrowseWorktrees,
        repo_name: "myrepo".into(),
        worktrees: vec![
            Worktree { dir: "autonomy".into(), branch: "plugin-snapshot-tests".into() },
            Worktree { dir: "competition".into(), branch: "competition".into() },
            Worktree { dir: "ding".into(), branch: "feat/ding-dong".into() },
        ],
        ..Default::default()
    };
    s.tabs = vec![
        make_tab_info("plugin-snapshot-tests", true),
        make_tab_info("competition", false),
        make_tab_info("feat-ding-dong", false),
    ];
    s.recompute_sidebar_items();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_sidebar_with_user_tab() {
    let mut s = state_with_worktrees();
    s.tabs.push(make_tab_info("notes", false));
    s.recompute_sidebar_items();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_sidebar_with_local_row() {
    let mut s = State {
        mode: Mode::BrowseWorktrees,
        repo_name: "zelligent".into(),
        worktrees: vec![
            Worktree { dir: "agent-test-mouse".into(), branch: "agent/test-mouse".into() },
        ],
        tabs: vec![
            make_tab_info("zelligent", false),
            make_tab_info("agent-test-mouse", true),
        ],
        ..Default::default()
    };
    s.recompute_sidebar_items();
    insta::assert_snapshot!(render_to_string(&s, 20, 44));
}

#[test]
fn render_sidebar_with_branch_subtitle() {
    let s = State {
        mode: Mode::BrowseWorktrees,
        repo_name: "myrepo".into(),
        sidebar_items: vec![
            SidebarItem {
                tab_name: "feat-cool".into(),
                display_name: "feat-cool".into(),
                matched_branch: Some("feat/cool".into()),
            },
            SidebarItem {
                tab_name: "notes".into(),
                display_name: "notes".into(),
                matched_branch: None,
            },
        ],
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_sidebar_with_redundant_branch_shows_subtitle_text() {
    let s = State {
        mode: Mode::BrowseWorktrees,
        repo_name: "myrepo".into(),
        sidebar_items: vec![
            SidebarItem {
                tab_name: "feature-a".into(),
                display_name: "feature-a".into(),
                matched_branch: Some("feature-a".into()),
            },
            SidebarItem {
                tab_name: "feature-cool".into(),
                display_name: "feature-cool".into(),
                matched_branch: Some("feature/cool".into()),
            },
        ],
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_not_git_repo() {
    let s = State {
        mode: Mode::NotGitRepo,
        status_message: "/tmp/foo is not a git repo: fatal: not a git repository".into(),
        status_is_error: true,
        initial_cwd: std::path::PathBuf::from("/tmp/foo"),
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_not_git_repo_without_status() {
    let s = State {
        mode: Mode::NotGitRepo,
        initial_cwd: std::path::PathBuf::from("/tmp/foo"),
        ..Default::default()
    };
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_with_agent_statuses() {
    let mut s = state_with_worktrees();
    s.agent_statuses.insert("feat-a".into(), AgentStatus::Working);
    s.agent_statuses.insert("feat-b".into(), AgentStatus::NeedsInput);
    // feat-c stays Idle (no entry)
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_with_done_status() {
    let mut s = state_with_worktrees();
    s.agent_statuses.insert("feat-b".into(), AgentStatus::Done);
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}

#[test]
fn render_browse_all_idle() {
    let s = state_with_worktrees();
    // No agent_statuses set — all should show as idle (2-space prefix)
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

// --- No-trailing-newline / exact-row-count invariant (#149 re-review) ---

/// Every `render_to` arm must write exactly `rows` physical lines with NO
/// trailing newline after the footer's last line — see the comment on
/// `render_footer`'s final `write!` (not `writeln!`) for why: a trailing
/// `\n` advances the cursor one row past the last valid row, forcing a
/// scroll that silently discards row 0 (the header), which is the verified
/// root cause of #136. This exercises `render_to` — the same entry point
/// production uses — across representative modes, through the same
/// `render_to_string`/`physical_rows` helpers the #135/#136 wrapped-status
/// regression tests above already rely on.
#[test]
fn render_never_ends_with_trailing_newline_and_fills_exactly_rows() {
    let cols = 80;
    let rows = 20;
    let cases: Vec<(&str, State)> = vec![
        ("BrowseWorktrees populated", state_with_worktrees()),
        (
            "BrowseWorktrees empty",
            State { mode: Mode::BrowseWorktrees, ..Default::default() },
        ),
        (
            "NotGitRepo",
            State {
                mode: Mode::NotGitRepo,
                initial_cwd: std::path::PathBuf::from("/tmp/foo"),
                ..Default::default()
            },
        ),
        ("InputBranch", State { mode: Mode::InputBranch, ..Default::default() }),
        ("SelectBranch", state_with_branches()),
        ("Loading", State::default()),
    ];
    for (label, state) in cases {
        let output = render_to_string(&state, rows, cols);
        assert!(
            !output.ends_with('\n'),
            "{label}: render must not end with a trailing newline (re-triggers #136)"
        );
        assert_eq!(
            physical_rows(&output, cols),
            rows,
            "{label}: render must fill exactly `rows` physical lines"
        );
    }
}
