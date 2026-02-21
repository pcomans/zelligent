mod ui;

use std::collections::BTreeMap;
use std::path::PathBuf;
use zellij_tile::prelude::*;

// Command context keys used to route RunCommandResult
const CMD_GIT_TOPLEVEL: &str = "git_toplevel";
const CMD_LIST_WORKTREES: &str = "list_worktrees";
const CMD_GIT_BRANCHES: &str = "git_branches";
const CMD_SPAWN: &str = "spawn";
const CMD_REMOVE: &str = "remove";

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Loading,
    BrowseWorktrees,
    SelectBranch,
    InputBranch,
    Confirming,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Loading
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Worktree {
    pub path: String,
    pub branch: String,
}

/// Actions returned by key/event handlers, executed by the plugin shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    Close,
    Spawn(String),
    Remove(String),
    Refresh,
    FetchToplevel,
    FetchWorktreesAndBranches,
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
    pub spawn_agent_path: String,
    pub tabs: Vec<TabInfo>,
}

register_plugin!(State);

/// Parse `git worktree list --porcelain` output, returning only worktrees managed by spawn-agent.
/// `spawn_suffix` is `/.spawn-agent/<repo_name>/` — matched anywhere in the path to avoid
/// depending on $HOME which isn't available in the WASM sandbox.
pub fn parse_worktrees(output: &str, spawn_suffix: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current_path = String::new();

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            if current_path.contains(spawn_suffix) {
                let branch = branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string();
                worktrees.push(Worktree {
                    path: current_path.clone(),
                    branch,
                });
            }
        }
    }

    worktrees
}

/// Parse `git branch --format=%(refname:short)` output into a list of branch names.
pub fn parse_branches(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

impl State {
    fn ctx(cmd_type: &str) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("cmd_type".to_string(), cmd_type.to_string());
        m
    }

    fn fire_git_toplevel(&self) {
        // Use --git-common-dir to get the main repo's .git dir even from a worktree.
        // This ensures repo_name is always the main repo, not a worktree subdirectory.
        run_command(
            &["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            Self::ctx(CMD_GIT_TOPLEVEL),
        );
    }

    fn fire_list_worktrees(&self) {
        run_command_with_env_variables_and_cwd(
            &["git", "worktree", "list", "--porcelain"],
            BTreeMap::new(),
            PathBuf::from(&self.repo_root),
            Self::ctx(CMD_LIST_WORKTREES),
        );
    }

    fn fire_git_branches(&self) {
        run_command_with_env_variables_and_cwd(
            &["git", "branch", "--format=%(refname:short)"],
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
            &[&self.spawn_agent_path, branch, &self.agent_cmd],
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
            &[&self.spawn_agent_path, "remove", branch],
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
            Action::Refresh => {
                self.fire_list_worktrees();
                self.fire_git_branches();
            }
            Action::FetchToplevel => self.fire_git_toplevel(),
            Action::FetchWorktreesAndBranches => {
                self.fire_list_worktrees();
                self.fire_git_branches();
            }
        }
    }

    // --- Pure state handlers (no zellij calls, fully testable) ---

    pub fn handle_git_toplevel(&mut self, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Action {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            self.status_message = format!("Not a git repo: {err}");
            self.status_is_error = true;
            return Action::None;
        }
        // --git-common-dir returns e.g. "/path/to/repo/.git" — strip the /.git suffix
        let git_dir = String::from_utf8_lossy(stdout).trim().to_string();
        let root = git_dir
            .strip_suffix("/.git")
            .unwrap_or(&git_dir)
            .to_string();
        self.repo_name = root
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();
        self.repo_root = root;
        self.mode = Mode::BrowseWorktrees;
        Action::FetchWorktreesAndBranches
    }

    pub fn handle_list_worktrees(&mut self, _exit_code: Option<i32>, stdout: &[u8], spawn_prefix: &str) {
        let output = String::from_utf8_lossy(stdout);
        self.worktrees = parse_worktrees(&output, spawn_prefix);
        if self.selected_index >= self.worktrees.len() && !self.worktrees.is_empty() {
            self.selected_index = self.worktrees.len() - 1;
        }
    }

    pub fn handle_git_branches(&mut self, _exit_code: Option<i32>, stdout: &[u8]) {
        let output = String::from_utf8_lossy(stdout);
        self.branches = parse_branches(&output);
    }

    pub fn handle_spawn_result(&mut self, exit_code: Option<i32>, stderr: &[u8], context: &BTreeMap<String, String>) -> Action {
        let branch = context.get("branch").cloned().unwrap_or_default();
        if exit_code == Some(0) {
            self.status_message = format!("Spawned '{branch}'");
            self.status_is_error = false;
        } else {
            let err = String::from_utf8_lossy(stderr).trim().to_string();
            self.status_message = format!("Error: {err}");
            self.status_is_error = true;
        }
        Action::Refresh
    }

    pub fn handle_remove_result(&mut self, exit_code: Option<i32>, stderr: &[u8], context: &BTreeMap<String, String>) -> Action {
        let branch = context.get("branch").cloned().unwrap_or_default();
        if exit_code == Some(0) {
            self.status_message = format!("Removed '{branch}'");
            self.status_is_error = false;
            #[cfg(target_arch = "wasm32")]
            if let Some(idx) = self.tab_index_for_branch(&branch) {
                close_tab_with_index(idx);
            }
        } else {
            let err = String::from_utf8_lossy(stderr).trim().to_string();
            self.status_message = format!("Remove failed: {err}");
            self.status_is_error = true;
        }
        self.mode = Mode::BrowseWorktrees;
        Action::Refresh
    }

    /// Find the Zellij tab position associated with a branch name.
    /// Tab names use the branch with `/` replaced by `-` (matching spawn-agent.sh).
    pub fn tab_index_for_branch(&self, branch: &str) -> Option<usize> {
        let tab_name = branch.replace('/', "-");
        self.tabs.iter().find(|t| t.name == tab_name).map(|t| t.position)
    }

    pub fn handle_key_browse(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('j') | BareKey::Down => {
                    if !self.worktrees.is_empty() {
                        self.selected_index = (self.selected_index + 1) % self.worktrees.len();
                    }
                }
                BareKey::Char('k') | BareKey::Up => {
                    if !self.worktrees.is_empty() {
                        self.selected_index = if self.selected_index == 0 {
                            self.worktrees.len() - 1
                        } else {
                            self.selected_index - 1
                        };
                    }
                }
                BareKey::Enter => {
                    if let Some(wt) = self.worktrees.get(self.selected_index) {
                        let branch = wt.branch.clone();
                        self.status_message = format!("Spawning '{branch}'...");
                        self.status_is_error = false;
                        return Action::Spawn(branch);
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
                    if !self.worktrees.is_empty() {
                        self.mode = Mode::Confirming;
                    }
                }
                BareKey::Char('r') => {
                    self.status_message = "Refreshed".to_string();
                    self.status_is_error = false;
                    return Action::Refresh;
                }
                BareKey::Char('q') | BareKey::Esc => {
                    return Action::Close;
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
                    if !self.filtered_branches.is_empty() {
                        self.selected_index = (self.selected_index + 1) % self.filtered_branches.len();
                    }
                }
                BareKey::Char('k') | BareKey::Up => {
                    if !self.filtered_branches.is_empty() {
                        self.selected_index = if self.selected_index == 0 {
                            self.filtered_branches.len() - 1
                        } else {
                            self.selected_index - 1
                        };
                    }
                }
                BareKey::Enter => {
                    if let Some(branch) = self.filtered_branches.get(self.selected_index).cloned() {
                        self.status_message = format!("Spawning '{branch}'...");
                        self.status_is_error = false;
                        self.mode = Mode::BrowseWorktrees;
                        return Action::Spawn(branch);
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
                let branch = self.input_buffer.trim().to_string();
                if !branch.is_empty() {
                    self.status_message = format!("Spawning '{branch}'...");
                    self.status_is_error = false;
                    self.mode = Mode::BrowseWorktrees;
                    return Action::Spawn(branch);
                }
            }
            BareKey::Esc if no_mod => {
                self.mode = Mode::BrowseWorktrees;
                self.selected_index = 0;
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
                    if let Some(wt) = self.worktrees.get(self.selected_index) {
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
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.agent_cmd = configuration
            .get("agent_cmd")
            .cloned()
            .unwrap_or_else(|| "claude".to_string());

        self.spawn_agent_path = configuration
            .get("spawn_agent_path")
            .cloned()
            .unwrap_or_else(|| "spawn-agent".to_string());

        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadApplicationState,
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
                        let suffix = format!("/.spawn-agent/{}/", self.repo_name);
                        self.handle_list_worktrees(exit_code, &stdout, &suffix);
                        Action::None
                    }
                    Some(CMD_GIT_BRANCHES) => {
                        self.handle_git_branches(exit_code, &stdout);
                        Action::None
                    }
                    Some(CMD_SPAWN) => self.handle_spawn_result(exit_code, &stderr, &context),
                    Some(CMD_REMOVE) => self.handle_remove_result(exit_code, &stderr, &context),
                    _ => Action::None,
                }
            }
            Event::TabUpdate(tab_info) => {
                self.tabs = tab_info;
                Action::None
            }
            Event::Key(key) => {
                match self.mode {
                    Mode::Loading => Action::None,
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

    fn render(&mut self, rows: usize, cols: usize) {
        match self.mode {
            Mode::Loading => {
                ui::render_header("loading...", cols);
                println!();
                println!("  Waiting for permissions...");
            }
            Mode::BrowseWorktrees => {
                ui::render_header(&self.repo_name, cols);
                ui::render_worktree_list(&self.worktrees, self.selected_index, rows);
                ui::render_status(&self.status_message, self.status_is_error);
                ui::render_footer(&self.mode);
            }
            Mode::SelectBranch => {
                ui::render_header(&self.repo_name, cols);
                ui::render_branch_list(&self.filtered_branches, self.selected_index, rows);
                ui::render_footer(&self.mode);
            }
            Mode::InputBranch => {
                ui::render_header(&self.repo_name, cols);
                ui::render_input(&self.input_buffer);
                ui::render_footer(&self.mode);
            }
            Mode::Confirming => {
                ui::render_header(&self.repo_name, cols);
                if let Some(wt) = self.worktrees.get(self.selected_index) {
                    ui::render_confirm(&wt.branch);
                }
            }
        }
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
            Worktree { path: "/wt/feat-a".into(), branch: "feat-a".into() },
            Worktree { path: "/wt/feat-b".into(), branch: "feat-b".into() },
            Worktree { path: "/wt/feat-c".into(), branch: "feat-c".into() },
        ];
        s.branches = vec!["main".into(), "feat-a".into(), "feat-b".into(), "dev".into()];
        s
    }

    // --- Parsing tests ---

    #[test]
    fn parse_worktrees_filters_by_suffix() {
        let output = "\
worktree /Users/me/code/myrepo
HEAD abc123
branch refs/heads/main

worktree /Users/me/.spawn-agent/myrepo/feature/cool
HEAD def456
branch refs/heads/feature/cool

worktree /Users/me/.spawn-agent/myrepo/fix-bug
HEAD 789abc
branch refs/heads/fix-bug

worktree /Users/me/.spawn-agent/other-repo/feature/cool
HEAD 111222
branch refs/heads/feature/cool
";
        let suffix = "/.spawn-agent/myrepo/";
        let wts = parse_worktrees(output, suffix);

        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].branch, "feature/cool");
        assert_eq!(wts[0].path, "/Users/me/.spawn-agent/myrepo/feature/cool");
        assert_eq!(wts[1].branch, "fix-bug");
    }

    #[test]
    fn parse_worktrees_empty_output() {
        let wts = parse_worktrees("", "/.spawn-agent/x/");
        assert!(wts.is_empty());
    }

    #[test]
    fn parse_worktrees_bare_entry_skipped() {
        let output = "\
worktree /Users/me/code/myrepo
HEAD abc123
branch refs/heads/main

worktree /Users/me/.spawn-agent/myrepo/dev
HEAD def456
bare
";
        let wts = parse_worktrees(output, "/.spawn-agent/myrepo/");
        assert!(wts.is_empty());
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
    fn browse_q_returns_close() {
        let mut s = state_with_worktrees();
        assert_eq!(s.handle_key_browse(&key(BareKey::Char('q'))), Action::Close);
    }

    #[test]
    fn browse_esc_returns_close() {
        let mut s = state_with_worktrees();
        assert_eq!(s.handle_key_browse(&key(BareKey::Esc)), Action::Close);
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
    fn input_branch_enter_noop_on_empty() {
        let mut s = State { mode: Mode::InputBranch, input_buffer: "  ".into(), ..Default::default() };
        let action = s.handle_key_input_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::None);
        assert_eq!(s.mode, Mode::InputBranch);
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
        // --git-common-dir returns the .git directory
        let action = s.handle_git_toplevel(Some(0), b"/home/user/myrepo/.git\n", b"");
        assert_eq!(s.repo_root, "/home/user/myrepo");
        assert_eq!(s.repo_name, "myrepo");
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(action, Action::FetchWorktreesAndBranches);
    }

    #[test]
    fn git_toplevel_from_worktree() {
        let mut s = State::default();
        // Even when launched from a worktree, --git-common-dir points to the main repo
        let action = s.handle_git_toplevel(Some(0), b"/home/user/myrepo/.git\n", b"");
        assert_eq!(s.repo_root, "/home/user/myrepo");
        assert_eq!(s.repo_name, "myrepo");
        assert_eq!(action, Action::FetchWorktreesAndBranches);
    }

    #[test]
    fn git_toplevel_error() {
        let mut s = State::default();
        let action = s.handle_git_toplevel(Some(128), b"", b"not a git repo");
        assert!(s.status_is_error);
        assert!(s.status_message.contains("not a git repo"));
        assert_eq!(s.mode, Mode::Loading);
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

    fn make_tab(name: &str, position: usize) -> TabInfo {
        TabInfo {
            position,
            name: name.to_string(),
            active: false,
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
    fn tab_index_for_branch_found() {
        let mut s = State::default();
        s.tabs = vec![
            make_tab("main-tab", 0),
            make_tab("feature-cool", 1),
            make_tab("fix-bug", 2),
        ];
        assert_eq!(s.tab_index_for_branch("feature/cool"), Some(1));
        assert_eq!(s.tab_index_for_branch("fix-bug"), Some(2));
    }

    #[test]
    fn tab_index_for_branch_not_found() {
        let mut s = State::default();
        s.tabs = vec![make_tab("main-tab", 0)];
        assert_eq!(s.tab_index_for_branch("nonexistent"), None);
    }

    #[test]
    fn list_worktrees_clamps_selected_index() {
        let mut s = State::default();
        s.repo_name = "myrepo".into();
        s.selected_index = 5;
        let output = b"worktree /home/me/.spawn-agent/myrepo/a\nHEAD abc\nbranch refs/heads/a\n";
        s.handle_list_worktrees(Some(0), output, "/.spawn-agent/myrepo/");
        assert_eq!(s.selected_index, 0);
    }
}
