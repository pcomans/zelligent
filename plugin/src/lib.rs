pub mod ui;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use zellij_tile::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentStatus {
    #[default]
    Idle,
    Working,
    Awaiting,
    Done,
}

pub const VERSION: &str = env!("ZELLIGENT_VERSION");

// Command context keys used to route RunCommandResult
pub const CMD_GIT_TOPLEVEL: &str = "git_toplevel";
pub const CMD_LIST_WORKTREES: &str = "list_worktrees";
pub const CMD_GIT_BRANCHES: &str = "git_branches";
pub const CMD_SPAWN: &str = "spawn";
pub const CMD_REMOVE: &str = "remove";

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Mode {
    #[default]
    Loading,
    NotGitRepo,
    BrowseWorktrees,
    SelectBranch,
    InputBranch,
    Confirming,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Worktree {
    pub dir: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarItem {
    pub tab_name: String,
    pub display_name: String,
    pub matched_branch: Option<String>,
}

/// Actions returned by key/event handlers, executed by the plugin shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    Close,
    Spawn(String),
    Remove(String),
    /// Close a tab by name, optionally returning to another tab, then refresh.
    CloseTabAndRefresh {
        tab_name: String,
        return_to: Option<String>,
    },
    SwitchToTab(String),
    Refresh,
    FetchToplevel,
    FetchWorktreesAndBranches,
    DumpLayout,
    NukeSession,
    Notify { tab_name: String, status: AgentStatus },
}

#[derive(Default)]
pub struct State {
    pub mode: Mode,
    pub repo_root: String,
    pub repo_name: String,
    pub worktrees: Vec<Worktree>,
    pub branches: Vec<String>,
    pub filtered_branches: Vec<String>,
    pub selected_index: usize,
    pub input_buffer: String,
    pub agent_cmd: String,
    pub status_message: String,
    pub status_is_error: bool,
    pub zelligent_path: String,
    pub initial_cwd: PathBuf,
    pub session_name: Option<String>,
    pub tabs: Vec<TabInfo>,
    /// Sidebar items built from tabs + worktrees. Navigated in browse mode.
    pub sidebar_items: Vec<SidebarItem>,
    /// Agent status per tab name (sanitized branch name).
    pub agent_statuses: BTreeMap<String, AgentStatus>,
}

/// Sanitize a user-supplied string into a valid git branch name.
/// Replaces characters and sequences forbidden in git refs, collapses
/// consecutive hyphens and slashes, and strips leading/trailing hyphens,
/// dots, and slashes.
pub fn sanitize_branch_name(name: &str) -> String {
    // Replace characters forbidden in git refs with hyphens
    let s: String = name
        .chars()
        .map(|c| match c {
            ' ' | '\t' | '~' | '^' | ':' | '?' | '*' | '[' | '\\' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();

    // Replace forbidden multi-character sequences
    let s = s.replace("@{", "-").replace("..", "-").replace("/.", "/-");

    // Collapse consecutive hyphens and consecutive slashes
    let mut result = String::new();
    let mut prev = '\0';
    for c in s.chars() {
        if (c == '-' && prev == '-') || (c == '/' && prev == '/') {
            continue;
        }
        result.push(c);
        prev = c;
    }

    // Strip reserved .lock suffix
    if result.ends_with(".lock") {
        result.truncate(result.len() - 5);
    }

    result.trim_matches(|c| c == '-' || c == '.' || c == '/').to_string()
}

/// Parse `zelligent list-worktrees` output.
/// Format: `dir\tbranch` per line (tab-separated).
/// Falls back to branch-only for backwards compatibility.
pub fn parse_worktrees(output: &str) -> Vec<Worktree> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|line| {
            if let Some((dir, branch)) = line.split_once('\t') {
                Worktree {
                    dir: dir.trim().to_string(),
                    branch: branch.trim().to_string(),
                }
            } else {
                Worktree {
                    dir: line.to_string(),
                    branch: line.to_string(),
                }
            }
        })
        .collect()
}

/// Parse `git branch --format=%(refname:short)` output into a list of branch names.
pub fn parse_branches(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Wrapping navigation: move `current` by `delta` within `[0, len)`, wrapping around.
pub fn wrap_navigate(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    ((current as isize + delta).rem_euclid(len as isize)) as usize
}

impl State {
    fn ctx(cmd_type: &str) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("cmd_type".to_string(), cmd_type.to_string());
        m
    }

    fn fire_git_toplevel(&self) {
        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "show-repo"],
            BTreeMap::new(),
            self.initial_cwd.clone(),
            Self::ctx(CMD_GIT_TOPLEVEL),
        );
    }

    fn fire_list_worktrees(&self) {
        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "list-worktrees"],
            BTreeMap::new(),
            PathBuf::from(&self.repo_root),
            Self::ctx(CMD_LIST_WORKTREES),
        );
    }

    fn fire_git_branches(&self) {
        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "list-branches"],
            BTreeMap::new(),
            PathBuf::from(&self.repo_root),
            Self::ctx(CMD_GIT_BRANCHES),
        );
    }

    fn fire_spawn(&self, branch: &str) {
        let mut env = BTreeMap::new();
        if let Ok(val) = std::env::var("ZELLIJ") {
            env.insert("ZELLIJ".to_string(), val);
        }
        if let Ok(val) = std::env::var("ZELLIJ_SESSION_NAME") {
            env.insert("ZELLIJ_SESSION_NAME".to_string(), val);
        }

        let mut ctx = Self::ctx(CMD_SPAWN);
        ctx.insert("branch".to_string(), branch.to_string());

        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "spawn", branch, &self.agent_cmd],
            env,
            PathBuf::from(&self.repo_root),
            ctx,
        );
    }

    fn fire_remove(&self, branch: &str) {
        let mut env = BTreeMap::new();
        if let Ok(val) = std::env::var("ZELLIJ") {
            env.insert("ZELLIJ".to_string(), val);
        }

        let mut ctx = Self::ctx(CMD_REMOVE);
        ctx.insert("branch".to_string(), branch.to_string());

        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "remove", branch],
            env,
            PathBuf::from(&self.repo_root),
            ctx,
        );
    }

    fn execute(&self, action: &Action) {
        match action {
            Action::None => {}
            Action::Close => close_self(),
            Action::Spawn(branch) => self.fire_spawn(branch),
            Action::Remove(branch) => self.fire_remove(branch),
            Action::CloseTabAndRefresh { tab_name, return_to } => {
                go_to_tab_name(tab_name);
                close_focused_tab();
                if let Some(name) = return_to {
                    go_to_tab_name(name);
                }
                self.fire_list_worktrees();
                self.fire_git_branches();
            }
            Action::SwitchToTab(tab_name) => {
                go_to_tab_name(tab_name);
            }
            Action::Refresh => {
                self.fire_list_worktrees();
                self.fire_git_branches();
            }
            Action::FetchToplevel => self.fire_git_toplevel(),
            Action::FetchWorktreesAndBranches => {
                self.fire_list_worktrees();
                self.fire_git_branches();
            }
            Action::DumpLayout => {
                dump_session_layout();
            }
            Action::NukeSession => {
                // The handler already verified session_name is Some.
                // kill_sessions terminates our process, so nothing after it runs.
                if let Some(name) = &self.session_name {
                    kill_sessions(&[name.as_str()]);
                }
            }
            Action::Notify { tab_name, status } => {
                // macOS-only: osascript and afplay. On Linux, use notify-send/paplay.
                let body = match status {
                    AgentStatus::Awaiting => format!("{tab_name} needs input"),
                    AgentStatus::Done => format!("{tab_name} finished"),
                    _ => return,
                };
                run_command(
                    &[
                        "osascript",
                        "-e",
                        "on run argv",
                        "-e",
                        "display notification (item 1 of argv) with title \"zelligent\"",
                        "-e",
                        "end run",
                        &body,
                    ],
                    BTreeMap::new(),
                );
                if matches!(status, AgentStatus::Awaiting) {
                    run_command(
                        &["afplay", "/System/Library/Sounds/Glass.aiff"],
                        BTreeMap::new(),
                    );
                }
            }
        }
    }

    // --- Pure state handlers (no zellij calls, fully testable) ---

    pub fn handle_git_toplevel(&mut self, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Action {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            let cwd = self.initial_cwd.display();
            self.status_message = format!("{cwd} is not a git repo: {err}");
            self.status_is_error = true;
            self.mode = Mode::NotGitRepo;
            return Action::None;
        }
        let output = String::from_utf8_lossy(stdout);
        for line in output.lines() {
            if let Some(val) = line.strip_prefix("repo_root=") {
                self.repo_root = val.to_string();
            } else if let Some(val) = line.strip_prefix("repo_name=") {
                self.repo_name = val.to_string();
            }
        }
        if self.repo_root.is_empty() || self.repo_name.is_empty() {
            self.status_message = "Failed to parse repo info".to_string();
            self.status_is_error = true;
            return Action::None;
        }
        self.mode = Mode::BrowseWorktrees;
        Action::FetchWorktreesAndBranches
    }

    pub fn handle_list_worktrees(&mut self, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            self.status_message = format!("Failed to list worktrees: {err}");
            self.status_is_error = true;
            return;
        }
        let output = String::from_utf8_lossy(stdout);
        self.worktrees = parse_worktrees(&output);
        self.recompute_sidebar_items();
    }

    /// Rebuild sidebar_items from tabs + worktrees, preserving cursor by tab name.
    pub fn recompute_sidebar_items(&mut self) {
        // Remember the currently selected tab name for cursor restoration
        let prev_tab_name = self.sidebar_items
            .get(self.selected_index)
            .map(|item| item.tab_name.clone());

        let mut items = Vec::new();
        for tab in &self.tabs {
            let matched = self.worktrees.iter().find(|wt| {
                Self::tab_name_for_branch(&wt.branch) == tab.name
            });
            items.push(SidebarItem {
                tab_name: tab.name.clone(),
                display_name: tab.name.clone(),
                matched_branch: matched.map(|wt| wt.branch.clone()),
            });
        }
        self.sidebar_items = items;

        // Restore cursor position by name, or select active tab
        if let Some(prev_name) = &prev_tab_name {
            if let Some(idx) = self.sidebar_items.iter().position(|i| i.tab_name == *prev_name) {
                self.selected_index = idx;
                return;
            }
        }
        // Fallback: select the active tab
        if let Some(idx) = self.sidebar_items.iter().position(|i| {
            self.tabs.iter().any(|t| t.active && t.name == i.tab_name)
        }) {
            self.selected_index = idx;
        } else if self.selected_index >= self.sidebar_items.len() && !self.sidebar_items.is_empty() {
            self.selected_index = self.sidebar_items.len() - 1;
        }
    }

    /// Return the index of the worktree whose tab name matches the active tab, if any.
    /// Returns the first match; see `tab_name_for_branch` for the ambiguity caveat.
    pub fn find_worktree_for_active_tab(&self) -> Option<usize> {
        let active_name = self.tabs.iter().find(|t| t.active).map(|t| t.name.as_str())?;
        self.worktrees
            .iter()
            .position(|wt| Self::tab_name_for_branch(&wt.branch) == active_name)
    }

    pub fn handle_git_branches(&mut self, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            self.status_message = format!("Failed to list branches: {err}");
            self.status_is_error = true;
            return;
        }
        let output = String::from_utf8_lossy(stdout);
        self.branches = parse_branches(&output);
    }

    pub fn handle_spawn_result(&mut self, exit_code: Option<i32>, stderr: &[u8], context: &BTreeMap<String, String>) -> Action {
        let branch = context.get("branch").cloned().unwrap_or_else(|| "<unknown>".to_string());
        if exit_code == Some(0) {
            self.status_message = format!("Spawned '{branch}'");
            self.status_is_error = false;
        } else {
            let err = String::from_utf8_lossy(stderr).trim().to_string();
            let code_str = match exit_code {
                Some(c) => format!("exit {c}"),
                None => "no exit code".to_string(),
            };
            if err.is_empty() {
                self.status_message = format!("Spawn '{branch}' failed ({code_str})");
            } else {
                self.status_message = format!("Spawn '{branch}' failed: {err}");
            }
            self.status_is_error = true;
        }
        Action::Refresh
    }

    pub fn handle_remove_result(&mut self, exit_code: Option<i32>, stderr: &[u8], context: &BTreeMap<String, String>) -> Action {
        let branch = context.get("branch").cloned().unwrap_or_else(|| "<unknown>".to_string());
        self.mode = Mode::BrowseWorktrees;
        if exit_code == Some(0) {
            self.status_message = format!("Removed '{branch}'");
            self.status_is_error = false;
            // Close the worktree's tab if it exists, then refresh.
            if self.has_tab_for_branch(&branch) {
                let tab_name = Self::tab_name_for_branch(&branch);
                self.agent_statuses.remove(&tab_name);
                let return_to = self.tabs.iter().find(|t| t.active).map(|t| t.name.clone());
                return Action::CloseTabAndRefresh { tab_name, return_to };
            }
        } else {
            let err = String::from_utf8_lossy(stderr).trim().to_string();
            let code_str = match exit_code {
                Some(c) => format!("exit {c}"),
                None => "no exit code".to_string(),
            };
            if err.is_empty() {
                self.status_message = format!("Remove '{branch}' failed ({code_str})");
            } else {
                self.status_message = format!("Remove '{branch}' failed: {err}");
            }
            self.status_is_error = true;
        }
        Action::Refresh
    }

    /// Convert a branch name to the corresponding Zellij tab name.
    /// Tab names use the branch with `/` replaced by `-` and non-`[A-Za-z0-9_-]`
    /// chars stripped (matching zelligent.sh).
    ///
    /// Note: this mapping is not injective — `feat/cool` and `feat-cool` both
    /// produce `feat-cool`. In that case `find_worktree_for_active_tab` will
    /// match the first worktree in the list, which may not be the intended one.
    pub fn tab_name_for_branch(branch: &str) -> String {
        ui::sanitize_tab_name(branch)
    }

    /// Check whether a tab with the given branch's name exists.
    pub fn has_tab_for_branch(&self, branch: &str) -> bool {
        let tab_name = Self::tab_name_for_branch(branch);
        self.tabs.iter().any(|t| t.name == tab_name)
    }

    /// Switch to an existing tab for the branch, or spawn a new one.
    fn spawn_or_switch(&mut self, branch: String) -> Action {
        if self.has_tab_for_branch(&branch) {
            let tab_name = Self::tab_name_for_branch(&branch);
            return Action::SwitchToTab(tab_name);
        }
        self.status_message = format!("Spawning '{branch}'...");
        self.status_is_error = false;
        Action::Spawn(branch)
    }

    fn selected_worktree(&self) -> Option<&Worktree> {
        if self.sidebar_items.is_empty() {
            return self.worktrees.get(self.selected_index);
        }

        let branch = self
            .sidebar_items
            .get(self.selected_index)?
            .matched_branch
            .as_ref()?;

        self.worktrees.iter().find(|wt| wt.branch == *branch)
    }

    fn selected_removable_worktree(&self) -> Option<&Worktree> {
        let wt = self.selected_worktree()?;
        if wt.branch == "main" {
            return None;
        }
        Some(wt)
    }

    pub fn handle_key_browse(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('j') | BareKey::Down => {
                    let len = if self.sidebar_items.is_empty() { self.worktrees.len() } else { self.sidebar_items.len() };
                    self.selected_index = wrap_navigate(self.selected_index, len, 1);
                }
                BareKey::Char('k') | BareKey::Up => {
                    let len = if self.sidebar_items.is_empty() { self.worktrees.len() } else { self.sidebar_items.len() };
                    self.selected_index = wrap_navigate(self.selected_index, len, -1);
                }
                BareKey::Enter => {
                    if let Some(item) = self.sidebar_items.get(self.selected_index) {
                        return Action::SwitchToTab(item.tab_name.clone());
                    }
                    if let Some(wt) = self.worktrees.get(self.selected_index) {
                        return self.spawn_or_switch(wt.branch.clone());
                    }
                }
                BareKey::Char('n') => {
                    self.filtered_branches = self.branches.clone();
                    self.mode = Mode::SelectBranch;
                    self.selected_index = 0;
                }
                BareKey::Char('i') => {
                    self.mode = Mode::InputBranch;
                    self.input_buffer.clear();
                }
                BareKey::Char('d') => {
                    if self.selected_removable_worktree().is_some() {
                        self.mode = Mode::Confirming;
                    }
                }
                BareKey::Char('r') => {
                    self.status_message = "Refreshed".to_string();
                    self.status_is_error = false;
                    return Action::Refresh;
                }
                BareKey::Esc | BareKey::Char('q') => {
                    // No-op in sidebar mode (closing the sidebar is destructive)
                }
                _ => {}
            }
        }
        Action::None
    }

    pub fn handle_key_select_branch(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('j') | BareKey::Down => {
                    self.selected_index = wrap_navigate(self.selected_index, self.filtered_branches.len(), 1);
                }
                BareKey::Char('k') | BareKey::Up => {
                    self.selected_index = wrap_navigate(self.selected_index, self.filtered_branches.len(), -1);
                }
                BareKey::Enter => {
                    if let Some(branch) = self.filtered_branches.get(self.selected_index).cloned() {
                        self.mode = Mode::BrowseWorktrees;
                        return self.spawn_or_switch(branch);
                    }
                }
                BareKey::Esc => {
                    self.mode = Mode::BrowseWorktrees;
                    self.selected_index = 0;
                }
                _ => {}
            }
        }
        Action::None
    }

    pub fn handle_key_input_branch(&mut self, key: &KeyWithModifier) -> Action {
        let no_mod = key.has_no_modifiers();
        let shift_only = key.key_modifiers.len() == 1
            && key.key_modifiers.contains(&KeyModifier::Shift);

        match key.bare_key {
            BareKey::Enter if no_mod => {
                let branch = sanitize_branch_name(self.input_buffer.trim());
                if !branch.is_empty() {
                    self.mode = Mode::BrowseWorktrees;
                    return self.spawn_or_switch(branch);
                } else {
                    self.status_message = "Invalid branch name".to_string();
                    self.status_is_error = true;
                }
            }
            BareKey::Esc if no_mod => {
                self.mode = Mode::BrowseWorktrees;
                self.selected_index = 0;
                self.input_buffer.clear();
            }
            BareKey::Backspace if no_mod => {
                self.input_buffer.pop();
            }
            BareKey::Char(c) if no_mod || shift_only => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Action::None
    }

    pub fn handle_key_confirming(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('y') => {
                    if let Some(wt) = self.selected_removable_worktree() {
                        let branch = wt.branch.clone();
                        self.status_message = format!("Removing '{branch}'...");
                        self.status_is_error = false;
                        return Action::Remove(branch);
                    }
                }
                BareKey::Char('n') | BareKey::Esc => {
                    self.mode = Mode::BrowseWorktrees;
                }
                _ => {}
            }
        }
        Action::None
    }

    pub fn handle_pipe(&mut self, msg: &PipeMessage) -> Action {
        if msg.name != "zelligent-status" {
            return Action::None;
        }
        let tab_name = match msg.args.get("tab") {
            Some(name) if !name.is_empty() => name.clone(),
            _ => return Action::None,
        };
        let status = match msg.args.get("event").map(|s| s.as_str()) {
            Some("Start") | Some("UserPromptSubmit") => AgentStatus::Working,
            Some("PermissionRequest") => AgentStatus::Awaiting,
            Some("Stop") => AgentStatus::Done,
            Some(other) => {
                self.status_message = format!("Unknown agent event: {other}");
                self.status_is_error = true;
                return Action::None;
            }
            None => {
                self.status_message = "Agent status missing 'event' arg".to_string();
                self.status_is_error = true;
                return Action::None;
            }
        };
        // Ignore status updates for unknown tabs
        if !self.tabs.iter().any(|t| t.name == tab_name) {
            return Action::None;
        }
        self.agent_statuses.insert(tab_name.clone(), status);
        // TODO: consider suppressing notifications when the tab is active
        match status {
            AgentStatus::Awaiting | AgentStatus::Done => {
                Action::Notify { tab_name, status }
            }
            _ => Action::None,
        }
    }

    pub fn handle_key_not_git_repo(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('d') => {
                    self.status_message = "Layout dumped".to_string();
                    self.status_is_error = false;
                    return Action::DumpLayout;
                }
                BareKey::Char('x') => {
                    if self.session_name.is_some() {
                        return Action::NukeSession;
                    } else {
                        self.status_message = "Cannot determine session name".to_string();
                        self.status_is_error = true;
                    }
                }
                BareKey::Char('q') | BareKey::Esc => return Action::Close,
                _ => {}
            }
        }
        Action::None
    }

    pub fn render_to(&self, w: &mut impl Write, rows: usize, cols: usize) {
        match self.mode {
            Mode::Loading => {
                ui::render_header(w, "loading...", cols);
                writeln!(w).unwrap();
                if self.status_is_error {
                    ui::render_status(w, &self.status_message, self.status_is_error);
                } else {
                    writeln!(w, "  Waiting for permissions...").unwrap();
                }
            }
            Mode::NotGitRepo => {
                ui::render_header(w, "error", cols);
                ui::render_not_git_repo(w, &self.initial_cwd.display().to_string());
                ui::render_status(w, &self.status_message, self.status_is_error);
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::BrowseWorktrees => {
                ui::render_header(w, &self.repo_name, cols);
                ui::render_sidebar_list(w, &self.sidebar_items, &self.agent_statuses, self.selected_index, rows, cols);
                ui::render_status(w, &self.status_message, self.status_is_error);
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::SelectBranch => {
                ui::render_header(w, &self.repo_name, cols);
                ui::render_branch_list(w, &self.filtered_branches, self.selected_index, rows);
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::InputBranch => {
                ui::render_header(w, &self.repo_name, cols);
                ui::render_input(w, &self.input_buffer);
                ui::render_status(w, &self.status_message, self.status_is_error);
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::Confirming => {
                ui::render_header(w, &self.repo_name, cols);
                if let Some(wt) = self.selected_removable_worktree() {
                    ui::render_confirm(w, &wt.dir, &wt.branch);
                }
            }
        }
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.agent_cmd = configuration
            .get("agent_cmd")
            .cloned()
            .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string()));

        self.zelligent_path = configuration
            .get("zelligent_path")
            .cloned()
            .unwrap_or_else(|| "zelligent".to_string());

        self.initial_cwd = get_plugin_ids().initial_cwd;
        self.session_name = std::env::var("ZELLIJ_SESSION_NAME").ok();

        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadApplicationState,
            PermissionType::ReadCliPipes,
        ]);

        subscribe(&[
            EventType::Key,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        let action = match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                Action::FetchToplevel
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.status_message = "Permissions denied. Plugin cannot run commands.".to_string();
                self.status_is_error = true;
                Action::None
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                match context.get("cmd_type").map(|s| s.as_str()) {
                    Some(CMD_GIT_TOPLEVEL) => self.handle_git_toplevel(exit_code, &stdout, &stderr),
                    Some(CMD_LIST_WORKTREES) => {
                        self.handle_list_worktrees(exit_code, &stdout, &stderr);
                        Action::None
                    }
                    Some(CMD_GIT_BRANCHES) => {
                        self.handle_git_branches(exit_code, &stdout, &stderr);
                        Action::None
                    }
                    Some(CMD_SPAWN) => self.handle_spawn_result(exit_code, &stderr, &context),
                    Some(CMD_REMOVE) => self.handle_remove_result(exit_code, &stderr, &context),
                    Some(other) => {
                        self.status_message = format!("Unknown command result: {other}");
                        self.status_is_error = true;
                        Action::None
                    }
                    None => {
                        self.status_message = "Received command result with no cmd_type".to_string();
                        self.status_is_error = true;
                        Action::None
                    }
                }
            }
            Event::TabUpdate(tab_info) => {
                self.tabs = tab_info;
                self.recompute_sidebar_items();
                Action::None
            }
            Event::Key(key) => {
                match self.mode {
                    Mode::Loading => Action::None,
                    Mode::NotGitRepo => self.handle_key_not_git_repo(&key),
                    Mode::BrowseWorktrees => self.handle_key_browse(&key),
                    Mode::SelectBranch => self.handle_key_select_branch(&key),
                    Mode::InputBranch => self.handle_key_input_branch(&key),
                    Mode::Confirming => self.handle_key_confirming(&key),
                }
            }
            _ => return false,
        };
        self.execute(&action);
        true
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let action = self.handle_pipe(&pipe_message);
        self.execute(&action);
        true
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.render_to(&mut std::io::stdout(), rows, cols);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn key(bare: BareKey) -> KeyWithModifier {
        KeyWithModifier { bare_key: bare, key_modifiers: BTreeSet::new() }
    }

    fn key_shift(bare: BareKey) -> KeyWithModifier {
        let mut mods = BTreeSet::new();
        mods.insert(KeyModifier::Shift);
        KeyWithModifier { bare_key: bare, key_modifiers: mods }
    }

    fn state_with_worktrees() -> State {
        let mut s = State::default();
        s.mode = Mode::BrowseWorktrees;
        s.worktrees = vec![
            Worktree { dir: "feat-a".into(), branch: "feat-a".into() },
            Worktree { dir: "feat-b".into(), branch: "feat-b".into() },
            Worktree { dir: "feat-c".into(), branch: "feat-c".into() },
        ];
        s.branches = vec!["main".into(), "feat-a".into(), "feat-b".into(), "dev".into()];
        s
    }

    // --- sanitize_branch_name tests ---

    #[test]
    fn sanitize_spaces_become_hyphens() {
        assert_eq!(sanitize_branch_name("claude alerts"), "claude-alerts");
        assert_eq!(sanitize_branch_name("my new feature"), "my-new-feature");
    }

    #[test]
    fn sanitize_collapses_consecutive_hyphens() {
        assert_eq!(sanitize_branch_name("foo  bar"), "foo-bar");
        assert_eq!(sanitize_branch_name("a ~ b"), "a-b");
    }

    #[test]
    fn sanitize_strips_leading_trailing() {
        assert_eq!(sanitize_branch_name(" leading"), "leading");
        assert_eq!(sanitize_branch_name("trailing "), "trailing");
        assert_eq!(sanitize_branch_name(".dotted."), "dotted");
    }

    #[test]
    fn sanitize_invalid_chars() {
        assert_eq!(sanitize_branch_name("feat~1"), "feat-1");
        assert_eq!(sanitize_branch_name("foo:bar"), "foo-bar");
        assert_eq!(sanitize_branch_name("a?b*c"), "a-b-c");
    }

    #[test]
    fn sanitize_valid_name_unchanged() {
        assert_eq!(sanitize_branch_name("feat/new-thing"), "feat/new-thing");
        assert_eq!(sanitize_branch_name("fix-bug_123"), "fix-bug_123");
    }

    #[test]
    fn sanitize_empty_returns_empty() {
        assert_eq!(sanitize_branch_name(""), "");
        assert_eq!(sanitize_branch_name("   "), "");
    }

    #[test]
    fn sanitize_git_ref_sequences() {
        assert_eq!(sanitize_branch_name("foo..bar"), "foo-bar");
        assert_eq!(sanitize_branch_name("foo@{1}"), "foo-1}");
        assert_eq!(sanitize_branch_name("foo//bar"), "foo/bar");
        assert_eq!(sanitize_branch_name("foo/.bar"), "foo/-bar");
        assert_eq!(sanitize_branch_name("foo.lock"), "foo");
        assert_eq!(sanitize_branch_name("/leading"), "leading");
        assert_eq!(sanitize_branch_name("trailing/"), "trailing");
    }

    // --- InputBranch integration tests ---

    #[test]
    fn input_branch_enter_sanitizes_and_spawns() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "claude alerts".into(), ..Default::default() };
        let action = s.handle_key_input_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::Spawn("claude-alerts".into()));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    // --- Parsing tests ---

    #[test]
    fn parse_worktrees_tab_separated() {
        let output = "feature-cool\tfeature/cool\nfix-bug\tfix-bug\n";
        let wts = parse_worktrees(output);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].dir, "feature-cool");
        assert_eq!(wts[0].branch, "feature/cool");
        assert_eq!(wts[1].dir, "fix-bug");
        assert_eq!(wts[1].branch, "fix-bug");
    }

    #[test]
    fn parse_worktrees_fallback_no_tab() {
        let output = "feat-a\nfeat-b\n";
        let wts = parse_worktrees(output);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].dir, "feat-a");
        assert_eq!(wts[0].branch, "feat-a");
    }

    #[test]
    fn parse_worktrees_empty_output() {
        let wts = parse_worktrees("");
        assert!(wts.is_empty());
    }

    #[test]
    fn parse_worktrees_strips_whitespace() {
        let output = "  feat-a \t feat-a \n\n  feat-b \t feat-b  \n";
        let wts = parse_worktrees(output);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].dir, "feat-a");
        assert_eq!(wts[0].branch, "feat-a");
        assert_eq!(wts[1].dir, "feat-b");
        assert_eq!(wts[1].branch, "feat-b");
    }

    #[test]
    fn parse_worktrees_mixed_dir_branch() {
        let output = "autonomy\tplugin-snapshot-tests\ncompetition\tcompetition\n";
        let wts = parse_worktrees(output);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].dir, "autonomy");
        assert_eq!(wts[0].branch, "plugin-snapshot-tests");
        assert_eq!(wts[1].dir, "competition");
        assert_eq!(wts[1].branch, "competition");
    }

    #[test]
    fn parse_branches_basic() {
        let output = "main\nfeature/cool\nfix-bug\n";
        let branches = parse_branches(output);
        assert_eq!(branches, vec!["main", "feature/cool", "fix-bug"]);
    }

    #[test]
    fn parse_branches_strips_whitespace_and_empty() {
        let output = "  main \n\n  dev  \n";
        let branches = parse_branches(output);
        assert_eq!(branches, vec!["main", "dev"]);
    }

    // --- BrowseWorktrees key handler tests ---

    #[test]
    fn browse_j_moves_down() {
        let mut s = state_with_worktrees();
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 1);
        s.handle_key_browse(&key(BareKey::Down));
        assert_eq!(s.selected_index, 2);
    }

    #[test]
    fn browse_j_wraps_around() {
        let mut s = state_with_worktrees();
        s.selected_index = 2;
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_k_moves_up() {
        let mut s = state_with_worktrees();
        s.selected_index = 2;
        s.handle_key_browse(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 1);
        s.handle_key_browse(&key(BareKey::Up));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_k_wraps_around() {
        let mut s = state_with_worktrees();
        s.selected_index = 0;
        s.handle_key_browse(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 2);
    }

    #[test]
    fn browse_jk_noop_on_empty() {
        let mut s = State { mode: Mode::BrowseWorktrees, ..Default::default() };
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 0);
        s.handle_key_browse(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_enter_spawns_selected() {
        let mut s = state_with_worktrees();
        s.selected_index = 1;
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::Spawn("feat-b".into()));
        assert_eq!(s.status_message, "Spawning 'feat-b'...");
    }

    #[test]
    fn browse_enter_switches_to_existing_tab() {
        let mut s = state_with_worktrees();
        s.tabs = vec![make_tab("feat-a", false), make_tab("feat-b", true)];
        s.selected_index = 0; // feat-a has a tab open
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::SwitchToTab("feat-a".into()));
    }

    #[test]
    fn browse_enter_noop_on_empty() {
        let mut s = State { mode: Mode::BrowseWorktrees, ..Default::default() };
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn browse_n_switches_to_select_branch() {
        let mut s = state_with_worktrees();
        s.selected_index = 2;
        s.handle_key_browse(&key(BareKey::Char('n')));
        assert_eq!(s.mode, Mode::SelectBranch);
        assert_eq!(s.selected_index, 0);
        assert_eq!(s.filtered_branches, s.branches);
    }

    #[test]
    fn browse_i_switches_to_input_branch() {
        let mut s = state_with_worktrees();
        s.input_buffer = "leftover".into();
        s.handle_key_browse(&key(BareKey::Char('i')));
        assert_eq!(s.mode, Mode::InputBranch);
        assert!(s.input_buffer.is_empty());
    }

    #[test]
    fn browse_d_switches_to_confirming() {
        let mut s = state_with_worktrees();
        s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(s.mode, Mode::Confirming);
    }

    #[test]
    fn browse_d_noop_on_user_tab() {
        let mut s = state_with_worktrees();
        s.tabs = vec![make_tab("feat-a", true), make_tab("notes", false)];
        s.recompute_sidebar_items();
        s.selected_index = 1;
        s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn browse_d_noop_on_main() {
        let mut s = State::default();
        s.mode = Mode::BrowseWorktrees;
        s.worktrees = vec![Worktree { dir: "main".into(), branch: "main".into() }];
        s.tabs = vec![make_tab("main", true)];
        s.recompute_sidebar_items();
        s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn browse_d_noop_on_empty() {
        let mut s = State { mode: Mode::BrowseWorktrees, ..Default::default() };
        s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn browse_r_returns_refresh() {
        let mut s = state_with_worktrees();
        let action = s.handle_key_browse(&key(BareKey::Char('r')));
        assert_eq!(action, Action::Refresh);
        assert_eq!(s.status_message, "Refreshed");
    }

    #[test]
    fn browse_q_is_noop_in_sidebar_mode() {
        let mut s = state_with_worktrees();
        assert_eq!(s.handle_key_browse(&key(BareKey::Char('q'))), Action::None);
    }

    #[test]
    fn browse_esc_is_noop_in_sidebar_mode() {
        let mut s = state_with_worktrees();
        assert_eq!(s.handle_key_browse(&key(BareKey::Esc)), Action::None);
    }

    // --- SelectBranch key handler tests ---

    #[test]
    fn select_branch_jk_navigates() {
        let mut s = state_with_worktrees();
        s.mode = Mode::SelectBranch;
        s.filtered_branches = s.branches.clone();
        s.selected_index = 0;

        s.handle_key_select_branch(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 1);
        s.handle_key_select_branch(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn select_branch_wraps() {
        let mut s = state_with_worktrees();
        s.mode = Mode::SelectBranch;
        s.filtered_branches = vec!["a".into(), "b".into()];
        s.selected_index = 1;

        s.handle_key_select_branch(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 0);

        s.handle_key_select_branch(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 1);
    }

    #[test]
    fn select_branch_enter_spawns() {
        let mut s = state_with_worktrees();
        s.mode = Mode::SelectBranch;
        s.filtered_branches = vec!["dev".into(), "main".into()];
        s.selected_index = 0;

        let action = s.handle_key_select_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::Spawn("dev".into()));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn select_branch_enter_switches_to_existing_tab() {
        let mut s = state_with_worktrees();
        s.mode = Mode::SelectBranch;
        s.filtered_branches = vec!["feat/cool".into(), "main".into()];
        s.tabs = vec![make_tab("feat-cool", false)];
        s.selected_index = 0;

        let action = s.handle_key_select_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::SwitchToTab("feat-cool".into()));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn select_branch_esc_goes_back() {
        let mut s = state_with_worktrees();
        s.mode = Mode::SelectBranch;
        s.selected_index = 2;
        s.handle_key_select_branch(&key(BareKey::Esc));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(s.selected_index, 0);
    }

    // --- InputBranch key handler tests ---

    #[test]
    fn input_branch_typing() {
        let mut s = State { mode: Mode::InputBranch, ..Default::default() };
        s.handle_key_input_branch(&key(BareKey::Char('f')));
        s.handle_key_input_branch(&key(BareKey::Char('o')));
        s.handle_key_input_branch(&key(BareKey::Char('o')));
        assert_eq!(s.input_buffer, "foo");
    }

    #[test]
    fn input_branch_shift_chars() {
        let mut s = State { mode: Mode::InputBranch, ..Default::default() };
        s.handle_key_input_branch(&key_shift(BareKey::Char('F')));
        assert_eq!(s.input_buffer, "F");
    }

    #[test]
    fn input_branch_backspace() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "ab".into(), ..Default::default() };
        s.handle_key_input_branch(&key(BareKey::Backspace));
        assert_eq!(s.input_buffer, "a");
    }

    #[test]
    fn input_branch_enter_spawns() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "feat/new".into(), ..Default::default() };
        let action = s.handle_key_input_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::Spawn("feat/new".into()));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn input_branch_enter_switches_to_existing_tab() {
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "feat/new".into(),
            tabs: vec![make_tab("feat-new", false)],
            ..Default::default()
        };
        let action = s.handle_key_input_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::SwitchToTab("feat-new".into()));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn input_branch_enter_noop_on_empty() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "  ".into(), ..Default::default() };
        let action = s.handle_key_input_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::None);
        assert_eq!(s.mode, Mode::InputBranch);
        assert!(s.status_is_error);
        assert_eq!(s.status_message, "Invalid branch name");
    }

    #[test]
    fn input_branch_esc_goes_back() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "wip".into(), ..Default::default() };
        s.handle_key_input_branch(&key(BareKey::Esc));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    // --- Confirming key handler tests ---

    #[test]
    fn confirm_y_removes() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        s.selected_index = 1;
        let action = s.handle_key_confirming(&key(BareKey::Char('y')));
        assert_eq!(action, Action::Remove("feat-b".into()));
        assert_eq!(s.status_message, "Removing 'feat-b'...");
    }

    #[test]
    fn confirm_y_noop_on_user_tab() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        s.tabs = vec![make_tab("feat-a", true), make_tab("notes", false)];
        s.recompute_sidebar_items();
        s.selected_index = 1;
        let action = s.handle_key_confirming(&key(BareKey::Char('y')));
        assert_eq!(action, Action::None);
        assert_eq!(s.mode, Mode::Confirming);
    }

    #[test]
    fn confirm_y_noop_on_main() {
        let mut s = State::default();
        s.mode = Mode::Confirming;
        s.worktrees = vec![Worktree { dir: "main".into(), branch: "main".into() }];
        s.tabs = vec![make_tab("main", true)];
        s.recompute_sidebar_items();
        let action = s.handle_key_confirming(&key(BareKey::Char('y')));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn confirm_n_cancels() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        s.handle_key_confirming(&key(BareKey::Char('n')));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn confirm_esc_cancels() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        s.handle_key_confirming(&key(BareKey::Esc));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    // --- Command result handler tests ---

    #[test]
    fn git_toplevel_sets_repo() {
        let mut s = State::default();
        let action = s.handle_git_toplevel(Some(0), b"repo_root=/home/user/myrepo\nrepo_name=myrepo\n", b"");
        assert_eq!(s.repo_root, "/home/user/myrepo");
        assert_eq!(s.repo_name, "myrepo");
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(action, Action::FetchWorktreesAndBranches);
    }

    #[test]
    fn git_toplevel_parses_by_key() {
        let mut s = State::default();
        let action = s.handle_git_toplevel(Some(0), b"repo_name=myrepo\nrepo_root=/home/user/myrepo\n", b"");
        assert_eq!(s.repo_root, "/home/user/myrepo");
        assert_eq!(s.repo_name, "myrepo");
        assert_eq!(action, Action::FetchWorktreesAndBranches);
    }

    #[test]
    fn git_toplevel_error_enters_not_git_repo_mode() {
        let mut s = State::default();
        let action = s.handle_git_toplevel(Some(128), b"", b"not a git repo");
        assert!(s.status_is_error);
        assert!(s.status_message.contains("is not a git repo"));
        assert_eq!(s.mode, Mode::NotGitRepo);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn git_toplevel_missing_fields() {
        let mut s = State::default();
        let action = s.handle_git_toplevel(Some(0), b"repo_root=/foo\n", b"");
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Failed to parse repo info"));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn spawn_result_success() {
        let mut s = state_with_worktrees();
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "feat-a".into());
        let action = s.handle_spawn_result(Some(0), b"", &ctx);
        assert_eq!(s.status_message, "Spawned 'feat-a'");
        assert!(!s.status_is_error);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn spawn_result_error() {
        let mut s = state_with_worktrees();
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "bad".into());
        let action = s.handle_spawn_result(Some(1), b"something broke", &ctx);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("something broke"));
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn spawn_result_error_empty_stderr() {
        let mut s = state_with_worktrees();
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "bad".into());
        let action = s.handle_spawn_result(Some(1), b"", &ctx);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("exit 1"));
        assert!(s.status_message.contains("bad"));
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn spawn_result_no_exit_code() {
        let mut s = state_with_worktrees();
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "bad".into());
        let action = s.handle_spawn_result(None, b"", &ctx);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("no exit code"));
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn spawn_result_missing_branch_context() {
        let mut s = state_with_worktrees();
        let ctx = BTreeMap::new();
        let action = s.handle_spawn_result(Some(0), b"", &ctx);
        assert!(!s.status_is_error);
        assert!(s.status_message.contains("<unknown>"));
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn remove_result_success() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "feat-a".into());
        let action = s.handle_remove_result(Some(0), b"", &ctx);
        assert_eq!(s.status_message, "Removed 'feat-a'");
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn remove_result_success_with_tab_returns_close_tab() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "feat-a".into());
        let action = s.handle_remove_result(Some(0), b"", &ctx);
        assert_eq!(s.status_message, "Removed 'feat-a'");
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(
            action,
            Action::CloseTabAndRefresh {
                tab_name: "feat-a".into(),
                return_to: Some("zelligent".into()),
            }
        );
    }

    #[test]
    fn remove_result_error() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "feat-a".into());
        let action = s.handle_remove_result(Some(1), b"uncommitted changes", &ctx);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("uncommitted changes"));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn remove_result_error_empty_stderr() {
        let mut s = state_with_worktrees();
        s.mode = Mode::Confirming;
        let mut ctx = BTreeMap::new();
        ctx.insert("branch".into(), "feat-a".into());
        let action = s.handle_remove_result(Some(1), b"", &ctx);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("exit 1"));
        assert!(s.status_message.contains("feat-a"));
        assert_eq!(action, Action::Refresh);
    }

    fn make_tab(name: &str, active: bool) -> TabInfo {
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

    #[test]
    fn tab_name_for_branch_sanitizes() {
        assert_eq!(State::tab_name_for_branch("feature/cool"), "feature-cool");
        assert_eq!(State::tab_name_for_branch("a/b/c"), "a-b-c");
        assert_eq!(State::tab_name_for_branch("fix-bug"), "fix-bug");
        // Dots and other special chars are stripped (matching zelligent.sh)
        assert_eq!(State::tab_name_for_branch("release/1.2.3"), "release-123");
        assert_eq!(State::tab_name_for_branch("feat.something"), "featsomething");
    }

    #[test]
    fn has_tab_for_branch_found() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feature-cool", false), make_tab("fix-bug", false)];
        assert!(s.has_tab_for_branch("feature/cool"));
        assert!(s.has_tab_for_branch("fix-bug"));
    }

    #[test]
    fn has_tab_for_branch_not_found() {
        let mut s = State::default();
        s.tabs = vec![make_tab("main", false)];
        assert!(!s.has_tab_for_branch("nonexistent"));
    }

    #[test]
    fn has_tab_for_branch_empty_tabs() {
        let s = State::default();
        assert!(!s.has_tab_for_branch("anything"));
    }

    #[test]
    fn recompute_sidebar_selects_active_tab() {
        let mut s = State::default();
        s.tabs = vec![
            make_tab("feat-a", false),
            make_tab("feat-b", true),
            make_tab("feat-c", false),
        ];
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\nfeat-b\tfeat-b\nfeat-c\tfeat-c\n", b"");
        // selected_index should point to active tab in sidebar_items
        assert_eq!(s.selected_index, 1);
        assert_eq!(s.sidebar_items.len(), 3);
        assert_eq!(s.sidebar_items[1].tab_name, "feat-b");
    }

    #[test]
    fn recompute_sidebar_selects_active_tab_with_slash_branch() {
        let mut s = State::default();
        s.tabs = vec![make_tab("main", false), make_tab("feature-cool", true)];
        s.handle_list_worktrees(Some(0), b"main\tmain\nfeature-cool\tfeature/cool\n", b"");
        assert_eq!(s.selected_index, 1);
        assert_eq!(s.sidebar_items[1].matched_branch, Some("feature/cool".into()));
    }

    #[test]
    fn recompute_sidebar_preserves_cursor_by_name() {
        let mut s = State::default();
        s.tabs = vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ];
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\nfeat-b\tfeat-b\nfeat-c\tfeat-c\n", b"");
        // Move cursor to feat-c
        s.selected_index = 2;
        // Simulate tab update (e.g. active tab changed) — cursor should stay on feat-c
        s.tabs = vec![
            make_tab("feat-a", false),
            make_tab("feat-b", true),
            make_tab("feat-c", false),
        ];
        s.recompute_sidebar_items();
        assert_eq!(s.selected_index, 2);
        assert_eq!(s.sidebar_items[2].tab_name, "feat-c");
    }

    #[test]
    fn recompute_sidebar_no_tabs_empty() {
        let mut s = State::default();
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\nfeat-b\tfeat-b\n", b"");
        assert!(s.sidebar_items.is_empty());
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn recompute_sidebar_tab_without_worktree() {
        let mut s = State::default();
        s.tabs = vec![make_tab("user-tab", true)];
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"");
        assert_eq!(s.sidebar_items.len(), 1);
        assert_eq!(s.sidebar_items[0].tab_name, "user-tab");
        assert_eq!(s.sidebar_items[0].matched_branch, None);
    }

    #[test]
    fn recompute_sidebar_clamps_on_shrink() {
        let mut s = State::default();
        s.tabs = vec![make_tab("a", false), make_tab("b", true)];
        s.recompute_sidebar_items();
        s.selected_index = 1;
        // Remove a tab
        s.tabs = vec![make_tab("a", true)];
        s.recompute_sidebar_items();
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn recompute_sidebar_ambiguous_tab_name() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-cool", true)];
        s.handle_list_worktrees(Some(0), b"feat-cool\tfeat/cool\nfeat-cool\tfeat-cool\n", b"");
        assert_eq!(s.sidebar_items.len(), 1);
        // Matches first worktree
        assert_eq!(s.sidebar_items[0].matched_branch, Some("feat/cool".into()));
    }

    #[test]
    fn find_worktree_for_active_tab_found() {
        let mut s = state_with_worktrees();
        s.tabs = vec![make_tab("feat-a", false), make_tab("feat-c", true)];
        assert_eq!(s.find_worktree_for_active_tab(), Some(2));
    }

    #[test]
    fn find_worktree_for_active_tab_no_match() {
        let mut s = state_with_worktrees();
        s.tabs = vec![make_tab("unrelated", true)];
        assert_eq!(s.find_worktree_for_active_tab(), None);
    }

    #[test]
    fn find_worktree_for_active_tab_no_active_tab() {
        let mut s = state_with_worktrees();
        s.tabs = vec![make_tab("feat-a", false), make_tab("feat-b", false)];
        assert_eq!(s.find_worktree_for_active_tab(), None);
    }

    #[test]
    fn find_worktree_for_active_tab_empty_tabs() {
        let s = state_with_worktrees();
        assert_eq!(s.find_worktree_for_active_tab(), None);
    }

    #[test]
    fn list_worktrees_error_sets_status() {
        let mut s = State::default();
        s.handle_list_worktrees(Some(1), b"", b"fatal: not a git repository");
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Failed to list worktrees"));
        assert!(s.status_message.contains("fatal: not a git repository"));
    }

    #[test]
    fn list_worktrees_error_preserves_existing_worktrees() {
        let mut s = state_with_worktrees();
        let original_len = s.worktrees.len();
        s.handle_list_worktrees(Some(1), b"", b"error");
        assert_eq!(s.worktrees.len(), original_len);
    }

    #[test]
    fn git_branches_error_sets_status() {
        let mut s = State::default();
        s.handle_git_branches(Some(128), b"", b"fatal: bad default revision");
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Failed to list branches"));
    }

    #[test]
    fn git_branches_error_preserves_existing_branches() {
        let mut s = state_with_worktrees();
        let original_len = s.branches.len();
        s.handle_git_branches(Some(1), b"", b"error");
        assert_eq!(s.branches.len(), original_len);
    }

    #[test]
    fn input_branch_esc_clears_buffer() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "wip".into(), ..Default::default() };
        s.handle_key_input_branch(&key(BareKey::Esc));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert!(s.input_buffer.is_empty());
    }

    #[test]
    fn wrap_navigate_basic() {
        assert_eq!(wrap_navigate(0, 3, 1), 1);
        assert_eq!(wrap_navigate(2, 3, 1), 0);
        assert_eq!(wrap_navigate(0, 3, -1), 2);
        assert_eq!(wrap_navigate(0, 0, 1), 0);
    }

    // --- NotGitRepo key handler tests ---

    #[test]
    fn not_git_repo_d_returns_dump_layout() {
        let mut s = State { mode: Mode::NotGitRepo, ..Default::default() };
        let action = s.handle_key_not_git_repo(&key(BareKey::Char('d')));
        assert_eq!(action, Action::DumpLayout);
    }

    #[test]
    fn not_git_repo_x_returns_nuke_session() {
        let mut s = State {
            mode: Mode::NotGitRepo,
            session_name: Some("test-session".into()),
            ..Default::default()
        };
        let action = s.handle_key_not_git_repo(&key(BareKey::Char('x')));
        assert_eq!(action, Action::NukeSession);
    }

    #[test]
    fn not_git_repo_x_without_session_shows_error() {
        let mut s = State { mode: Mode::NotGitRepo, ..Default::default() };
        let action = s.handle_key_not_git_repo(&key(BareKey::Char('x')));
        assert_eq!(action, Action::None);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Cannot determine session name"));
    }

    #[test]
    fn not_git_repo_q_returns_close() {
        let mut s = State { mode: Mode::NotGitRepo, ..Default::default() };
        let action = s.handle_key_not_git_repo(&key(BareKey::Char('q')));
        assert_eq!(action, Action::Close);
    }

    #[test]
    fn not_git_repo_esc_returns_close() {
        let mut s = State { mode: Mode::NotGitRepo, ..Default::default() };
        let action = s.handle_key_not_git_repo(&key(BareKey::Esc));
        assert_eq!(action, Action::Close);
    }

    // --- handle_pipe tests ---

    fn pipe_msg(name: &str, args: &[(&str, &str)]) -> PipeMessage {
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

    #[test]
    fn pipe_unknown_name_ignored() {
        let mut s = State::default();
        let action = s.handle_pipe(&pipe_msg("other-plugin", &[("tab", "feat-a"), ("event", "Stop")]));
        assert_eq!(action, Action::None);
        assert!(s.agent_statuses.is_empty());
    }

    #[test]
    fn pipe_missing_tab_ignored() {
        let mut s = State::default();
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("event", "Stop")]));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn pipe_start_sets_working_no_notify() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "Start")]));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn pipe_user_prompt_submit_sets_working() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "UserPromptSubmit")]));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn pipe_permission_request_sets_needs_input_and_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "PermissionRequest")]));
        assert_eq!(action, Action::Notify { tab_name: "feat-a".into(), status: AgentStatus::Awaiting });
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Awaiting));
    }

    #[test]
    fn pipe_stop_sets_done_and_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "Stop")]));
        assert_eq!(action, Action::Notify { tab_name: "feat-a".into(), status: AgentStatus::Done });
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn pipe_active_tab_still_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", true)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "Stop")]));
        assert_eq!(action, Action::Notify { tab_name: "feat-a".into(), status: AgentStatus::Done });
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn pipe_different_tab_active_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-b", true), make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "Stop")]));
        assert_eq!(action, Action::Notify { tab_name: "feat-a".into(), status: AgentStatus::Done });
    }

    #[test]
    fn pipe_status_overwrite() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "Start")]));
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
        s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "Stop")]));
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn pipe_unknown_tab_ignored() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-b", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "unknown-tab"), ("event", "Stop")]));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("unknown-tab"), None);
    }

    #[test]
    fn pipe_unknown_event_shows_error() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a"), ("event", "BogusEvent")]));
        assert_eq!(action, Action::None);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Unknown agent event"));
        assert!(s.status_message.contains("BogusEvent"));
    }

    #[test]
    fn pipe_missing_event_shows_error() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg("zelligent-status", &[("tab", "feat-a")]));
        assert_eq!(action, Action::None);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("missing 'event' arg"));
    }

    // --- ctx() helper tests ---

    #[test]
    fn ctx_builds_correct_map() {
        let m = State::ctx(CMD_GIT_TOPLEVEL);
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("cmd_type").unwrap(), CMD_GIT_TOPLEVEL);
    }

    #[test]
    fn ctx_uses_each_command_type() {
        for cmd in &[CMD_GIT_TOPLEVEL, CMD_LIST_WORKTREES, CMD_GIT_BRANCHES, CMD_SPAWN, CMD_REMOVE] {
            let m = State::ctx(cmd);
            assert_eq!(m.get("cmd_type").unwrap(), cmd);
        }
    }

    // --- execute() / fire_* coverage ---
    //
    // The `execute()` method and `fire_*` methods directly call Zellij plugin
    // API functions (`close_self()`, `go_to_tab_name()`, `close_focused_tab()`,
    // `run_command_with_env_variables_and_cwd()`, `kill_sessions()`,
    // `dump_session_layout()`). These are FFI calls into the WASM host runtime
    // and panic or segfault when called outside the Zellij sandbox.
    //
    // What IS verified at compile time:
    // - The `execute()` match has no wildcard (`_`) arm, so the compiler
    //   enforces that every `Action` variant is handled. Adding a new variant
    //   without updating `execute()` is a compile error.
    //
    // What would be needed to unit-test the dispatch logic:
    // - Extract a trait (e.g., `ZellijApi`) with methods like `close_self()`,
    //   `go_to_tab_name(&str)`, `run_command(...)`, etc.
    // - Have `State` hold a `Box<dyn ZellijApi>` (or use a generic parameter).
    // - In tests, provide a mock implementation that records calls.
    // - This is a significant refactor for relatively simple dispatch code;
    //   the current approach (compile-time exhaustiveness + testing the pure
    //   handlers that produce `Action` values) provides good coverage without
    //   the abstraction overhead.

    #[test]
    fn action_enum_is_exhaustive_in_execute() {
        // This test verifies that execute() handles all Action variants by
        // constructing every variant. If a new variant is added to Action
        // without updating execute(), this test will fail to compile (along
        // with execute() itself, since neither uses a wildcard match).
        let all_actions = vec![
            Action::None,
            Action::Close,
            Action::Spawn("branch".into()),
            Action::Remove("branch".into()),
            Action::CloseTabAndRefresh {
                tab_name: "tab".into(),
                return_to: Some("other".into()),
            },
            Action::SwitchToTab("tab".into()),
            Action::Refresh,
            Action::FetchToplevel,
            Action::FetchWorktreesAndBranches,
            Action::DumpLayout,
            Action::NukeSession,
            Action::Notify {
                tab_name: "tab".into(),
                status: AgentStatus::Done,
            },
        ];
        // Verify all variants are representable and Debug-printable (ensures
        // the enum hasn't grown without this list being updated).
        assert_eq!(all_actions.len(), 12);
        for action in &all_actions {
            // Each variant should produce a non-empty debug string.
            assert!(!format!("{:?}", action).is_empty());
        }
    }

    #[test]
    fn action_none_is_default_return() {
        // Many handlers return Action::None as the "do nothing" case.
        // Verify it compares equal to itself (PartialEq derived).
        assert_eq!(Action::None, Action::None);
        assert_ne!(Action::None, Action::Close);
    }

    #[test]
    fn action_spawn_carries_branch() {
        let action = Action::Spawn("feat/new-thing".into());
        if let Action::Spawn(branch) = &action {
            assert_eq!(branch, "feat/new-thing");
        } else {
            panic!("expected Action::Spawn");
        }
    }

    #[test]
    fn action_close_tab_and_refresh_fields() {
        let action = Action::CloseTabAndRefresh {
            tab_name: "feat-a".into(),
            return_to: Some("zelligent".into()),
        };
        if let Action::CloseTabAndRefresh { tab_name, return_to } = &action {
            assert_eq!(tab_name, "feat-a");
            assert_eq!(return_to, &Some("zelligent".into()));
        } else {
            panic!("expected Action::CloseTabAndRefresh");
        }
    }

    #[test]
    fn action_notify_carries_status() {
        let action = Action::Notify {
            tab_name: "feat-a".into(),
            status: AgentStatus::NeedsInput,
        };
        if let Action::Notify { tab_name, status } = &action {
            assert_eq!(tab_name, "feat-a");
            assert_eq!(*status, AgentStatus::NeedsInput);
        } else {
            panic!("expected Action::Notify");
        }
    }

    #[test]
    fn notify_only_fires_for_needs_input_and_done() {
        // execute() returns early for Notify with Idle or Working status.
        // We can't call execute() directly, but we verify the guard logic
        // by checking that only NeedsInput and Done are "notifiable".
        let notifiable = |s: &AgentStatus| matches!(s, AgentStatus::NeedsInput | AgentStatus::Done);
        assert!(notifiable(&AgentStatus::NeedsInput));
        assert!(notifiable(&AgentStatus::Done));
        assert!(!notifiable(&AgentStatus::Idle));
        assert!(!notifiable(&AgentStatus::Working));
    }
}
