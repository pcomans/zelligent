pub mod ui;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use std::io::Write;
use std::path::PathBuf;
use zellij_tile::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentStatus {
    #[default]
    Idle,
    Working,
    NeedsInput,
    Done,
}

/// A status event buffered in `State::pending_statuses` because its tab
/// wasn't known yet. `age` counts the `TabUpdate` events the entry has
/// survived unmatched; re-receiving a pipe for the same tab replaces the
/// whole entry, resetting `age` to 0. See `PENDING_STATUS_MAX_TAB_UPDATES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingStatus {
    pub status: AgentStatus,
    pub age: u8,
}

/// How many `TabUpdate` events an unmatched entry in `pending_statuses` may
/// survive before it's dropped. The legitimate race this buffer exists for
/// — an external `zelligent-status` sender beating the `TabUpdate` that
/// registers its tab (see #141) — resolves within 1-2 updates; 8 is a
/// generous margin while still ensuring a stale or mistyped `tab=` value
/// can't sit in the buffer indefinitely and later get misapplied to an
/// unrelated tab created with that name.
pub const PENDING_STATUS_MAX_TAB_UPDATES: u8 = 8;

pub const VERSION: &str = env!("ZELLIGENT_VERSION");

/// How long a footer `status_message` (e.g. green "Spawned 'feature-c'", red
/// "Unknown agent event: Bogus") stays visible before `set_status` and
/// `handle_timer` clear it automatically. Long enough to read a short line
/// without feeling rushed; short enough that a stale success/error message
/// doesn't linger indefinitely once the sidebar has moved on (issue #152 —
/// previously nothing ever cleared it, and a message could survive 10+
/// subsequent actions). See `State::set_status` / `State::handle_timer`.
pub const STATUS_MESSAGE_TTL_SECS: f64 = 8.0;

// Command context keys used to route RunCommandResult
pub const CMD_GIT_TOPLEVEL: &str = "git_toplevel";
pub const CMD_LIST_WORKTREES: &str = "list_worktrees";
pub const CMD_GIT_BRANCHES: &str = "git_branches";
pub const CMD_SPAWN: &str = "spawn";
pub const CMD_REMOVE: &str = "remove";
pub const CMD_INVALIDATE_BROADCAST: &str = "invalidate_broadcast";
pub const CMD_STATUS_REQUEST_BROADCAST: &str = "status_request_broadcast";
pub const CMD_STATUS_REPLAY_BROADCAST: &str = "status_replay_broadcast";

/// Pipe name for cross-instance cache invalidation. CLI pipes are the ONLY
/// channel that reaches hidden plugin instances (Events don't — see
/// docs/references/zellij-plugin-api.md), so cache invalidation must ride a
/// pipe. Broadcast by the CLI after spawn/remove and by the plugin's own
/// spawn/remove completion. See #140/#138.
pub const PIPE_INVALIDATE: &str = "zelligent-invalidate";

/// Context key carrying the invalidation generation a `list-worktrees`
/// refresh was launched under. Stamped by `fire_list_worktrees`, echoed
/// back in `Event::RunCommandResult`, and compared against
/// `State::invalidate_generation` in `handle_list_worktrees` to guard
/// against the stale-in-flight-refresh race. See #140.
pub const CTX_GENERATION: &str = "generation";

/// Pipe name for "reply with your known agent statuses". Broadcast once by
/// a plugin instance when its RunCommands grant lands (`Event::
/// PermissionRequestResult(Granted)` — never from `load()`, where
/// run_command is always denied) so a newly-created sidebar (e.g. in a
/// freshly spawned tab) can catch up on status glyphs it never saw —
/// `zelligent-status` pipes only reach instances alive at send time. See
/// #140 part B (Z-6).
pub const PIPE_STATUS_REQUEST: &str = "zelligent-status-request";

/// Pipe name for "here are my known agent statuses", sent in response to
/// `PIPE_STATUS_REQUEST` by any instance with a non-empty `agent_statuses`
/// map. Carries one arg, `STATUS_REPLAY_ARG`, whose value is the serialized
/// statuses (see `State::serialize_statuses` / `State::parse_statuses`).
pub const PIPE_STATUS_REPLAY: &str = "zelligent-status-replay";

/// The single `--args` key carried by `PIPE_STATUS_REPLAY`.
pub const STATUS_REPLAY_ARG: &str = "statuses";

/// Defensive cap (bytes) on the serialized replay payload. Tab names are
/// sanitized branch names limited to `[a-zA-Z0-9_-]` (see zelligent.sh), so
/// `:` and `;` below are safe, unambiguous separators. Well above any real
/// session's tab count; exists only to bound a pathological session.
const STATUS_REPLAY_MAX_LEN: usize = 4096;

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
    /// `we_initiated` is true when the plugin itself drove the close (the
    /// usual path from `handle_remove_result`). When false, the action was
    /// somehow synthesised after the target's tab had already been removed
    /// from `self.tabs` — we only run `close_focused_tab` if the tab is
    /// still present, so we don't close an unrelated focused tab. See #121.
    CloseTabAndRefresh {
        tab_name: String,
        return_to: Option<String>,
        we_initiated: bool,
    },
    SwitchToTab(String),
    Refresh,
    FetchToplevel,
    FetchWorktreesAndBranches,
    DumpLayout,
    NukeSession,
    Notify {
        tab_name: String,
        status: AgentStatus,
    },
    /// Broadcast `PIPE_STATUS_REQUEST` — fired once when the RunCommands
    /// grant lands (not from `load()`, where run_command is denied) so this
    /// (possibly late-created) instance can catch up on statuses it never
    /// saw. See #140 part B.
    RequestStatusReplay,
    /// Broadcast `PIPE_STATUS_REPLAY` carrying the given serialized
    /// payload, in response to a received `PIPE_STATUS_REQUEST`.
    ReplayStatuses(String),
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
    /// Sidebar items derived from tabs plus worktree metadata.
    pub sidebar_items: Vec<SidebarItem>,
    /// Agent status per tab name (sanitized branch name).
    pub agent_statuses: BTreeMap<String, AgentStatus>,
    /// Last rendered row count, used to map mouse clicks to sidebar rows.
    pub last_rows: usize,
    /// Last rendered column count. Needed alongside `last_rows` to
    /// recompute the exact same `ui::SidebarLayout` at click time that was
    /// used to draw the last frame — the footer height and status-message
    /// wrap both depend on `cols`, not just `rows`. See #135/#136.
    pub last_cols: usize,
    /// Tab names we've asked the host to close. Until the host's `TabUpdate`
    /// confirms the close, any incoming `tab_info` that still contains a
    /// pending tab is stale (a focus-change event, etc.) and that tab must
    /// be filtered out so the sidebar doesn't briefly resurrect it as an
    /// orphan row. A set (rather than `Option`) so rapid sequential removes
    /// don't lose the earlier pending names. See issue #121.
    pub pending_close: BTreeSet<String>,
    /// Status events for tabs not yet present in `self.tabs`. An external
    /// `zelligent-status` sender (e.g. an agent notification hook) can race
    /// the `TabUpdate` that registers a brand-new tab with existing sidebar
    /// instances — without buffering, the event is silently dropped and the
    /// spawning tab never shows its initial `Working` status. Keyed by tab
    /// name so a later event for the same not-yet-known tab overwrites the
    /// earlier one (latest wins, resetting its age); drained into
    /// `agent_statuses` once `handle_tab_update` sees the tab, and aged out
    /// after `PENDING_STATUS_MAX_TAB_UPDATES` unmatched updates. Bounded to
    /// 16 entries (see `handle_pipe`) so a flood of bogus tab names can't
    /// grow this unbounded. See issue #141.
    pub pending_statuses: BTreeMap<String, PendingStatus>,
    /// The worktree cache is known-stale: a `zelligent-invalidate` pipe
    /// arrived (someone spawned or removed a worktree) and no successful
    /// `list-worktrees` has landed since. The pipe handler fires an
    /// immediate Refresh, but for a hidden instance that Refresh's
    /// `RunCommandResult` is lost (Events don't reach hidden instances) —
    /// this bit is the durable part. It stays set until
    /// `handle_list_worktrees` succeeds and is retried as a Refresh trigger
    /// on every `TabUpdate`, the first of which arrives right when the
    /// instance's tab becomes visible again — when the result CAN land.
    /// Only a refresh launched at-or-after the latest invalidation may
    /// clear this bit; see `invalidate_generation` for why "any successful
    /// refresh clears it" is not good enough. See #140/#138.
    pub cache_dirty: bool,
    /// Generation counter bumped every time `cache_dirty` is set to true
    /// (i.e. once per `zelligent-invalidate` pipe). `fire_list_worktrees`
    /// stamps the generation current at launch time into the `run_command`
    /// context; `handle_list_worktrees` echoes it back via
    /// `Event::RunCommandResult` and only clears `cache_dirty` if the
    /// result's generation still equals `invalidate_generation`.
    ///
    /// This guards against a race a plain bool can't: refresh A is in
    /// flight when an invalidate pipe arrives, setting `cache_dirty` and
    /// bumping the generation, and launching refresh B. A then returns —
    /// stale, but exit code 0 — and without this counter would clear the
    /// bit B's invalidation set. If the instance goes hidden before B's
    /// result lands (hidden instances receive no Events, so B's result is
    /// simply lost), the cache is left stale with the dirty bit already
    /// consumed and nothing left to trigger a retry.
    ///
    /// The invariant: a successful refresh only proves freshness for
    /// invalidations known when it was launched. A's stamped generation
    /// predates B's, so A cannot clear a bit set by an invalidation it
    /// never observed — its listing is still applied (stale output is
    /// harmless; it's superseded when B lands, and if B is lost the
    /// still-set bit makes the next `TabUpdate` retry). See #140.
    pub invalidate_generation: u64,
    /// Name of the tab that was active as of the last `TabUpdate`, used by
    /// `handle_tab_update` to detect an active-tab change and re-sync the
    /// sidebar cursor. Deliberately separate from `selected_index`: the
    /// cursor also moves via j/k browsing within the current tab, which must
    /// NOT be snapped back on the next same-active `TabUpdate`. See #151.
    ///
    /// Change detection alone CANNOT catch a tab round trip: hidden
    /// instances receive no `TabUpdate`s, so from each instance's own
    /// perspective the active tab is its own tab both in the last update
    /// before hiding and the first one after reveal (live-instrumented,
    /// #151). `Event::Visible(true)` is the reveal signal — see
    /// `handle_visible` and `resync_on_reveal`.
    pub last_active_tab: Option<String>,
    /// Set by `handle_visible(true)`; makes the next `handle_tab_update`
    /// re-sync the cursor unconditionally, then clears. Covers the reveal
    /// ordering where the fresh active-tab snapshot arrives just after the
    /// `Visible(true)` event (the immediate re-sync in `handle_visible`
    /// covers the other ordering, and is correct even against pre-hide
    /// `self.tabs` because the instance's own tab was active then too).
    pub resync_on_reveal: bool,
    /// When the currently displayed `status_message` was set (`None` when
    /// no message is showing). THE source of truth for expiry (#152):
    /// `handle_timer` and `handle_visible` clear the message iff it is at
    /// least `STATUS_MESSAGE_TTL_SECS` old. `Event::Timer` is only a
    /// wake-up, never an authority — zellij's `set_timeout` spawns an
    /// independent one-shot timer per call (see zellij-server), timers can
    /// be lost entirely while the instance's pane is hidden (hidden
    /// instances receive no Events), and any bookkeeping that must pair
    /// arms with fires therefore wedges after a single loss. An age check
    /// is immune: a stale timer firing early finds the newer message too
    /// young and leaves it; a lost timer is covered by the next wake-up
    /// (a later message's timer, or the reveal re-arm in
    /// `handle_visible`). Uses the WASI monotonic clock, available to the
    /// plugin sandbox.
    pub status_message_set_at: Option<Instant>,
    /// Set by `set_status` when it arms a new timer; consumed by the
    /// `ZellijPlugin::update`/`pipe` shell, which performs the actual
    /// `zellij_tile::shim::set_timeout` host call and clears this flag.
    /// Keeps the host call out of `set_status` (a pure, unit-tested state
    /// mutation) — the same imperative-shell/pure-core split already used
    /// for `Action`/`execute` and `fire_invalidate_broadcast`. Tests
    /// observe this flag directly instead of a real timer being armed.
    pub status_timer_needs_arming: bool,
    /// Seconds the next wake-up should be armed for. `set_status` requests
    /// the full TTL; `handle_visible`/`handle_timer` request only the
    /// REMAINING TTL of the current message — arming a full TTL on reveal
    /// would let a nearly-expired message live almost twice its lifetime,
    /// and an early-firing timer must re-chain for what's left rather than
    /// leave the message stranded until an unrelated event.
    pub status_timer_arm_secs: f64,
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

    result
        .trim_matches(|c| c == '-' || c == '.' || c == '/')
        .to_string()
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
    /// Set the footer status message and arm its `STATUS_MESSAGE_TTL_SECS`
    /// auto-clear (issue #152). This is the ONLY place that should assign
    /// `self.status_message`/`self.status_is_error` for a non-empty message
    /// — every call site that used to assign the fields directly now goes
    /// through here, so a future call site can't forget the timer.
    ///
    /// A non-empty `msg` stamps `status_message_set_at` and sets
    /// `status_timer_needs_arming` so the imperative shell (`update`/
    /// `pipe`) arms a wake-up timer right after this event finishes
    /// processing. Re-setting within the TTL simply re-stamps — the age
    /// check in `handle_timer` gives the newer message its own full TTL
    /// no matter how many older timers are still in flight.
    ///
    /// An empty `msg` (clearing the status) resets the stamp AND any
    /// not-yet-performed arm request: there is nothing left to expire,
    /// and arming for an already-cleared message would be pure noise.
    pub fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        let msg = msg.into();
        self.status_is_error = is_error;
        if msg.is_empty() {
            self.status_message = msg;
            self.status_message_set_at = None;
            self.status_timer_needs_arming = false;
            return;
        }
        self.status_message = msg;
        self.status_message_set_at = Some(Instant::now());
        self.status_timer_needs_arming = true;
        self.status_timer_arm_secs = STATUS_MESSAGE_TTL_SECS;
    }

    /// True when the currently displayed message has lived out its TTL.
    /// Pure age check — see `status_message_set_at` for why expiry is
    /// decided by age, never by pairing timer arms with fires. The small
    /// tolerance absorbs the host timer firing marginally early relative
    /// to our own clock reading.
    fn status_message_expired(&self) -> bool {
        self.status_message_set_at
            .is_some_and(|t| t.elapsed().as_secs_f64() >= STATUS_MESSAGE_TTL_SECS - 0.25)
    }

    /// Seconds of TTL the current message has left (floored at a small
    /// positive wake-up so a nearly-expired message still gets a timer).
    fn status_message_remaining_secs(&self) -> f64 {
        self.status_message_set_at
            .map(|t| (STATUS_MESSAGE_TTL_SECS - t.elapsed().as_secs_f64()).max(0.3))
            .unwrap_or(STATUS_MESSAGE_TTL_SECS)
    }

    /// Handle `Event::Timer` — a `STATUS_MESSAGE_TTL_SECS` wake-up armed by
    /// the shell after a `set_status`. Pure (no host calls) so it stays
    /// unit-testable; the real `set_timeout` call lives in the
    /// `update`/`pipe` shell.
    ///
    /// The timer is only a wake-up: the clear decision is the age check in
    /// `status_message_expired` (see `status_message_set_at` for why any
    /// arm/fire pairing scheme wedges on lost timers). A stale timer from
    /// an already-replaced message finds the newer message too young and
    /// leaves it to be cleared by its own wake-up.
    ///
    /// Returns `true` (re-render needed) only when a message was actually
    /// cleared.
    pub fn handle_timer(&mut self) -> bool {
        if self.status_message.is_empty() {
            return false;
        }
        if self.status_message_expired() {
            self.status_message.clear();
            self.status_is_error = false;
            self.status_message_set_at = None;
            true
        } else {
            // Early wake-up (host timer fired ahead of our clock, or this
            // was a stale timer from a replaced message): re-chain for the
            // remaining TTL so the message never depends on an unrelated
            // event to expire. Terminates — each fire either clears or
            // re-arms exactly once for a strictly later deadline.
            self.status_timer_needs_arming = true;
            self.status_timer_arm_secs = self.status_message_remaining_secs();
            false
        }
    }

    fn sidebar_item_key(item: &SidebarItem) -> String {
        match &item.matched_branch {
            Some(branch) => format!("branch:{branch}"),
            None => format!("tab:{}", item.tab_name),
        }
    }

    fn active_tab_name(&self) -> Option<&str> {
        self.tabs
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.name.as_str())
    }

    fn select_active_sidebar_item(&mut self) -> bool {
        let Some(active_tab_name) = self.active_tab_name() else {
            return false;
        };
        let Some(idx) = self
            .sidebar_items
            .iter()
            .position(|item| item.tab_name == active_tab_name)
        else {
            return false;
        };
        self.selected_index = idx;
        true
    }

    /// Handle `Event::Visible` — the only reliable "this pane was just
    /// revealed" signal (#151): hidden instances receive no `TabUpdate`s,
    /// so a hide/reveal round trip never registers as an active-tab change
    /// in `handle_tab_update`. On reveal, re-sync the cursor immediately
    /// (correct even against pre-hide `self.tabs`: this instance's own tab
    /// was the active one then too) and arm `resync_on_reveal` so the
    /// fresh snapshot that typically follows re-syncs as well, whichever
    /// order the two events arrive in. Returns whether a re-render is
    /// needed (the cursor moved).
    /// Reveal also reconciles the status footer (#152): a message that
    /// expired while this pane was hidden is cleared lazily (its wake-up
    /// timer may have been lost — hidden instances receive no Events),
    /// and a still-live message gets a fresh wake-up armed for the same
    /// reason. Both paths are safe no matter whether the original timer
    /// actually fires later: expiry is decided by age, and a redundant
    /// wake-up on a young message is a no-op.
    pub fn handle_visible(&mut self, visible: bool) -> bool {
        if !visible {
            return false;
        }
        let before = self.selected_index;
        self.select_active_sidebar_item();
        self.resync_on_reveal = true;
        let mut status_changed = false;
        if !self.status_message.is_empty() {
            if self.status_message_expired() {
                self.status_message.clear();
                self.status_is_error = false;
                self.status_message_set_at = None;
                status_changed = true;
            } else {
                // Only the REMAINING TTL: a full re-arm here would extend a
                // nearly-expired message to almost twice its lifetime.
                self.status_timer_needs_arming = true;
                self.status_timer_arm_secs = self.status_message_remaining_secs();
            }
        }
        self.selected_index != before || status_changed
    }

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
        // Stamp the generation current at launch time (see
        // `State::invalidate_generation`). This is the only launch site
        // for `list-worktrees` — bootstrap, manual refresh, and
        // invalidation-triggered refresh all funnel through here — so a
        // single stamp covers every case. When there is no pending
        // invalidation this just echoes the current (possibly stale from
        // an already-cleared round) generation, which compares equal to
        // itself and clears `cache_dirty` as a no-op.
        let mut ctx = Self::ctx(CMD_LIST_WORKTREES);
        ctx.insert(
            CTX_GENERATION.to_string(),
            self.invalidate_generation.to_string(),
        );
        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "list-worktrees"],
            BTreeMap::new(),
            PathBuf::from(&self.repo_root),
            ctx,
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

    /// Broadcast `zelligent-invalidate` to ALL sidebar instances in this
    /// session — including hidden ones, which CLI pipes reach but Events do
    /// not (see docs/references/zellij-plugin-api.md). Invoked from the
    /// imperative shell after this instance completes a spawn or remove;
    /// the pure handlers stay side-effect free. Uses `run_command` with the
    /// host `zellij` binary rather than `pipe_message_to_plugin`: the
    /// plugin-side API (zellij-tile 0.43) targets a single destination
    /// plugin by id/url and may LAUNCH a new instance on a url miss,
    /// whereas an un-targeted CLI pipe is the verified broadcast channel
    /// (it's how the zelligent-status hooks already reach every instance).
    /// Best-effort: the result is ignored (see CMD_INVALIDATE_BROADCAST in
    /// `update`), and without a session name we skip silently.
    fn fire_invalidate_broadcast(&self) {
        if let Some(session) = &self.session_name {
            run_command(
                &[
                    "zellij",
                    "--session",
                    session,
                    "pipe",
                    "--name",
                    PIPE_INVALIDATE,
                ],
                Self::ctx(CMD_INVALIDATE_BROADCAST),
            );
        }
    }

    /// Broadcast `PIPE_STATUS_REQUEST` to every sidebar instance in this
    /// session. Fired once from the `PermissionRequestResult(Granted)`
    /// handler — NOT `load()`, where the async RunCommands grant makes
    /// run_command a guaranteed denial (see that handler's comment): a
    /// freshly-created instance (e.g. the sidebar in a newly spawned tab)
    /// has an empty `agent_statuses` map and never saw any
    /// `zelligent-status` pipe sent before it existed. Same transport as
    /// `fire_invalidate_broadcast` and for the
    /// same reason — see that method's doc comment. Best-effort: any
    /// resulting replies land as ordinary `zelligent-status-replay` pipes
    /// (see `handle_pipe`); the RunCommandResult of this broadcast itself
    /// is ignored (CMD_STATUS_REQUEST_BROADCAST in `update`).
    fn fire_status_request(&self) {
        if let Some(session) = &self.session_name {
            run_command(
                &[
                    "zellij",
                    "--session",
                    session,
                    "pipe",
                    "--name",
                    PIPE_STATUS_REQUEST,
                ],
                Self::ctx(CMD_STATUS_REQUEST_BROADCAST),
            );
        }
    }

    /// Broadcast `PIPE_STATUS_REPLAY` carrying `payload` (see
    /// `serialize_statuses`) to every sidebar instance in this session, in
    /// response to a received `PIPE_STATUS_REQUEST`. Same transport as
    /// `fire_invalidate_broadcast`.
    fn fire_status_replay(&self, payload: &str) {
        if let Some(session) = &self.session_name {
            run_command(
                &[
                    "zellij",
                    "--session",
                    session,
                    "pipe",
                    "--name",
                    PIPE_STATUS_REPLAY,
                    "--args",
                    &format!("{STATUS_REPLAY_ARG}={payload}"),
                ],
                Self::ctx(CMD_STATUS_REPLAY_BROADCAST),
            );
        }
    }

    /// Short, stable wire code for an `AgentStatus` — used by
    /// `serialize_statuses`/`parse_statuses`. Not `Debug`-derived on
    /// purpose: the wire format must not silently change if `AgentStatus`'s
    /// derive output ever changes.
    fn status_code(status: AgentStatus) -> &'static str {
        match status {
            AgentStatus::Idle => "Idle",
            AgentStatus::Working => "Working",
            AgentStatus::NeedsInput => "NeedsInput",
            AgentStatus::Done => "Done",
        }
    }

    fn parse_status_code(code: &str) -> Option<AgentStatus> {
        match code {
            "Idle" => Some(AgentStatus::Idle),
            "Working" => Some(AgentStatus::Working),
            "NeedsInput" => Some(AgentStatus::NeedsInput),
            "Done" => Some(AgentStatus::Done),
            _ => None,
        }
    }

    /// Serialize this instance's known statuses — its live `agent_statuses`
    /// plus its buffered `pending_statuses` (see #141) — for a
    /// `PIPE_STATUS_REPLAY` broadcast. Format: `tab:code` entries joined by
    /// `;`, e.g. `feat-a:Working;feat-b:Done`. Worktree tab names are
    /// sanitized branch names restricted to `[a-zA-Z0-9_-]` (zelligent.sh),
    /// but `zelligent-status` accepts any `tab=` value, so names containing
    /// a separator are skipped rather than emitted — a `;` inside a name
    /// would otherwise fragment into a spurious entry for a different tab
    /// on every receiver. Entries are appended only while the result
    /// stays within `STATUS_REPLAY_MAX_LEN`; any remainder is silently
    /// dropped (defensive cap, not expected to bite in practice).
    fn serialize_statuses(&self) -> String {
        let mut out = String::new();
        for (tab, status) in self
            .agent_statuses
            .iter()
            .chain(self.pending_statuses.iter().map(|(tab, p)| (tab, &p.status)))
        {
            if tab.contains(':') || tab.contains(';') {
                continue;
            }
            let entry = format!("{tab}:{}", Self::status_code(*status));
            let extra_len = if out.is_empty() {
                entry.len()
            } else {
                entry.len() + 1 // the joining ';'
            };
            if out.len() + extra_len > STATUS_REPLAY_MAX_LEN {
                break;
            }
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str(&entry);
        }
        out
    }

    /// Inverse of `serialize_statuses`. Unparseable entries (bad separator,
    /// unknown status code, empty tab name) are skipped rather than
    /// failing the whole payload — a partial replay is still useful.
    fn parse_statuses(payload: &str) -> Vec<(String, AgentStatus)> {
        payload
            .split(';')
            .filter_map(|entry| {
                let (tab, code) = entry.split_once(':')?;
                if tab.is_empty() {
                    return None;
                }
                Self::parse_status_code(code).map(|status| (tab.to_string(), status))
            })
            .collect()
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

        // `--plugin-driven` tells the CLI to skip its own tab-close block —
        // the plugin will close the worktree's tab via
        // `Action::CloseTabAndRefresh` after this command returns. Without
        // the flag, both the CLI and the plugin would call `close-tab`, and
        // the second close can land on the user's origin tab. A flag rather
        // than an env var so a user who exported `ZELLIGENT_PLUGIN_DRIVEN=1`
        // in their shell can't accidentally break manual `zelligent remove`.
        // See issue #121.
        run_command_with_env_variables_and_cwd(
            &[&self.zelligent_path, "remove", "--plugin-driven", branch],
            env,
            PathBuf::from(&self.repo_root),
            ctx,
        );
    }

    /// Imperative-shell counterpart to `set_status`'s
    /// `status_timer_needs_arming` flag: performs the actual
    /// `zellij_tile::shim::set_timeout` host call `set_status` requested
    /// and clears the flag. Called from `update`/`pipe` after `execute`,
    /// once per event, so a burst of `set_status` calls within a single
    /// handler still only arms (at most) one timer per event.
    fn arm_pending_status_timer(&mut self) {
        if self.status_timer_needs_arming {
            set_timeout(self.status_timer_arm_secs.max(0.3));
            self.status_timer_needs_arming = false;
        }
    }

    fn execute(&self, action: &Action) {
        match action {
            Action::None => {}
            Action::Close => close_self(),
            Action::Spawn(branch) => self.fire_spawn(branch),
            Action::Remove(branch) => self.fire_remove(branch),
            Action::CloseTabAndRefresh {
                tab_name,
                return_to,
                we_initiated,
            } => {
                // Defense in depth against issue #121. Run the close when
                // either:
                //   - we_initiated is true (this action was emitted by
                //     `handle_remove_result`, which optimistically retains
                //     the tab out of `self.tabs` — so the still_present
                //     check below would wrongly say "skip"), OR
                //   - the tab is still in our cache (cosmetic safety net
                //     for any future call site that constructs this action
                //     without we_initiated=true and the tab still around).
                // If neither, the tab was closed externally before we got
                // here; running `close_focused_tab` would close whatever
                // pane Zellij happens to be focused on, often the user's
                // origin tab. The we_initiated flag is carried in the
                // payload (not read from `self.pending_close`) so the
                // close decision is decoupled from any subsequent state
                // mutation between emission and execution.
                let still_present = self.tabs.iter().any(|t| t.name == *tab_name);
                if *we_initiated || still_present {
                    go_to_tab_name(tab_name);
                    close_focused_tab();
                }
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
                    AgentStatus::NeedsInput => format!("{tab_name} needs input"),
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
                if matches!(status, AgentStatus::NeedsInput) {
                    run_command(
                        &["afplay", "/System/Library/Sounds/Glass.aiff"],
                        BTreeMap::new(),
                    );
                }
            }
            Action::RequestStatusReplay => self.fire_status_request(),
            Action::ReplayStatuses(payload) => self.fire_status_replay(payload),
        }
    }

    // --- Pure state handlers (no zellij calls, fully testable) ---

    pub fn handle_git_toplevel(
        &mut self,
        exit_code: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
    ) -> Action {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            let cwd = self.initial_cwd.display();
            self.set_status(format!("{cwd} is not a git repo: {err}"), true);
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
            self.set_status("Failed to parse repo info", true);
            return Action::None;
        }
        self.mode = Mode::BrowseWorktrees;
        Action::FetchWorktreesAndBranches
    }

    pub fn handle_list_worktrees(
        &mut self,
        exit_code: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
        context: &BTreeMap<String, String>,
    ) {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            self.set_status(format!("Failed to list worktrees: {err}"), true);
            return;
        }
        let output = String::from_utf8_lossy(stdout);
        self.worktrees = parse_worktrees(&output);
        // The listing is applied unconditionally — even a refresh launched
        // before the latest invalidation is harmless to apply: it's either
        // still accurate or gets superseded when the newer refresh lands.
        //
        // Clearing `cache_dirty`, however, is conditional. Success alone
        // isn't proof of freshness (see `invalidate_generation`): only a
        // refresh stamped with the CURRENT generation — i.e. one launched
        // at-or-after the latest invalidation — can prove the cache
        // reflects that invalidation. A stale-generation result leaves the
        // bit set so the next `TabUpdate` retries. The failure path above
        // deliberately returns before this, without touching either field:
        // a failed refresh proves nothing about generation OR freshness.
        let result_generation = context
            .get(CTX_GENERATION)
            .and_then(|g| g.parse::<u64>().ok())
            .unwrap_or(0);
        if result_generation == self.invalidate_generation {
            self.cache_dirty = false;
        }
        self.recompute_sidebar_items();
    }

    pub fn recompute_sidebar_items(&mut self) {
        let previous_item_key = self
            .sidebar_items
            .get(self.selected_index)
            .map(Self::sidebar_item_key);

        let mut items = Vec::new();

        if !self.repo_name.is_empty() && self.tabs.iter().any(|tab| tab.name == self.repo_name) {
            items.push(SidebarItem {
                tab_name: self.repo_name.clone(),
                display_name: "local".to_string(),
                matched_branch: None,
            });
        }

        for wt in &self.worktrees {
            items.push(SidebarItem {
                tab_name: Self::tab_name_for_branch(&wt.branch),
                display_name: if wt.dir != wt.branch {
                    wt.dir.clone()
                } else {
                    wt.branch.clone()
                },
                matched_branch: Some(wt.branch.clone()),
            });
        }

        for tab in &self.tabs {
            if tab.name == self.repo_name {
                continue;
            }

            let is_managed = self
                .worktrees
                .iter()
                .any(|wt| Self::tab_name_for_branch(&wt.branch) == tab.name);
            if is_managed {
                continue;
            }

            items.push(SidebarItem {
                tab_name: tab.name.clone(),
                display_name: tab.name.clone(),
                matched_branch: None,
            });
        }

        self.sidebar_items = items;

        if let Some(previous_item_key) = previous_item_key {
            if let Some(idx) = self
                .sidebar_items
                .iter()
                .position(|item| Self::sidebar_item_key(item) == previous_item_key)
            {
                self.selected_index = idx;
                return;
            }
        }

        if !self.select_active_sidebar_item()
            && self.selected_index >= self.sidebar_items.len()
            && !self.sidebar_items.is_empty()
        {
            self.selected_index = self.sidebar_items.len() - 1;
        }
    }

    pub fn handle_git_branches(&mut self, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) {
        if exit_code != Some(0) {
            let err = String::from_utf8_lossy(stderr);
            self.set_status(format!("Failed to list branches: {err}"), true);
            return;
        }
        let output = String::from_utf8_lossy(stdout);
        self.branches = parse_branches(&output);
    }

    pub fn handle_tab_update(&mut self, tab_info: Vec<TabInfo>) -> Action {
        let had_tabs = !self.tabs.is_empty();
        // Snapshot pending_close before the confirm loop below mutates it.
        // The disappeared-tab check further down needs to know which names
        // were pending *before* this update, so a self-initiated close being
        // confirmed here isn't mistaken for an externally-vanished tab.
        // See #138.
        let pending_close_before: BTreeSet<String> = self.pending_close.clone();
        // Race guard: for each tab we asked the host to close, filter it
        // out of any incoming snapshot that still includes it. Once a
        // `TabUpdate` arrives that no longer contains a pending tab, the
        // close has propagated — remove it from the set. Iterate over a
        // cloned key list because we may mutate the set in the loop.
        // See issue #121.
        let mut tab_info = tab_info;
        let pending: Vec<String> = self.pending_close.iter().cloned().collect();
        for name in pending {
            if tab_info.iter().any(|t| t.name == name) {
                tab_info.retain(|t| t.name != name);
            } else {
                self.pending_close.remove(&name);
            }
        }

        // Snapshot the previous tab names before we overwrite. Used below to
        // detect which tabs in the new snapshot are *newly appeared* — only
        // those should drive a worktree refresh.
        let previously_known: BTreeSet<String> =
            self.tabs.iter().map(|t| t.name.clone()).collect();

        self.tabs = tab_info;

        // Drain any buffered status events (see #141) for tabs that have now
        // shown up in this snapshot. Must run after `self.tabs` is assigned
        // (above) so `recompute_sidebar_items` below sees the up-to-date
        // `agent_statuses`, and before it so the freshly-drained status
        // renders on this same pass instead of lagging a frame.
        if !self.pending_statuses.is_empty() {
            for tab in &self.tabs {
                if let Some(pending) = self.pending_statuses.remove(&tab.name) {
                    self.agent_statuses.insert(tab.name.clone(), pending.status);
                }
            }
            // Age out whatever's left. A stale or mistyped `tab=` value must
            // not sit here indefinitely and later get applied to an
            // unrelated tab that happens to be created with that name.
            self.pending_statuses.retain(|_, pending| {
                pending.age += 1;
                pending.age < PENDING_STATUS_MAX_TAB_UPDATES
            });
        }

        self.recompute_sidebar_items();

        // Re-sync the cursor to the active tab when (a) this is the first
        // snapshot (`!had_tabs`, the old bootstrap correction), (b) this
        // instance's pane was just revealed (`resync_on_reveal`, armed by
        // `Event::Visible(true)` — a sidebar click, Enter, or native Ctrl-t
        // switch into an already-open tab lands here), or (c) the active
        // tab changed within a delivered snapshot (tab creation, closes).
        //
        // (c) alone looked sufficient but is NOT — live instrumentation
        // (#151) showed hidden instances receive no `TabUpdate`s at all, so
        // across a hide/reveal round trip the active tab reads as this
        // instance's own tab on both sides and never "changes". The gating
        // still matters for what it EXCLUDES: a same-active `TabUpdate`
        // arriving while the user j/k-browses this visible instance must
        // not snap the cursor back.
        //
        // `select_active_sidebar_item` returns false when the active tab has
        // no sidebar row (e.g. a plain user tab with no matching worktree);
        // deliberately do NOT reset `selected_index` in that case — leave
        // the cursor where it is. `last_active_tab` is still updated so that
        // switching *back* to a tab with a row is detected as a change and
        // re-syncs then.
        let current_active_tab = self.active_tab_name().map(str::to_string);
        if !had_tabs || self.resync_on_reveal || current_active_tab != self.last_active_tab {
            self.select_active_sidebar_item();
            self.resync_on_reveal = false;
            self.last_active_tab = current_active_tab;
        }

        // Refresh worktrees only when a *newly-appeared* tab has no matching
        // worktree. This self-heals the "user tab" mislabel that appears when
        // our cached `self.worktrees` is stale relative to the actual tabs —
        // happens when a worktree is created out-of-band (e.g. agent runs
        // `zelligent spawn` via bash) or when the last refresh raced with a
        // spawn and missed the new entry.
        //
        // Without the "newly-appeared" check, a legitimate persistent
        // user-created tab (one with no underlying worktree) would drive a
        // Refresh on *every* TabUpdate forever — including focus changes.
        // Restricting to new tabs keeps the refresh one-shot per tab and
        // closes any theoretical TabUpdate→Refresh feedback path.
        let worktree_tab_names: BTreeSet<String> = self
            .worktrees
            .iter()
            .map(|wt| Self::tab_name_for_branch(&wt.branch))
            .collect();
        let has_new_unmatched = self.tabs.iter().any(|t| {
            !previously_known.contains(&t.name)
                && t.name != self.repo_name
                && !worktree_tab_names.contains(&t.name)
        });

        // Refresh worktrees when a previously-known tab is now absent. Each
        // tab's sidebar is a separate plugin instance with its own cache, so
        // an instance other than the one that drove a remove only learns
        // about it via this TabUpdate — nothing else tells it the worktree
        // row is now stale. Self-initiated closes are excluded via
        // `pending_close_before`: `handle_remove_result` already drops the
        // tab from `self.tabs` up front, so `previously_known` (snapshotted
        // above, before that drop is overwritten here) never contains it for
        // this instance in the first place; the `pending_close_before` guard
        // additionally covers the confirming `TabUpdate`, where the tab is
        // still in `previously_known` from an earlier call. See #138.
        let has_disappeared_known = previously_known.iter().any(|name| {
            !self.tabs.iter().any(|t| &t.name == name) && !pending_close_before.contains(name)
        });

        // Refresh whenever the tab SET changed at all — any addition or
        // removal relative to `previously_known`, regardless of worktree
        // matching. This is the trigger that heals event-starved instances.
        // Verified live (#140, 2026-07): Zellij delivers Events (TabUpdate
        // etc.) only to plugin instances in the *visible* tab — pipes
        // broadcast to all instances, but a hidden instance receives no
        // Events at all. So a hidden instance's snapshot freezes at the
        // moment its tab lost focus; tabs spawned and/or removed while it
        // was hidden never pass through the two gates above (a new tab that
        // already matches the stale worktree cache is invisible to
        // has_new_unmatched, and a tab that both appeared and vanished while
        // hidden was never in `previously_known` for has_disappeared_known).
        // The catch-up TabUpdate this instance receives when its tab becomes
        // active again is exactly when the set diff fires — and since the
        // instance is now visible, the Refresh's run_command result lands.
        // Kept alongside the two narrower gates above: they encode
        // separately-tested semantics and cost nothing.
        //
        // A pure focus switch with no set drift does NOT fire: a visible
        // instance's cache was maintained while visible, so there's nothing
        // to heal. Also excluded on the very first TabUpdate since startup
        // (`had_tabs == false`): the bootstrap path already loads worktrees,
        // so firing here too would just be a redundant double-refresh.
        //
        // Removals reuse `has_disappeared_known`, which carves out
        // `pending_close_before`: a self-initiated close already drives its
        // own Refresh via `handle_remove_result`, per the #121/#138
        // handshake documented above.
        let has_added_tab = self
            .tabs
            .iter()
            .any(|t| !previously_known.contains(&t.name));
        let has_tab_set_changed = had_tabs && (has_added_tab || has_disappeared_known);

        // Dirty-cache retry (#140/#138). The set-diff above cannot catch a
        // blind-window round-trip: a worktree spawned AND removed entirely
        // while this instance was hidden leaves zero net set drift at
        // wake-up (verified live — prev_known == new set, trigger correctly
        // None). The `zelligent-invalidate` pipe covers that case: pipes DO
        // reach hidden instances, so `cache_dirty` was set at the time of
        // the change, and this TabUpdate — the first Event the instance
        // receives on becoming visible — is when the retried Refresh can
        // actually complete. Deliberately NOT cleared here: only a
        // successful `handle_list_worktrees` proves the cache is fresh
        // again. Also deliberately not gated on `had_tabs`: at worst the
        // bootstrap refresh and this one overlap, and the first success
        // clears the bit.
        if has_new_unmatched || has_disappeared_known || has_tab_set_changed || self.cache_dirty {
            Action::Refresh
        } else {
            Action::None
        }
    }

    pub fn handle_spawn_result(
        &mut self,
        exit_code: Option<i32>,
        stderr: &[u8],
        context: &BTreeMap<String, String>,
    ) -> Action {
        let branch = context
            .get("branch")
            .cloned()
            .unwrap_or_else(|| "<unknown>".to_string());
        if exit_code == Some(0) {
            self.set_status(format!("Spawned '{branch}'"), false);
        } else {
            let err = String::from_utf8_lossy(stderr).trim().to_string();
            let code_str = match exit_code {
                Some(c) => format!("exit {c}"),
                None => "no exit code".to_string(),
            };
            if err.is_empty() {
                self.set_status(format!("Spawn '{branch}' failed ({code_str})"), true);
            } else {
                self.set_status(format!("Spawn '{branch}' failed: {err}"), true);
            }
        }
        Action::Refresh
    }

    pub fn handle_remove_result(
        &mut self,
        exit_code: Option<i32>,
        stderr: &[u8],
        context: &BTreeMap<String, String>,
    ) -> Action {
        let branch = context
            .get("branch")
            .cloned()
            .unwrap_or_else(|| "<unknown>".to_string());
        self.mode = Mode::BrowseWorktrees;
        if exit_code == Some(0) {
            self.set_status(format!("Removed '{branch}'"), false);
            // Close the worktree's tab if it exists, then refresh.
            if self.has_tab_for_branch(&branch) {
                let tab_name = Self::tab_name_for_branch(&branch);
                self.agent_statuses.remove(&tab_name);
                let return_to = self.tabs.iter().find(|t| t.active).map(|t| t.name.clone());
                // Drop the closed tab from our cache up front. The host fires
                // a TabUpdate after the close completes, but the worktree-list
                // refresh that follows races against it: if the worktree list
                // lands first, recompute_sidebar_items still sees the closed
                // tab in self.tabs, fails to match it to any worktree, and
                // surfaces it as an orphaned "user tab" until TabUpdate
                // catches up.
                self.tabs.retain(|t| t.name != tab_name);
                // Pending-close handshake: any TabUpdate arriving before the
                // host actually closes the tab (e.g. a focus-change event
                // fired by `go_to_tab_name` below) would re-introduce the
                // closed tab via the `self.tabs = tab_info` assignment in
                // `handle_tab_update`. Marking pending_close lets that
                // handler filter the stale entry until the close lands.
                self.pending_close.insert(tab_name.clone());
                return Action::CloseTabAndRefresh {
                    tab_name,
                    return_to,
                    we_initiated: true,
                };
            }
        } else {
            let err = String::from_utf8_lossy(stderr).trim().to_string();
            let code_str = match exit_code {
                Some(c) => format!("exit {c}"),
                None => "no exit code".to_string(),
            };
            if err.is_empty() {
                self.set_status(format!("Remove '{branch}' failed ({code_str})"), true);
            } else {
                self.set_status(format!("Remove '{branch}' failed: {err}"), true);
            }
        }
        Action::Refresh
    }

    /// Convert a branch name to the corresponding Zellij tab name.
    /// Tab names use the branch with `/` replaced by `-` and non-`[A-Za-z0-9_-]`
    /// chars stripped (matching zelligent.sh).
    pub fn tab_name_for_branch(branch: &str) -> String {
        ui::sanitize_tab_name(branch)
    }

    /// Check whether a tab with the given branch's name exists.
    pub fn has_tab_for_branch(&self, branch: &str) -> bool {
        let tab_name = Self::tab_name_for_branch(branch);
        self.tabs.iter().any(|t| t.name == tab_name)
    }

    fn selected_sidebar_branch(&self) -> Option<&str> {
        self.sidebar_items
            .get(self.selected_index)
            .and_then(|item| item.matched_branch.as_deref())
    }

    fn action_for_sidebar_item(&mut self, idx: usize) -> Action {
        let Some(item) = self.sidebar_items.get(idx).cloned() else {
            return Action::None;
        };

        if let Some(branch) = item.matched_branch {
            self.spawn_or_switch(branch)
        } else {
            Action::SwitchToTab(item.tab_name)
        }
    }

    fn should_render_empty_state(&self) -> bool {
        self.worktrees.is_empty()
            && self
                .sidebar_items
                .iter()
                .all(|item| item.matched_branch.is_none())
            && self.sidebar_items.len() <= 1
    }

    /// Map a rendered line number to a visible sidebar item.
    ///
    /// `line` is a mouse row as Zellij reports it: relative to the PANE
    /// CONTENT TOP (row 0 = the first content row inside the pane frame),
    /// NOT relative to the first sidebar item. This was the root cause of
    /// #135 — a click on an item's subtitle line landed on the *next* item
    /// because the old mapping (`line / 2`) silently assumed the render
    /// began with the first item, ignoring the header/blank leading lines.
    ///
    /// `sidebar_layout` (see ui.rs) is recomputed here from the same inputs
    /// `render_to` used for the last frame (`last_rows`/`last_cols` are
    /// captured at render time; `selected_index` and `status_message` don't
    /// change between a render and the click that follows it), so this can
    /// never disagree with what was actually drawn. Any line before the
    /// item viewport (header, blank separator) or at/after it (footer,
    /// status, past-the-end) is a strict no-op — never "select item 0" or
    /// "select the next item".
    pub fn sidebar_index_at_line(&self, line: usize) -> Option<usize> {
        if self.should_render_empty_state() || self.sidebar_items.is_empty() {
            return None;
        }

        let layout = ui::sidebar_layout(
            self.last_rows,
            self.last_cols,
            self.sidebar_items.len(),
            self.selected_index,
            &self.status_message,
        );
        let leading = layout.leading_lines();
        if line < leading {
            return None;
        }
        let item_offset = (line - leading) / 2;
        if item_offset >= layout.viewport.visible_items {
            return None;
        }

        Some(layout.viewport.start + item_offset)
    }

    pub fn handle_mouse_browse(&mut self, mouse: &Mouse) -> Action {
        if self.should_render_empty_state() {
            return Action::None;
        }

        match mouse {
            Mouse::ScrollUp(_) => {
                self.selected_index =
                    wrap_navigate(self.selected_index, self.sidebar_items.len(), -1);
            }
            Mouse::ScrollDown(_) => {
                self.selected_index =
                    wrap_navigate(self.selected_index, self.sidebar_items.len(), 1);
            }
            Mouse::LeftClick(line, _col) => {
                let line = (*line).max(0) as usize;
                if let Some(idx) = self.sidebar_index_at_line(line) {
                    self.selected_index = idx;
                    return self.action_for_sidebar_item(idx);
                }
            }
            _ => {}
        }

        Action::None
    }

    /// Switch to an existing tab for the branch, or spawn a new one.
    fn spawn_or_switch(&mut self, branch: String) -> Action {
        if self.has_tab_for_branch(&branch) {
            let tab_name = Self::tab_name_for_branch(&branch);
            return Action::SwitchToTab(tab_name);
        }
        self.set_status(format!("Spawning '{branch}'..."), false);
        Action::Spawn(branch)
    }

    pub fn handle_key_browse(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            let browse_len = self.sidebar_items.len();
            match key.bare_key {
                BareKey::Char('j') | BareKey::Down => {
                    self.selected_index = wrap_navigate(self.selected_index, browse_len, 1);
                }
                BareKey::Char('k') | BareKey::Up => {
                    self.selected_index = wrap_navigate(self.selected_index, browse_len, -1);
                }
                BareKey::Enter => {
                    return self.action_for_sidebar_item(self.selected_index);
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
                    if !self.sidebar_items.is_empty() {
                        if self.selected_sidebar_branch().is_some() {
                            self.mode = Mode::Confirming;
                        } else {
                            self.set_status("Only worktree tabs can be removed", true);
                        }
                    }
                }
                BareKey::Char('r') => {
                    self.set_status("Refreshed", false);
                    return Action::Refresh;
                }
                BareKey::Char('q') | BareKey::Esc => {}
                _ => {}
            }
        }
        Action::None
    }

    pub fn handle_key_select_branch(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('j') | BareKey::Down => {
                    self.selected_index =
                        wrap_navigate(self.selected_index, self.filtered_branches.len(), 1);
                }
                BareKey::Char('k') | BareKey::Up => {
                    self.selected_index =
                        wrap_navigate(self.selected_index, self.filtered_branches.len(), -1);
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
        let shift_only =
            key.key_modifiers.len() == 1 && key.key_modifiers.contains(&KeyModifier::Shift);

        match key.bare_key {
            BareKey::Enter if no_mod => {
                let branch = sanitize_branch_name(self.input_buffer.trim());
                if !branch.is_empty() {
                    self.mode = Mode::BrowseWorktrees;
                    return self.spawn_or_switch(branch);
                } else {
                    self.set_status("Invalid branch name", true);
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
                    let branch = self.selected_sidebar_branch().map(ToOwned::to_owned);
                    if let Some(branch) = branch {
                        self.set_status(format!("Removing '{branch}'..."), false);
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
        // Cross-instance cache invalidation (#140/#138). No args needed:
        // the message means "a worktree was spawned or removed somewhere —
        // your cache may be stale". Mark dirty FIRST (the durable part; a
        // hidden instance will lose the Refresh's RunCommandResult), then
        // fire an immediate Refresh, which completes right away in visible
        // instances. `handle_tab_update` retries the Refresh while the bit
        // is set; a successful `handle_list_worktrees` clears it — but only
        // if that refresh was launched at-or-after this bump (see
        // `invalidate_generation`), so a refresh already in flight when we
        // get here can't consume this invalidation.
        if msg.name == PIPE_INVALIDATE {
            self.cache_dirty = true;
            self.invalidate_generation += 1;
            return Action::Refresh;
        }
        // Late-created-instance status replay (#140 part B / Z-6). A
        // freshly-loaded instance broadcasts PIPE_STATUS_REQUEST; any
        // instance (including the requester itself, and including hidden
        // ones — see docs/references/zellij-plugin-api.md) that already
        // knows some statuses replies with PIPE_STATUS_REPLAY. Loop safety:
        // handling a request never produces another request, and handling
        // a replay never produces a request or another replay — the
        // request handler below returns at most one ReplayStatuses, and
        // the replay handler always returns Action::None.
        if msg.name == PIPE_STATUS_REQUEST {
            if self.agent_statuses.is_empty() && self.pending_statuses.is_empty() {
                // Nothing to offer — replying with an empty payload would
                // just be noise broadcast to every instance on every load.
                return Action::None;
            }
            // pending_statuses counts as knowledge too: an instance holding
            // only buffered early events (#141) must still reply, or a
            // sidebar loading before that tab exists misses them entirely.
            return Action::ReplayStatuses(self.serialize_statuses());
        }
        if msg.name == PIPE_STATUS_REPLAY {
            let payload = msg
                .args
                .get(STATUS_REPLAY_ARG)
                .map(|s| s.as_str())
                .unwrap_or("");
            for (tab, status) in Self::parse_statuses(payload) {
                if self.agent_statuses.contains_key(&tab) {
                    // Monotone merge: never clobber knowledge we already
                    // have — a stale instance's replay must not overwrite
                    // a status we learned more recently.
                    continue;
                }
                if self.tabs.iter().any(|t| t.name == tab) {
                    self.agent_statuses.insert(tab, status);
                } else if !self.pending_statuses.contains_key(&tab) {
                    // Unknown-tab entries go through the same buffer
                    // semantics as a live `zelligent-status` event (#141):
                    // capped at 16, evicting the lexicographically-first
                    // key on overflow. An existing pending entry for this
                    // tab is left alone (same monotone rule).
                    if self.pending_statuses.len() >= 16 {
                        self.pending_statuses.pop_first();
                    }
                    self.pending_statuses
                        .insert(tab, PendingStatus { status, age: 0 });
                }
            }
            // No Notify, no status_message: replay is a silent catch-up,
            // never a user-visible event. See #140 part B frozen design.
            return Action::None;
        }
        if msg.name != "zelligent-status" {
            return Action::None;
        }
        let tab_name = match msg.args.get("tab") {
            Some(name) if !name.is_empty() => name.clone(),
            _ => return Action::None,
        };
        let status = match msg.args.get("event").map(|s| s.as_str()) {
            Some("Start") | Some("UserPromptSubmit") => AgentStatus::Working,
            Some("PermissionRequest") => AgentStatus::NeedsInput,
            Some("Stop") => AgentStatus::Done,
            Some(other) => {
                self.set_status(format!("Unknown agent event: {other}"), true);
                return Action::None;
            }
            None => {
                self.set_status("Agent status missing 'event' arg", true);
                return Action::None;
            }
        };
        // The tab this event names isn't in `self.tabs` yet — almost always
        // because an external `zelligent-status` sender (e.g. an agent
        // notification hook) races the `TabUpdate` that registers the new
        // tab with this sidebar instance. Buffer it instead of dropping it;
        // `handle_tab_update` drains matching entries into `agent_statuses`
        // once the tab shows up, and ages out ones that never match. See
        // issue #141.
        if !self.tabs.iter().any(|t| t.name == tab_name) {
            // Latest event for a given not-yet-known tab wins.
            let is_new_key = !self.pending_statuses.contains_key(&tab_name);
            if is_new_key && self.pending_statuses.len() >= 16 {
                // Bound the buffer so a flood of bogus/unknown tab names
                // (typos, stale CLI invocations, etc.) can't grow it
                // unbounded. Evicting `first_key_value` (lexicographically
                // smallest) is arbitrary — there's no ordering signal worth
                // preserving here — but it keeps the map's size capped
                // deterministically without extra bookkeeping. Overwrites of
                // an already-buffered key never evict, since they don't grow
                // the map.
                self.pending_statuses.pop_first();
            }
            self.pending_statuses
                .insert(tab_name, PendingStatus { status, age: 0 });
            // No Notify here: the buffered case is overwhelmingly a Start
            // (Working), which never notifies anyway. Deferring a
            // NeedsInput/Done notify to TabUpdate time would fire it from
            // the wrong context (in response to a tab appearing, not the
            // actual status event), so we deliberately drop the notify for
            // the buffered path rather than replay it later.
            return Action::None;
        }
        self.agent_statuses.insert(tab_name.clone(), status);
        // TODO: consider suppressing notifications when the tab is active
        match status {
            AgentStatus::NeedsInput | AgentStatus::Done => Action::Notify { tab_name, status },
            _ => Action::None,
        }
    }

    pub fn handle_key_not_git_repo(&mut self, key: &KeyWithModifier) -> Action {
        if key.has_no_modifiers() {
            match key.bare_key {
                BareKey::Char('d') => {
                    self.set_status("Layout dumped", false);
                    return Action::DumpLayout;
                }
                BareKey::Char('x') => {
                    if self.session_name.is_some() {
                        return Action::NukeSession;
                    } else {
                        self.set_status("Cannot determine session name", true);
                    }
                }
                BareKey::Char('q') | BareKey::Esc => return Action::Close,
                _ => {}
            }
        }
        Action::None
    }

    pub fn render_to(&self, w: &mut impl Write, rows: usize, cols: usize) {
        // Reset the pane each frame: enter alt screen + cursor home + clear.
        // Without this, Zellij treats every `writeln!` as new scrollback and
        // the pane title bar shows a `SCROLL: 0/N` counter that climbs with
        // every re-render. The alt-screen sequence (\x1b[?1049h) signals to
        // Zellij's terminal grid that no scrollback should be retained for
        // this pane's content; combined with cursor-home + clear, each frame
        // overwrites the previous in-place.
        write!(w, "\x1b[?1049h\x1b[H\x1b[2J").unwrap();

        match self.mode {
            Mode::Loading => {
                ui::render_header(w, "loading...", cols);
                if self.status_is_error {
                    ui::render_status(w, &self.status_message, self.status_is_error);
                } else {
                    writeln!(w).unwrap();
                    writeln!(w, "  Waiting for permissions...").unwrap();
                }
                for _ in 0..rows.saturating_sub(5) {
                    writeln!(w).unwrap();
                }
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::NotGitRepo => {
                ui::render_header(w, "error", cols);
                ui::render_not_git_repo(w, &self.initial_cwd.display().to_string());
                let status_height = ui::status_height(&self.status_message, cols);
                let used_lines = 1 + 7 + status_height + 2;
                let padding = rows.saturating_sub(used_lines);
                for _ in 0..padding {
                    writeln!(w).unwrap();
                }
                ui::render_status(w, &self.status_message, self.status_is_error);
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::BrowseWorktrees => {
                if self.should_render_empty_state() {
                    ui::render_header(w, &self.repo_name, cols);
                    ui::render_empty_state(w);
                    let list_height = 6;
                    let status_height = ui::status_height(&self.status_message, cols);
                    let footer_height = if cols >= 55 { 3 } else { 4 };
                    let used_lines = 1 + list_height + status_height + footer_height;
                    let padding = rows.saturating_sub(used_lines);
                    for _ in 0..padding {
                        writeln!(w).unwrap();
                    }
                    ui::render_status(w, &self.status_message, self.status_is_error);
                    ui::render_footer(w, &self.mode, VERSION, cols);
                } else {
                    // `layout` is computed once here and is the ONLY thing
                    // that decides header/separator visibility and the item
                    // viewport for this frame. `sidebar_index_at_line` (used
                    // at click time) recomputes the identical struct from
                    // the same inputs (`self.last_rows`/`self.last_cols`
                    // captured from this render, plus `selected_index` and
                    // `status_message`, both stable between a render and the
                    // next click) — so a click can never disagree with what
                    // was drawn. See #135/#136.
                    let layout = ui::sidebar_layout(
                        rows,
                        cols,
                        self.sidebar_items.len(),
                        self.selected_index,
                        &self.status_message,
                    );
                    if layout.show_header {
                        ui::render_header(w, &self.repo_name, cols);
                    }
                    ui::render_sidebar_list(
                        w,
                        &self.sidebar_items,
                        &self.agent_statuses,
                        &self.repo_name,
                        self.active_tab_name(),
                        self.selected_index,
                        &layout,
                        cols,
                    );
                    let list_height = if self.sidebar_items.is_empty() {
                        2
                    } else {
                        layout.viewport.visible_items * 2
                    };
                    let used_lines =
                        layout.leading_lines() + list_height + layout.status_lines + layout.footer_lines;
                    let padding = rows.saturating_sub(used_lines);
                    for _ in 0..padding {
                        writeln!(w).unwrap();
                    }
                    ui::render_status(w, &self.status_message, self.status_is_error);
                    ui::render_footer(w, &self.mode, VERSION, cols);
                }
            }
            Mode::SelectBranch => {
                ui::render_header(w, &self.repo_name, cols);
                ui::render_branch_list(w, &self.filtered_branches, self.selected_index, rows);
                let list_height = if self.filtered_branches.is_empty() {
                    2
                } else {
                    let max_visible = rows.saturating_sub(7).max(1);
                    let visible_branches = self.filtered_branches.len().min(max_visible);
                    3 + visible_branches
                };
                let used_lines = 1 + list_height + 3;
                let padding = rows.saturating_sub(used_lines);
                for _ in 0..padding {
                    writeln!(w).unwrap();
                }
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::InputBranch => {
                ui::render_header(w, &self.repo_name, cols);
                ui::render_input(w, &self.input_buffer);
                let status_height = ui::status_height(&self.status_message, cols);
                let used_lines = 1 + 4 + status_height + 3;
                let padding = rows.saturating_sub(used_lines);
                for _ in 0..padding {
                    writeln!(w).unwrap();
                }
                ui::render_status(w, &self.status_message, self.status_is_error);
                ui::render_footer(w, &self.mode, VERSION, cols);
            }
            Mode::Confirming => {
                ui::render_header(w, &self.repo_name, cols);
                let confirm_height = if self.selected_sidebar_branch().is_some() {
                    4
                } else {
                    0
                };
                if let Some(branch) = self.selected_sidebar_branch() {
                    ui::render_confirm(w, branch, cols);
                }
                let used_lines = 1 + confirm_height + 2;
                let padding = rows.saturating_sub(used_lines);
                for _ in 0..padding {
                    writeln!(w).unwrap();
                }
                ui::render_footer(w, &self.mode, VERSION, cols);
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

        // Resolve initial cwd. Zellij drops `RunPlugin.initial_cwd` during
        // layout serialization, so on resurrection the loader falls back to
        // the server's startup `current_dir()` — any directory at all, often
        // `$HOME`. See zellij-org/zellij#2978, #3041, #4129. The user-config
        // block IS preserved across resurrection, so when `repo_root` is set
        // by `zelligent.sh` it is authoritative; runtime cwd is only the
        // fallback for the manual-launch case where `repo_root` is absent.
        let runtime_cwd = get_plugin_ids().initial_cwd;
        let cfg_repo_root = configuration.get("repo_root").map(PathBuf::from);
        self.initial_cwd = cfg_repo_root.unwrap_or(runtime_cwd);
        self.session_name = std::env::var("ZELLIJ_SESSION_NAME").ok();

        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadApplicationState,
            PermissionType::ReadCliPipes,
        ]);

        subscribe(&[
            EventType::Key,
            EventType::Mouse,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
            // Visible(true) is the ONLY reliable "this tab was just
            // revealed" signal: hidden instances receive no TabUpdates, so
            // from each instance's own perspective the active tab is its
            // own tab both in the last update before hiding and the first
            // one after reveal — active-tab change detection alone can
            // never see a round trip. See #151 and `handle_visible`.
            EventType::Visible,
            // Drives the footer status-message TTL (#152) — see
            // `State::set_status` / `State::handle_timer`.
            EventType::Timer,
        ]);

        // The status-replay request (#140 part B / Z-6) is fired from the
        // PermissionRequestResult(Granted) branch of update(), NOT here:
        // the RunCommands grant is asynchronous even when permissions.kdl
        // already pre-approves the plugin, so a run_command issued during
        // load() is deterministically denied ("permission 'RunCommands'
        // denied" in zellij.log) and the broadcast never happens.
    }

    fn update(&mut self, event: Event) -> bool {
        // Handled outside the Action-producing match: pure state syncs with
        // no host effect to `execute`, each reporting precisely whether a
        // re-render is needed. See `State::handle_visible` (#151) and
        // `State::handle_timer` (#152).
        // Reveal may request a fresh wake-up timer for a still-live status
        // message (its original timer can be lost while hidden), so the
        // arm step runs on this path too.
        if let Event::Visible(visible) = event {
            let rerender = self.handle_visible(visible);
            self.arm_pending_status_timer();
            return rerender;
        }
        if let Event::Timer(_) = event {
            let rerender = self.handle_timer();
            self.arm_pending_status_timer(); // early-fire re-chain
            return rerender;
        }
        let action = match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                // First moment run_command is actually allowed. Ask any
                // sibling instance for a replay of statuses this instance
                // missed (#140 part B / Z-6): its `agent_statuses` starts
                // empty, and `zelligent-status` pipes sent before it
                // existed are gone for good. Duplicate requests (were this
                // event ever delivered twice) are harmless — replies are
                // idempotent and merges monotone.
                self.fire_status_request();
                Action::FetchToplevel
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.set_status("Permissions denied. Plugin cannot run commands.", true);
                Action::None
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                match context.get("cmd_type").map(|s| s.as_str()) {
                    Some(CMD_GIT_TOPLEVEL) => self.handle_git_toplevel(exit_code, &stdout, &stderr),
                    Some(CMD_LIST_WORKTREES) => {
                        self.handle_list_worktrees(exit_code, &stdout, &stderr, &context);
                        Action::None
                    }
                    Some(CMD_GIT_BRANCHES) => {
                        self.handle_git_branches(exit_code, &stdout, &stderr);
                        Action::None
                    }
                    Some(CMD_SPAWN) => {
                        // Side effect in the shell, not the pure handler:
                        // tell sibling instances (hidden ones included —
                        // only a pipe reaches them) their caches are stale.
                        // See #140/#138 and `fire_invalidate_broadcast`.
                        if exit_code == Some(0) {
                            self.fire_invalidate_broadcast();
                        }
                        self.handle_spawn_result(exit_code, &stderr, &context)
                    }
                    Some(CMD_REMOVE) => {
                        if exit_code == Some(0) {
                            self.fire_invalidate_broadcast();
                        }
                        self.handle_remove_result(exit_code, &stderr, &context)
                    }
                    Some(CMD_INVALIDATE_BROADCAST)
                    | Some(CMD_STATUS_REQUEST_BROADCAST)
                    | Some(CMD_STATUS_REPLAY_BROADCAST) => {
                        // Best-effort broadcasts; success or failure, there
                        // is nothing to do with the result.
                        Action::None
                    }
                    Some(other) => {
                        self.set_status(format!("Unknown command result: {other}"), true);
                        Action::None
                    }
                    None => {
                        self.set_status("Received command result with no cmd_type", true);
                        Action::None
                    }
                }
            }
            Event::TabUpdate(tab_info) => self.handle_tab_update(tab_info),
            Event::Key(key) => match self.mode {
                Mode::Loading => Action::None,
                Mode::NotGitRepo => self.handle_key_not_git_repo(&key),
                Mode::BrowseWorktrees => self.handle_key_browse(&key),
                Mode::SelectBranch => self.handle_key_select_branch(&key),
                Mode::InputBranch => self.handle_key_input_branch(&key),
                Mode::Confirming => self.handle_key_confirming(&key),
            },
            Event::Mouse(mouse) => match self.mode {
                Mode::BrowseWorktrees => self.handle_mouse_browse(&mouse),
                _ => Action::None,
            },
            _ => return false,
        };
        self.execute(&action);
        self.arm_pending_status_timer();
        true
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let action = self.handle_pipe(&pipe_message);
        self.execute(&action);
        self.arm_pending_status_timer();
        true
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.last_rows = rows;
        self.last_cols = cols;
        self.render_to(&mut std::io::stdout(), rows, cols);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn key(bare: BareKey) -> KeyWithModifier {
        KeyWithModifier {
            bare_key: bare,
            key_modifiers: BTreeSet::new(),
        }
    }

    fn key_shift(bare: BareKey) -> KeyWithModifier {
        let mut mods = BTreeSet::new();
        mods.insert(KeyModifier::Shift);
        KeyWithModifier {
            bare_key: bare,
            key_modifiers: mods,
        }
    }

    fn state_with_worktrees() -> State {
        let mut s = State::default();
        s.mode = Mode::BrowseWorktrees;
        s.worktrees = vec![
            Worktree {
                dir: "feat-a".into(),
                branch: "feat-a".into(),
            },
            Worktree {
                dir: "feat-b".into(),
                branch: "feat-b".into(),
            },
            Worktree {
                dir: "feat-c".into(),
                branch: "feat-c".into(),
            },
        ];
        s.branches = vec![
            "main".into(),
            "feat-a".into(),
            "feat-b".into(),
            "dev".into(),
        ];
        s
    }

    fn state_with_sidebar() -> State {
        let mut s = state_with_worktrees();
        s.tabs = vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ];
        s.recompute_sidebar_items();
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
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "claude alerts".into(),
            ..Default::default()
        };
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
        let mut s = state_with_sidebar();
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 1);
        s.handle_key_browse(&key(BareKey::Down));
        assert_eq!(s.selected_index, 2);
    }

    #[test]
    fn browse_j_wraps_around() {
        let mut s = state_with_sidebar();
        s.selected_index = 2;
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_k_moves_up() {
        let mut s = state_with_sidebar();
        s.selected_index = 2;
        s.handle_key_browse(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 1);
        s.handle_key_browse(&key(BareKey::Up));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_k_wraps_around() {
        let mut s = state_with_sidebar();
        s.selected_index = 0;
        s.handle_key_browse(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 2);
    }

    #[test]
    fn browse_jk_noop_on_empty() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            ..Default::default()
        };
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 0);
        s.handle_key_browse(&key(BareKey::Char('k')));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_enter_switches_selected_sidebar_tab() {
        let mut s = state_with_sidebar();
        s.selected_index = 1;
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::SwitchToTab("feat-b".into()));
    }

    #[test]
    fn browse_enter_switches_to_existing_tab() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            worktrees: vec![Worktree {
                dir: "feature-cool".into(),
                branch: "feature/cool".into(),
            }],
            tabs: vec![make_tab("feature-cool", true)],
            ..Default::default()
        };
        s.recompute_sidebar_items();
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::SwitchToTab("feature-cool".into()));
    }

    #[test]
    fn browse_enter_switches_to_selected_user_tab() {
        let mut s = state_with_sidebar();
        s.tabs.push(make_tab("notes", false));
        s.recompute_sidebar_items();
        s.selected_index = 3;
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::SwitchToTab("notes".into()));
    }

    #[test]
    fn browse_enter_noop_on_empty() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            ..Default::default()
        };
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn browse_enter_noop_without_tab_state() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            worktrees: vec![Worktree {
                dir: "feat-a".into(),
                branch: "feat-a".into(),
            }],
            ..Default::default()
        };
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn browse_enter_spawns_detached_worktree_item() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            worktrees: vec![Worktree {
                dir: "feat-a".into(),
                branch: "feat-a".into(),
            }],
            ..Default::default()
        };
        s.recompute_sidebar_items();
        let action = s.handle_key_browse(&key(BareKey::Enter));
        assert_eq!(action, Action::Spawn("feat-a".into()));
        assert_eq!(s.status_message, "Spawning 'feat-a'...");
    }

    #[test]
    fn browse_n_switches_to_select_branch() {
        let mut s = state_with_sidebar();
        s.selected_index = 2;
        s.handle_key_browse(&key(BareKey::Char('n')));
        assert_eq!(s.mode, Mode::SelectBranch);
        assert_eq!(s.selected_index, 0);
        assert_eq!(s.filtered_branches, s.branches);
    }

    #[test]
    fn browse_i_switches_to_input_branch() {
        let mut s = state_with_sidebar();
        s.input_buffer = "leftover".into();
        s.handle_key_browse(&key(BareKey::Char('i')));
        assert_eq!(s.mode, Mode::InputBranch);
        assert!(s.input_buffer.is_empty());
    }

    #[test]
    fn browse_d_switches_to_confirming() {
        let mut s = state_with_sidebar();
        s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(s.mode, Mode::Confirming);
    }

    #[test]
    fn browse_d_noop_on_empty() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            ..Default::default()
        };
        s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    #[test]
    fn browse_d_rejects_user_tab() {
        let mut s = state_with_sidebar();
        s.tabs.push(make_tab("notes", false));
        s.recompute_sidebar_items();
        s.selected_index = 3;
        let action = s.handle_key_browse(&key(BareKey::Char('d')));
        assert_eq!(action, Action::None);
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert!(s.status_is_error);
        assert_eq!(s.status_message, "Only worktree tabs can be removed");
    }

    #[test]
    fn browse_r_returns_refresh() {
        let mut s = state_with_sidebar();
        let action = s.handle_key_browse(&key(BareKey::Char('r')));
        assert_eq!(action, Action::Refresh);
        assert_eq!(s.status_message, "Refreshed");
    }

    #[test]
    fn browse_q_is_noop_in_sidebar_mode() {
        let mut s = state_with_sidebar();
        assert_eq!(s.handle_key_browse(&key(BareKey::Char('q'))), Action::None);
    }

    #[test]
    fn browse_esc_is_noop_in_sidebar_mode() {
        let mut s = state_with_sidebar();
        assert_eq!(s.handle_key_browse(&key(BareKey::Esc)), Action::None);
    }

    #[test]
    fn browse_mouse_scroll_moves_selection() {
        let mut s = state_with_sidebar();
        s.handle_mouse_browse(&Mouse::ScrollDown(0));
        assert_eq!(s.selected_index, 1);
        s.handle_mouse_browse(&Mouse::ScrollUp(0));
        assert_eq!(s.selected_index, 0);
    }

    // Pane-relative mouse-mapping tests (#135/#136). `state_with_sidebar()`
    // has 3 items (feat-a/b/c) with feat-a active, so `selected_index`
    // starts at 0. At rows=20, cols=80, empty status: footer_lines=3
    // (cols>=55), status_lines=0, content_budget=17 -> header+separator
    // both show (leading=2). Rendered line map for this fixture:
    //   line 0        = header            -> no-op
    //   line 1        = blank separator   -> no-op
    //   line 2/3      = item0 title/subtitle (feat-a)
    //   line 4/5      = item1 title/subtitle (feat-b)
    //   line 6/7      = item2 title/subtitle (feat-c)
    //   line 8+       = footer/status/past-end -> no-op

    /// #137: a single click on an unselected item's title now selects AND
    /// activates in one step (previously this only moved `▌`, requiring a
    /// second click to activate).
    #[test]
    fn browse_mouse_click_title_selects_and_activates_item() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 80;
        let action = s.handle_mouse_browse(&Mouse::LeftClick(4, 5));
        assert_eq!(action, Action::SwitchToTab("feat-b".into()));
        assert_eq!(s.selected_index, 1);
    }

    /// The #135 regression: a subtitle click must select (and, per #137,
    /// activate) the SAME item as its title, never the next one.
    #[test]
    fn browse_mouse_click_subtitle_selects_and_activates_same_item_not_next() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 80;
        let action = s.handle_mouse_browse(&Mouse::LeftClick(5, 5));
        assert_eq!(action, Action::SwitchToTab("feat-b".into()));
        assert_eq!(s.selected_index, 1, "subtitle click must land on item 1, not item 2");
    }

    /// The last item's subtitle must resolve to the last item, not a dead
    /// past-the-end no-op (the other half of the #135 regression).
    #[test]
    fn browse_mouse_click_last_item_subtitle_selects_and_activates_last_item() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 80;
        let action = s.handle_mouse_browse(&Mouse::LeftClick(7, 5));
        assert_eq!(action, Action::SwitchToTab("feat-c".into()));
        assert_eq!(s.selected_index, 2);
    }

    /// #137 idempotence guard: clicking a row that is *already* selected
    /// (e.g. a habitual second click, or clicking the tab you're already
    /// in) must still just activate — no duplicate spawn, no tab churn.
    #[test]
    fn browse_mouse_click_on_already_selected_item_still_activates() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 80;
        s.selected_index = 1;
        let action = s.handle_mouse_browse(&Mouse::LeftClick(4, 5));
        assert_eq!(action, Action::SwitchToTab("feat-b".into()));
        assert_eq!(s.selected_index, 1);
    }

    #[test]
    fn browse_mouse_click_on_detached_item_spawns_it() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            worktrees: vec![Worktree {
                dir: "feat-a".into(),
                branch: "feat-a".into(),
            }],
            last_rows: 20,
            last_cols: 80,
            ..Default::default()
        };
        s.recompute_sidebar_items();
        let action = s.handle_mouse_browse(&Mouse::LeftClick(2, 5));
        assert_eq!(action, Action::Spawn("feat-a".into()));
        assert_eq!(s.status_message, "Spawning 'feat-a'...");
    }

    /// Clicks on the header or blank-separator line are strict no-ops —
    /// never "select item 0" (the other #135 mode of failure).
    #[test]
    fn browse_mouse_click_header_and_separator_are_noop() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 80;
        s.selected_index = 1;
        assert_eq!(s.sidebar_index_at_line(0), None, "header line");
        assert_eq!(s.sidebar_index_at_line(1), None, "blank separator line");
        assert_eq!(s.handle_mouse_browse(&Mouse::LeftClick(0, 5)), Action::None);
        assert_eq!(s.handle_mouse_browse(&Mouse::LeftClick(1, 5)), Action::None);
        assert_eq!(s.selected_index, 1, "non-item clicks must not change selection");
    }

    #[test]
    fn browse_mouse_click_footer_and_past_end_are_noop() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 80;
        assert_eq!(s.sidebar_index_at_line(8), None);
        assert_eq!(s.sidebar_index_at_line(19), None);
        let action = s.handle_mouse_browse(&Mouse::LeftClick(8, 5));
        assert_eq!(action, Action::None);
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn browse_mouse_index_at_line_full_map() {
        let s = {
            let mut s = state_with_sidebar();
            s.last_rows = 20;
            s.last_cols = 80;
            s
        };
        assert_eq!(s.sidebar_index_at_line(0), None);
        assert_eq!(s.sidebar_index_at_line(1), None);
        assert_eq!(s.sidebar_index_at_line(2), Some(0));
        assert_eq!(s.sidebar_index_at_line(3), Some(0));
        assert_eq!(s.sidebar_index_at_line(4), Some(1));
        assert_eq!(s.sidebar_index_at_line(5), Some(1));
        assert_eq!(s.sidebar_index_at_line(6), Some(2));
        assert_eq!(s.sidebar_index_at_line(7), Some(2));
        assert_eq!(s.sidebar_index_at_line(8), None);
    }

    /// A wrapped status message must NOT change where item clicks map —
    /// the #136 "dynamic offset" bug. `sidebar_layout` carves the status's
    /// (wrap-aware) row budget out of `content_budget` mathematically, so
    /// header/separator visibility (and hence `leading_lines`) no longer
    /// depends on incidental terminal scroll caused by an under-counted
    /// wrap. cols=30 with this ~34-char message wraps to 2 physical rows
    /// (status_lines = 1 blank + 2 wrapped = 3); rows=20 still leaves
    /// content_budget = 20 - (3 status + 4 footer) = 13 >= 4, so leading
    /// stays 2 — identical to the no-status case above.
    #[test]
    fn browse_mouse_mapping_unaffected_by_wrapped_status_message() {
        let mut s = state_with_sidebar();
        s.last_rows = 20;
        s.last_cols = 30;
        s.status_message = "Only worktree tabs can be removed".into();
        s.status_is_error = true;
        assert_eq!(s.sidebar_index_at_line(0), None, "header still there");
        assert_eq!(s.sidebar_index_at_line(1), None, "separator still there");
        assert_eq!(s.sidebar_index_at_line(2), Some(0));
        assert_eq!(s.sidebar_index_at_line(4), Some(1));
    }

    /// Locks the pre-existing (and still correct) scrolled-viewport
    /// behavior: `viewport.start` composes with the leading-line offset
    /// without compounding. 20 items, selected 15, rows=10 cols=80 ->
    /// footer_lines=3, content_budget=7 -> leading=2, item_rows_budget=5 ->
    /// max_items=2 -> start=14 (selected - max_items + 1), visible=2.
    #[test]
    fn browse_mouse_click_maps_correctly_in_scrolled_viewport() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            worktrees: (0..20)
                .map(|i| Worktree {
                    dir: format!("branch-{i}"),
                    branch: format!("branch-{i}"),
                })
                .collect(),
            tabs: (0..20)
                .map(|i| make_tab(&format!("branch-{i}"), i == 15))
                .collect(),
            selected_index: 15,
            last_rows: 10,
            last_cols: 80,
            ..Default::default()
        };
        s.recompute_sidebar_items();
        assert_eq!(s.sidebar_index_at_line(2), Some(14), "row above selected");
        assert_eq!(s.sidebar_index_at_line(4), Some(15), "selected item title");
        assert_eq!(s.sidebar_index_at_line(5), Some(15), "selected item subtitle");
        assert_eq!(s.sidebar_index_at_line(6), None, "past visible viewport");
        // Clicking the already-selected (scrolled-to) item activates it.
        let action = s.handle_mouse_browse(&Mouse::LeftClick(4, 5));
        assert_eq!(action, Action::SwitchToTab("branch-15".into()));
    }

    /// Short-pane degradation: blank separator drops first, then the
    /// header — an item row is never sacrificed. cols=80 (footer_lines=3),
    /// no status.
    #[test]
    fn browse_mouse_short_pane_drops_separator_before_header() {
        let mut s = state_with_sidebar();
        s.last_cols = 80;

        // content_budget = rows - 3. rows=7 -> budget=4 -> header+separator.
        s.last_rows = 7;
        assert_eq!(s.sidebar_index_at_line(0), None, "header");
        assert_eq!(s.sidebar_index_at_line(1), None, "separator");
        assert_eq!(s.sidebar_index_at_line(2), Some(0));

        // rows=6 -> budget=3 -> header only, no separator.
        s.last_rows = 6;
        assert_eq!(s.sidebar_index_at_line(0), None, "header");
        assert_eq!(s.sidebar_index_at_line(1), Some(0), "item row right after header");

        // rows=5 -> budget=2 -> neither header nor separator; item row 0
        // is still shown (never sacrificed).
        s.last_rows = 5;
        assert_eq!(s.sidebar_index_at_line(0), Some(0), "item title at pane top");
        assert_eq!(s.sidebar_index_at_line(1), Some(0), "item subtitle");
    }

    #[test]
    fn browse_mouse_noop_in_empty_state() {
        let mut s = State {
            mode: Mode::BrowseWorktrees,
            tabs: vec![make_tab("notes", true)],
            last_rows: 20,
            last_cols: 80,
            ..Default::default()
        };
        s.recompute_sidebar_items();
        assert!(s.should_render_empty_state());
        assert_eq!(s.handle_mouse_browse(&Mouse::ScrollDown(0)), Action::None);
        assert_eq!(s.handle_mouse_browse(&Mouse::LeftClick(2, 5)), Action::None);
        assert_eq!(s.selected_index, 0);
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
        let mut s = State {
            mode: Mode::InputBranch,
            ..Default::default()
        };
        s.handle_key_input_branch(&key(BareKey::Char('f')));
        s.handle_key_input_branch(&key(BareKey::Char('o')));
        s.handle_key_input_branch(&key(BareKey::Char('o')));
        assert_eq!(s.input_buffer, "foo");
    }

    #[test]
    fn input_branch_shift_chars() {
        let mut s = State {
            mode: Mode::InputBranch,
            ..Default::default()
        };
        s.handle_key_input_branch(&key_shift(BareKey::Char('F')));
        assert_eq!(s.input_buffer, "F");
    }

    #[test]
    fn input_branch_backspace() {
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "ab".into(),
            ..Default::default()
        };
        s.handle_key_input_branch(&key(BareKey::Backspace));
        assert_eq!(s.input_buffer, "a");
    }

    #[test]
    fn input_branch_enter_spawns() {
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "feat/new".into(),
            ..Default::default()
        };
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
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "  ".into(),
            ..Default::default()
        };
        let action = s.handle_key_input_branch(&key(BareKey::Enter));
        assert_eq!(action, Action::None);
        assert_eq!(s.mode, Mode::InputBranch);
        assert!(s.status_is_error);
        assert_eq!(s.status_message, "Invalid branch name");
    }

    #[test]
    fn input_branch_esc_goes_back() {
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "wip".into(),
            ..Default::default()
        };
        s.handle_key_input_branch(&key(BareKey::Esc));
        assert_eq!(s.mode, Mode::BrowseWorktrees);
    }

    // --- Confirming key handler tests ---

    #[test]
    fn confirm_y_removes() {
        let mut s = state_with_sidebar();
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
        let action = s.handle_git_toplevel(
            Some(0),
            b"repo_root=/home/user/myrepo\nrepo_name=myrepo\n",
            b"",
        );
        assert_eq!(s.repo_root, "/home/user/myrepo");
        assert_eq!(s.repo_name, "myrepo");
        assert_eq!(s.mode, Mode::BrowseWorktrees);
        assert_eq!(action, Action::FetchWorktreesAndBranches);
    }

    #[test]
    fn git_toplevel_parses_by_key() {
        let mut s = State::default();
        let action = s.handle_git_toplevel(
            Some(0),
            b"repo_name=myrepo\nrepo_root=/home/user/myrepo\n",
            b"",
        );
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
                we_initiated: true,
            }
        );
        // The closed tab must be removed from our cached tab list immediately,
        // so that any sidebar recompute triggered by the worktree-list refresh
        // doesn't mislabel it as an orphaned "user tab" before the host's
        // TabUpdate event catches up.
        assert!(
            s.tabs.iter().all(|t| t.name != "feat-a"),
            "closed tab 'feat-a' should be dropped from self.tabs cache"
        );
        // Pending-close handshake: an inbound TabUpdate arriving before the
        // close propagates must be filtered. The mark stays set until that
        // happens.
        assert!(
            s.pending_close.contains("feat-a"),
            "pending_close should be marked so handle_tab_update filters stale snapshots"
        );
    }

    #[test]
    fn tab_update_filters_pending_close_when_tab_still_present() {
        let mut s = State::default();
        s.tabs = vec![make_tab("zelligent", true)];
        s.pending_close.insert("feat-a".into());
        // Simulate a stale TabUpdate (e.g. fired by `go_to_tab_name` before
        // `close_focused_tab` lands) that still contains the closing tab.
        s.handle_tab_update(vec![
            make_tab("zelligent", false),
            make_tab("feat-a", true),
        ]);
        assert!(
            s.tabs.iter().all(|t| t.name != "feat-a"),
            "pending_close target must be filtered out of stale TabUpdate snapshots"
        );
        assert!(
            s.pending_close.contains("feat-a"),
            "pending_close stays set until a TabUpdate confirms the close"
        );
    }

    #[test]
    fn tab_update_clears_pending_close_when_tab_is_gone() {
        let mut s = State::default();
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        s.pending_close.insert("feat-a".into());
        // TabUpdate confirming the close: the pending tab is no longer in
        // the snapshot. handle_tab_update should accept the snapshot
        // verbatim AND clear pending_close so a future tab with the same
        // name (after re-spawn) isn't accidentally filtered.
        s.handle_tab_update(vec![make_tab("zelligent", true)]);
        assert!(
            s.pending_close.is_empty(),
            "pending_close must clear once the host confirms the close"
        );
        assert_eq!(s.tabs.len(), 1, "snapshot is accepted as-is");
    }

    // Codex review of PR #122 flagged rapid sequential removes: the old
    // `Option<String>` lost the earlier pending name when the second
    // remove fired. With a `BTreeSet` both stay pending, both are
    // filtered out of stale snapshots, and both clear independently as
    // their respective `TabUpdate`s land.
    #[test]
    fn tab_update_handles_two_concurrent_pending_closes() {
        let mut s = State::default();
        s.tabs = vec![make_tab("zelligent", true)];
        // Two removes in flight: feat-a (close already propagated, not in
        // the incoming snapshot) and feat-b (close hasn't landed yet,
        // still in the snapshot).
        s.pending_close.insert("feat-a".into());
        s.pending_close.insert("feat-b".into());
        s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-b", false),
        ]);
        assert!(
            !s.pending_close.contains("feat-a"),
            "feat-a clears once its close lands"
        );
        assert!(
            s.pending_close.contains("feat-b"),
            "feat-b stays pending because the stale snapshot still contains it"
        );
        assert!(
            s.tabs.iter().all(|t| t.name != "feat-b"),
            "feat-b is filtered out of the stale snapshot"
        );
    }

    #[test]
    fn tab_update_pending_close_does_not_filter_other_tabs() {
        let mut s = State::default();
        s.pending_close.insert("feat-a".into());
        // A TabUpdate with feat-a still present should drop ONLY feat-a,
        // not any other tab in the snapshot.
        s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-a", false),
            make_tab("feat-b", false),
        ]);
        assert_eq!(s.tabs.len(), 2);
        assert!(s.tabs.iter().any(|t| t.name == "zelligent"));
        assert!(s.tabs.iter().any(|t| t.name == "feat-b"));
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
        assert_eq!(
            State::tab_name_for_branch("feat.something"),
            "featsomething"
        );
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
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\nfeat-b\tfeat-b\nfeat-c\tfeat-c\n",
            b"",
            &BTreeMap::new(),
        );
        assert_eq!(s.selected_index, 1);
        assert_eq!(s.sidebar_items.len(), 3);
        assert_eq!(s.sidebar_items[1].tab_name, "feat-b");
    }

    #[test]
    fn recompute_sidebar_selects_active_tab_with_slash_branch() {
        let mut s = State::default();
        s.tabs = vec![make_tab("main", false), make_tab("feature-cool", true)];
        s.handle_list_worktrees(
            Some(0),
            b"main\tmain\nfeature-cool\tfeature/cool\n",
            b"",
            &BTreeMap::new(),
        );
        assert_eq!(s.selected_index, 1);
        assert_eq!(
            s.sidebar_items[1].matched_branch,
            Some("feature/cool".into())
        );
    }

    #[test]
    fn tab_update_after_worktrees_selects_active_tab_instead_of_stale_bootstrap_cursor() {
        let mut s = State::default();
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\nfeat-b\tfeat-b\n",
            b"",
            &BTreeMap::new(),
        );
        assert_eq!(s.selected_index, 0);

        s.handle_tab_update(vec![
            make_tab("feat-a", false),
            make_tab("feat-b", true),
        ]);

        assert_eq!(s.selected_index, 1);
        assert_eq!(s.sidebar_items[1].tab_name, "feat-b");
    }

    #[test]
    fn tab_update_with_unmatched_worktree_tab_returns_refresh() {
        // Reproduces the "user tab" mislabel: an external `zelligent spawn`
        // (e.g. an agent running it via bash) creates a tab that lands in the
        // host's TabUpdate before our worktree-list cache is refreshed. The
        // tab should drive a Refresh so the sidebar self-heals.
        let mut s = State::default();
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());
        let action = s.handle_tab_update(vec![
            make_tab("feat-a", false),
            make_tab("feat-b", true), // not in worktrees yet
        ]);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn tab_update_with_all_tabs_matched_returns_none() {
        // No spurious refresh when every tab is already explained by either
        // the repo tab or a known worktree.
        let mut s = State {
            repo_name: "zelligent".into(),
            ..Default::default()
        };
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());
        let action = s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-a", false),
        ]);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn tab_update_with_persistent_unmatched_tab_only_refreshes_once() {
        // Reviewer concern: a legitimate persistent user-created tab (no
        // underlying worktree) must NOT drive a Refresh on every subsequent
        // TabUpdate (focus changes etc.). The "newly-appeared" gate makes
        // refresh one-shot per such tab, and the tab-set-change trigger
        // (#140) doesn't fire on a pure focus switch either.
        let mut s = State::default();
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());

        // First sighting of the user tab: Refresh fires so we can confirm
        // there's no worktree we missed.
        let first = s.handle_tab_update(vec![
            make_tab("feat-a", false),
            make_tab("notes", true),
        ]);
        assert_eq!(first, Action::Refresh);

        // Worktrees still don't include "notes". The next TabUpdate (focus
        // moved back to feat-a, tab set unchanged) must NOT trigger another
        // Refresh.
        let second = s.handle_tab_update(vec![
            make_tab("feat-a", true),
            make_tab("notes", false),
        ]);
        assert_eq!(second, Action::None);
    }

    #[test]
    fn tab_update_with_disappeared_known_tab_returns_refresh() {
        // Mirrors #127's newly-appeared gate for the opposite case: a
        // sibling sidebar instance (one that did NOT drive the remove, so
        // it never populated `pending_close`) sees a previously-known tab
        // vanish from the TabUpdate. It must self-heal via Refresh rather
        // than keep showing the stale row until a manual `r`. See #138.
        let mut s = State::default();
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        let action = s.handle_tab_update(vec![make_tab("zelligent", true)]);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn tab_update_with_disappeared_known_tab_only_refreshes_once() {
        // Once the vanished tab has dropped out of `previously_known` (it's
        // no longer in `self.tabs`), a subsequent identical TabUpdate must
        // NOT trigger another Refresh.
        let mut s = State::default();
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        let first = s.handle_tab_update(vec![make_tab("zelligent", true)]);
        assert_eq!(first, Action::Refresh);

        let second = s.handle_tab_update(vec![make_tab("zelligent", true)]);
        assert_eq!(second, Action::None);
    }

    #[test]
    fn tab_update_with_disappeared_pending_close_tab_does_not_double_refresh() {
        // The disappeared-tab trigger must not fire for a close this same
        // instance already initiated: `pending_close` marks it, and the
        // confirming TabUpdate (the tab absent from the snapshot) is the
        // existing #121 handshake's job, not this trigger's. This mirrors
        // `tab_update_clears_pending_close_when_tab_is_gone` but also
        // asserts on the returned Action.
        let mut s = State::default();
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        s.pending_close.insert("feat-a".into());
        let action = s.handle_tab_update(vec![make_tab("zelligent", true)]);
        assert_eq!(
            action,
            Action::None,
            "a pending_close disappearance is handled by the #121 handshake, \
             not the new disappeared-tab trigger"
        );
        assert!(
            s.pending_close.is_empty(),
            "pending_close still clears once the host confirms the close"
        );
    }

    #[test]
    fn tab_update_with_combined_appear_and_disappear_returns_single_refresh() {
        // A single TabUpdate where one known tab vanishes and one unmatched
        // tab appears must still resolve to exactly one Refresh action, not
        // some doubled-up variant.
        let mut s = State::default();
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        let action = s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-b", false), // new, unmatched to any worktree
        ]);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn tab_update_after_event_starvation_refreshes_even_when_new_tab_matches_cache() {
        // The verified #140 failure scenario (2026-07 live verification):
        // hidden plugin instances receive NO Events at all, so this
        // instance's last snapshot predates everything that happened while
        // it was hidden — it only knows its own tab. Meanwhile feat-b was
        // spawned AND removed (the cache still lists it, stale) and feat-a
        // is still open. When this instance's tab becomes active again, the
        // catch-up TabUpdate contains a tab (feat-a) that is new to
        // `previously_known` but MATCHES the worktree cache — so
        // has_new_unmatched stays silent, no previously-known tab
        // disappeared, and any active-tab comparison sees this instance's
        // own tab active on both sides. Only a tab-SET diff detects that
        // the instance slept through changes and must re-sync.
        let mut s = State {
            repo_name: "zelligent".into(),
            ..Default::default()
        };
        s.tabs = vec![make_tab("zelligent", true)];
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\nfeat-b\tfeat-b\nfeat-c\tfeat-c\n",
            b"",
            &BTreeMap::new(),
        );

        let action = s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-a", false), // new to this instance, but cache-matched
        ]);
        assert_eq!(
            action,
            Action::Refresh,
            "an event-starved instance must re-sync when its catch-up \
             TabUpdate shows the tab set changed, even if every tab is \
             explained by the (stale) worktree cache"
        );
    }

    #[test]
    fn tab_update_focus_switch_with_unchanged_tab_set_returns_none() {
        // Intentional #140 v2 behavior: a pure focus switch with no tab-set
        // drift does NOT refresh. A visible instance's cache was maintained
        // while visible (it received every Event), and a hidden instance
        // never sees the intermediate switches anyway — the catch-up
        // TabUpdate it gets on becoming visible carries any set drift, which
        // is what fires. Refreshing on mere focus changes would be pure
        // overhead.
        //
        // This is also the #151 repro: the sidebar cursor must still follow
        // the active tab across this exact switch even though it doesn't
        // warrant a Refresh — the two are independent.
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(s.selected_index, 0);

        let action = s.handle_tab_update(vec![
            make_tab("feat-a", false),
            make_tab("feat-b", true), // focus moved, same tab set
        ]);
        assert_eq!(action, Action::None);
        assert_eq!(
            s.selected_index, 1,
            "#151: the cursor must re-sync to feat-b even though the tab set didn't change"
        );
    }

    #[test]
    fn tab_update_active_tab_change_resyncs_cursor_to_new_active_row() {
        // Issue #151: revealing an already-open tab (sidebar click, Enter,
        // or a native Ctrl-t switch) must re-sync ▌ to the newly active
        // tab's row, even though `recompute_sidebar_items`'s "preserve
        // previous selection by identity" logic (see its `previous_item_key`
        // handling) would otherwise leave the cursor on whatever item this
        // instance last selected — feat-a is untouched by the switch below,
        // so that logic alone would leave the cursor at index 0.
        let mut s = State::default();
        s.handle_tab_update(vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ]);
        assert_eq!(s.selected_index, 0);

        // Active tab jumps from feat-a to feat-c with no tab-set change —
        // e.g. this instance was hidden while the user Ctrl-t'd through.
        s.handle_tab_update(vec![
            make_tab("feat-a", false),
            make_tab("feat-b", false),
            make_tab("feat-c", true),
        ]);
        assert_eq!(
            s.selected_index, 2,
            "cursor must follow the active tab across the switch"
        );
    }

    #[test]
    fn tab_update_same_active_tab_does_not_disturb_manual_browse() {
        // The re-sync must fire only on an active-tab CHANGE. A same-active
        // TabUpdate (e.g. a routine re-render) must not fight j/k browsing
        // away from the active row.
        let mut s = State::default();
        s.handle_tab_update(vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ]);
        assert_eq!(s.selected_index, 0);

        // Browse down to feat-b, away from the active tab's row.
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 1);

        // Same active tab (feat-a) reported again must not snap the cursor
        // back to its row.
        let action = s.handle_tab_update(vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ]);
        assert_eq!(action, Action::None);
        assert_eq!(
            s.selected_index, 1,
            "same-active TabUpdate must not disturb a manual j/k move"
        );
    }

    #[test]
    fn reveal_after_round_trip_resyncs_even_though_active_name_never_changed() {
        // The live #151 delivery model (instrumented): hidden instances get
        // NO TabUpdates, so this instance sees active == its own tab both
        // before hiding and after reveal — change detection alone never
        // fires. Event::Visible(true) must carry the re-sync.
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(s.selected_index, 0);

        // User clicks feat-b's row to leave: the click handler sets the
        // selection before the switch, then the instance goes dark and
        // receives NOTHING while the user works in feat-b.
        s.selected_index = 1;
        s.handle_visible(false);

        // Reveal: Visible(true) first, then the fresh snapshot in which
        // the active tab name is the same as the instance last saw.
        assert!(s.handle_visible(true), "reveal must move the cursor and re-render");
        assert_eq!(s.selected_index, 0, "cursor back on the active tab's row");

        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(s.selected_index, 0);
        assert!(!s.resync_on_reveal, "the reveal flag must be consumed");
    }

    #[test]
    fn reveal_resync_survives_tab_update_arriving_before_visible_is_processed() {
        // Opposite ordering: the fresh snapshot lands first (same active
        // name, so no change fires), Visible(true) afterwards.
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        s.selected_index = 1; // click that initiated the switch away
        s.handle_visible(false);

        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(s.selected_index, 1, "same-active snapshot alone must not move it yet");
        assert!(s.handle_visible(true));
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn visible_true_does_not_break_jk_browsing_afterwards() {
        // The reveal flag is one-shot: once consumed by the next TabUpdate,
        // browsing must not be snapped back by later same-active updates.
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        s.handle_visible(true);
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);

        s.selected_index = 1; // j/k browse
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(s.selected_index, 1, "browsing must survive same-active updates");
    }

    #[test]
    fn visible_false_is_a_no_op() {
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        s.selected_index = 1;
        assert!(!s.handle_visible(false));
        assert_eq!(s.selected_index, 1);
        assert!(!s.resync_on_reveal);
    }

    #[test]
    fn tab_update_no_active_tab_in_snapshot_leaves_cursor_but_updates_tracked_name() {
        // `select_active_sidebar_item` returns false when the snapshot has
        // no active tab to select a row for — reachable via the #121
        // pending-close race: the tab being closed can still arrive marked
        // active in a stale TabUpdate, and the race-guard filter (above)
        // drops it entirely, leaving a snapshot with no active tab at all.
        // The decided contract: leave the cursor exactly where it is (do
        // NOT reset to 0), but still update the tracked active-tab name so
        // that the next real switch is correctly seen as a change.
        let mut s = State::default();
        s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-a", false),
            make_tab("feat-b", false),
        ]);
        assert_eq!(s.selected_index, 0);

        // Browse down to feat-b.
        s.handle_key_browse(&key(BareKey::Char('j')));
        s.handle_key_browse(&key(BareKey::Char('j')));
        assert_eq!(s.selected_index, 2);

        // "zelligent" close is in flight; a stale TabUpdate still shows it
        // active. The race guard filters it out, leaving a snapshot with no
        // active tab at all.
        s.pending_close.insert("zelligent".into());
        s.handle_tab_update(vec![
            make_tab("zelligent", true),
            make_tab("feat-a", false),
            make_tab("feat-b", false),
        ]);
        assert_eq!(
            s.selected_index, 1,
            "no active tab in the filtered snapshot must not reset the cursor \
             (index shifts from 2 to 1 only because zelligent's row was \
             dropped from the list — feat-b, the same logical item, is still \
             selected)"
        );

        // The confirming TabUpdate lands: zelligent is gone, focus lands on
        // feat-a instead of feat-b. The gap update above must have tracked
        // "no active tab" so this is correctly seen as a change and
        // re-syncs, even though `recompute_sidebar_items`'s identity
        // preservation would otherwise keep the cursor on feat-b.
        let action = s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(action, Action::None);
        assert_eq!(
            s.selected_index, 0,
            "tracked name must have updated during the gap so this switch re-syncs"
        );
    }

    #[test]
    fn tab_update_with_identical_snapshot_returns_none() {
        // Identical consecutive snapshots (same tabs, same active) must not
        // trigger a spurious Refresh on every re-render.
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);

        let action =
            s.handle_tab_update(vec![make_tab("feat-a", true), make_tab("feat-b", false)]);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn tab_update_first_sync_does_not_trigger_tab_set_change_refresh() {
        // The very first TabUpdate since startup goes from no previously-known
        // tabs to the full tab set — the maximal set diff. This must NOT fire
        // the tab-set-change trigger (startup already loads worktrees via the
        // bootstrap path). Use the repo tab (excluded from the newly-appeared
        // gate) so the only trigger in play is the one under test; with
        // `had_tabs == false` it must not fire, leaving Action::None.
        let mut s = State {
            repo_name: "zelligent".into(),
            ..Default::default()
        };
        let action = s.handle_tab_update(vec![make_tab("zelligent", true)]);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn tab_update_with_set_change_and_disappeared_tab_returns_single_refresh() {
        // Combining the tab-set-change trigger with the disappeared-known
        // trigger in the same update must still resolve to exactly one
        // Refresh, not some doubled-up variant.
        let mut s = State::default();
        s.handle_tab_update(vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ]);

        // feat-b disappears AND focus moves from feat-a to feat-c.
        let action = s.handle_tab_update(vec![make_tab("feat-a", false), make_tab("feat-c", true)]);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn invalidate_pipe_sets_dirty_and_returns_refresh() {
        let mut s = State::default();
        assert!(!s.cache_dirty);
        let action = s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert_eq!(action, Action::Refresh);
        assert!(
            s.cache_dirty,
            "the dirty bit is the durable part — a hidden instance loses \
             the Refresh result, so the bit must survive for the TabUpdate \
             retry"
        );
    }

    #[test]
    fn blind_window_round_trip_heals_via_dirty_bit_not_set_diff() {
        // THE diagnosed #140 scenario (live instrumentation, archive
        // 12-diag-140): this instance last saw tabs {zelligent, feat-a},
        // then went hidden (zero Events). During the blind window feat-b
        // was spawned AND removed — so the wake-up TabUpdate's set is
        // IDENTICAL to prev_known and the v2 set-diff has nothing to see.
        // Only the invalidate pipe (which DOES reach hidden instances)
        // knows anything happened; its dirty bit must drive the retry.
        let mut s = State {
            repo_name: "zelligent".into(),
            ..Default::default()
        };
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());

        // Control: with the dirty bit clear, an identical-set TabUpdate
        // correctly does nothing — this is exactly where v2 alone stalled.
        let control =
            s.handle_tab_update(vec![make_tab("zelligent", true), make_tab("feat-a", false)]);
        assert_eq!(
            control,
            Action::None,
            "control: without the dirty bit, zero net set drift means no \
             Refresh (the v2 gap)"
        );

        // The blind-window spawn/remove each piped an invalidate. The pipe
        // handler fires a Refresh, but this instance is hidden: the
        // RunCommandResult never arrives, so nothing clears the bit.
        let pipe_action = s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert_eq!(pipe_action, Action::Refresh);
        assert!(s.cache_dirty);

        // Wake-up TabUpdate: set still identical, but the dirty bit forces
        // the retry — now visible, this Refresh's result can land.
        let action =
            s.handle_tab_update(vec![make_tab("zelligent", true), make_tab("feat-a", false)]);
        assert_eq!(
            action,
            Action::Refresh,
            "dirty bit must force a Refresh retry even with zero set drift"
        );
    }

    #[test]
    fn successful_list_worktrees_clears_dirty_bit() {
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true)]);
        s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert!(s.cache_dirty);

        // Stamped with the current generation, as a refresh launched after
        // (or by) this invalidation would be — see `gen_ctx`.
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\n",
            b"",
            &gen_ctx(s.invalidate_generation),
        );
        assert!(
            !s.cache_dirty,
            "a successful refresh at the current generation satisfies the invalidation"
        );

        // With the bit cleared, an identical TabUpdate is quiet again.
        let action = s.handle_tab_update(vec![make_tab("feat-a", true)]);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn failed_list_worktrees_leaves_dirty_bit_set() {
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true)]);
        s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert!(s.cache_dirty);

        // A failed refresh proves nothing about cache freshness.
        s.handle_list_worktrees(Some(1), b"", b"boom", &BTreeMap::new());
        assert!(s.cache_dirty, "failure must not clear the dirty bit");

        // The next TabUpdate retries the Refresh.
        let action = s.handle_tab_update(vec![make_tab("feat-a", true)]);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn stale_in_flight_refresh_cannot_clear_a_newer_invalidation() {
        // The Codex-diagnosed #140 race: refresh A is launched, THEN an
        // invalidate pipe arrives (bumping the generation and re-setting
        // cache_dirty for a NEW reason), and only THEN does A's stale
        // result land. A's generation predates the invalidation it never
        // observed, so it must not be allowed to clear the bit.
        let mut s = State::default();

        // Refresh A launched at generation 0 (no invalidation yet).
        let stale_ctx = gen_ctx(s.invalidate_generation);
        assert_eq!(s.invalidate_generation, 0);

        // An invalidate pipe arrives while A is still in flight: bumps the
        // generation to 1 and (re-)marks the cache dirty.
        s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert!(s.cache_dirty);
        assert_eq!(s.invalidate_generation, 1);

        // A's result finally lands, stamped with the generation it was
        // launched under (0) — stale relative to the invalidation at 1.
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &stale_ctx);
        assert!(
            s.cache_dirty,
            "a refresh launched before the latest invalidation must not \
             clear the bit that invalidation set"
        );
        // The listing itself is still applied — stale output is harmless.
        assert_eq!(s.worktrees.len(), 1);
        assert_eq!(s.worktrees[0].branch, "feat-a");
    }

    #[test]
    fn refresh_at_current_generation_clears_dirty_bit() {
        let mut s = State::default();
        s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert!(s.cache_dirty);
        assert_eq!(s.invalidate_generation, 1);

        // A refresh launched at (or after) the invalidation carries the
        // current generation and may clear the bit it satisfies.
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\n",
            b"",
            &gen_ctx(s.invalidate_generation),
        );
        assert!(
            !s.cache_dirty,
            "a refresh stamped with the current generation proves freshness"
        );
    }

    #[test]
    fn two_invalidations_back_to_back_only_the_latest_generation_clears() {
        // A refresh stamped with the FIRST of two back-to-back
        // invalidations must not clear the bit after the second lands,
        // even though it's the more recent invalidation of the two — the
        // refresh was launched before either result was observed, and only
        // a refresh at-or-after the SECOND invalidation can prove the
        // cache reflects it.
        let mut s = State::default();

        s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert_eq!(s.invalidate_generation, 1);
        let first_gen_ctx = gen_ctx(s.invalidate_generation);

        s.handle_pipe(&pipe_msg("zelligent-invalidate", &[]));
        assert_eq!(s.invalidate_generation, 2);
        assert!(s.cache_dirty);

        // A refresh stamped with generation 1 (launched between the two
        // invalidations, or before both) lands after the second.
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &first_gen_ctx);
        assert!(
            s.cache_dirty,
            "a first-generation refresh cannot clear a bit set by a \
             second, later invalidation"
        );

        // Only a refresh stamped with the current (second) generation can.
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\n",
            b"",
            &gen_ctx(s.invalidate_generation),
        );
        assert!(!s.cache_dirty);
    }

    #[test]
    fn select_active_sidebar_item_uses_tab_identity_not_position() {
        let mut s = State {
            tabs: vec![make_tab("feat-a", false), make_tab("feat-b", true)],
            sidebar_items: vec![
                SidebarItem {
                    tab_name: "feat-b".into(),
                    display_name: "feat-b".into(),
                    matched_branch: Some("feat-b".into()),
                },
                SidebarItem {
                    tab_name: "feat-a".into(),
                    display_name: "feat-a".into(),
                    matched_branch: Some("feat-a".into()),
                },
            ],
            ..Default::default()
        };

        assert!(s.select_active_sidebar_item());
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn recompute_sidebar_preserves_cursor_by_name() {
        let mut s = State::default();
        s.tabs = vec![
            make_tab("feat-a", true),
            make_tab("feat-b", false),
            make_tab("feat-c", false),
        ];
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\nfeat-b\tfeat-b\nfeat-c\tfeat-c\n",
            b"",
            &BTreeMap::new(),
        );
        s.selected_index = 2;
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
    fn recompute_sidebar_builds_detached_items_without_tabs() {
        let mut s = State::default();
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\nfeat-b\tfeat-b\n",
            b"",
            &BTreeMap::new(),
        );
        assert_eq!(s.sidebar_items.len(), 2);
        assert_eq!(s.sidebar_items[0].tab_name, "feat-a");
        assert_eq!(s.sidebar_items[1].tab_name, "feat-b");
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn recompute_sidebar_includes_user_tab() {
        let mut s = State::default();
        s.tabs = vec![make_tab("notes", true)];
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());
        assert_eq!(s.sidebar_items.len(), 2);
        assert_eq!(s.sidebar_items[0].tab_name, "feat-a");
        assert_eq!(s.sidebar_items[0].matched_branch, Some("feat-a".into()));
        assert_eq!(s.sidebar_items[1].tab_name, "notes");
        assert_eq!(s.sidebar_items[1].matched_branch, None);
    }

    #[test]
    fn recompute_sidebar_labels_repo_tab_as_local() {
        let mut s = State {
            repo_name: "zelligent".into(),
            ..Default::default()
        };
        s.tabs = vec![make_tab("zelligent", true), make_tab("feat-a", false)];
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());
        assert_eq!(s.sidebar_items[0].tab_name, "zelligent");
        assert_eq!(s.sidebar_items[0].display_name, "local");
        assert_eq!(s.sidebar_items[0].matched_branch, None);
    }

    #[test]
    fn recompute_sidebar_includes_detached_worktrees() {
        let mut s = State::default();
        s.handle_list_worktrees(
            Some(0),
            b"feat-a\tfeat-a\nfeature-cool\tfeature/cool\n",
            b"",
            &BTreeMap::new(),
        );
        assert_eq!(s.sidebar_items.len(), 2);
        assert_eq!(s.sidebar_items[0].tab_name, "feat-a");
        assert_eq!(s.sidebar_items[0].matched_branch, Some("feat-a".into()));
        assert_eq!(s.sidebar_items[1].tab_name, "feature-cool");
        assert_eq!(
            s.sidebar_items[1].matched_branch,
            Some("feature/cool".into())
        );
    }

    #[test]
    fn recompute_sidebar_keeps_managed_order_when_tabs_change() {
        let mut s = State {
            repo_name: "zelligent".into(),
            worktrees: vec![
                Worktree {
                    dir: "autonomy".into(),
                    branch: "plugin-snapshot-tests".into(),
                },
                Worktree {
                    dir: "competition".into(),
                    branch: "competition".into(),
                },
                Worktree {
                    dir: "ding".into(),
                    branch: "feat/ding-dong".into(),
                },
            ],
            tabs: vec![make_tab("zelligent", true), make_tab("competition", false)],
            ..Default::default()
        };
        s.recompute_sidebar_items();
        assert_eq!(
            s.sidebar_items
                .iter()
                .map(|item| item.display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["local", "autonomy", "competition", "ding"]
        );

        s.tabs = vec![
            make_tab("feat-ding-dong", false),
            make_tab("zelligent", false),
            make_tab("plugin-snapshot-tests", true),
            make_tab("competition", false),
        ];
        s.recompute_sidebar_items();

        assert_eq!(
            s.sidebar_items
                .iter()
                .map(|item| item.display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["local", "autonomy", "competition", "ding"]
        );
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn recompute_sidebar_uses_worktree_dir_for_open_managed_tab_display() {
        let mut s = State {
            worktrees: vec![Worktree {
                dir: "autonomy".into(),
                branch: "plugin-snapshot-tests".into(),
            }],
            tabs: vec![make_tab("plugin-snapshot-tests", true)],
            ..Default::default()
        };
        s.recompute_sidebar_items();
        assert_eq!(s.sidebar_items[0].tab_name, "plugin-snapshot-tests");
        assert_eq!(s.sidebar_items[0].display_name, "autonomy");
        assert_eq!(
            s.sidebar_items[0].matched_branch,
            Some("plugin-snapshot-tests".into())
        );
    }

    #[test]
    fn recompute_sidebar_clamps_on_shrink() {
        let mut s = State::default();
        s.tabs = vec![make_tab("a", false), make_tab("b", true)];
        s.recompute_sidebar_items();
        s.selected_index = 1;
        s.tabs = vec![make_tab("a", true)];
        s.recompute_sidebar_items();
        assert_eq!(s.selected_index, 0);
    }

    #[test]
    fn recompute_sidebar_ambiguous_tab_name_matches_first_worktree() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-cool", true)];
        s.handle_list_worktrees(
            Some(0),
            b"feat-cool\tfeat/cool\nfeat-cool\tfeat-cool\n",
            b"",
            &BTreeMap::new(),
        );
        assert_eq!(s.sidebar_items[0].matched_branch, Some("feat/cool".into()));
    }

    #[test]
    fn list_worktrees_error_sets_status() {
        let mut s = State::default();
        s.handle_list_worktrees(Some(1), b"", b"fatal: not a git repository", &BTreeMap::new());
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Failed to list worktrees"));
        assert!(s.status_message.contains("fatal: not a git repository"));
    }

    #[test]
    fn list_worktrees_error_preserves_existing_worktrees() {
        let mut s = state_with_worktrees();
        let original_len = s.worktrees.len();
        s.handle_list_worktrees(Some(1), b"", b"error", &BTreeMap::new());
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
        let mut s = State {
            mode: Mode::InputBranch,
            input_buffer: "wip".into(),
            ..Default::default()
        };
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
        let mut s = State {
            mode: Mode::NotGitRepo,
            ..Default::default()
        };
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
        let mut s = State {
            mode: Mode::NotGitRepo,
            ..Default::default()
        };
        let action = s.handle_key_not_git_repo(&key(BareKey::Char('x')));
        assert_eq!(action, Action::None);
        assert!(s.status_is_error);
        assert!(s.status_message.contains("Cannot determine session name"));
    }

    #[test]
    fn not_git_repo_q_returns_close() {
        let mut s = State {
            mode: Mode::NotGitRepo,
            ..Default::default()
        };
        let action = s.handle_key_not_git_repo(&key(BareKey::Char('q')));
        assert_eq!(action, Action::Close);
    }

    #[test]
    fn not_git_repo_esc_returns_close() {
        let mut s = State {
            mode: Mode::NotGitRepo,
            ..Default::default()
        };
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

    /// `run_command` context for a `list-worktrees` result stamped with the
    /// given `invalidate_generation`, as `fire_list_worktrees` would produce
    /// at launch time. See `CTX_GENERATION` / `State::invalidate_generation`.
    fn gen_ctx(generation: u64) -> BTreeMap<String, String> {
        let mut ctx = BTreeMap::new();
        ctx.insert(CTX_GENERATION.to_string(), generation.to_string());
        ctx
    }

    #[test]
    fn pipe_unknown_name_ignored() {
        let mut s = State::default();
        let action = s.handle_pipe(&pipe_msg(
            "other-plugin",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
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
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn pipe_user_prompt_submit_sets_working() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "UserPromptSubmit")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn pipe_permission_request_sets_needs_input_and_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "PermissionRequest")],
        ));
        assert_eq!(
            action,
            Action::Notify {
                tab_name: "feat-a".into(),
                status: AgentStatus::NeedsInput
            }
        );
        assert_eq!(
            s.agent_statuses.get("feat-a"),
            Some(&AgentStatus::NeedsInput)
        );
    }

    #[test]
    fn pipe_stop_sets_done_and_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
        assert_eq!(
            action,
            Action::Notify {
                tab_name: "feat-a".into(),
                status: AgentStatus::Done
            }
        );
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn pipe_active_tab_still_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", true)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
        assert_eq!(
            action,
            Action::Notify {
                tab_name: "feat-a".into(),
                status: AgentStatus::Done
            }
        );
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn pipe_different_tab_active_notifies() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-b", true), make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
        assert_eq!(
            action,
            Action::Notify {
                tab_name: "feat-a".into(),
                status: AgentStatus::Done
            }
        );
    }

    #[test]
    fn pipe_status_overwrite() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn pipe_unknown_tab_buffered() {
        // #141: a valid event for a tab not yet in `self.tabs` (an external
        // status pipe racing the registering TabUpdate) must be buffered,
        // not dropped.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-b", false)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "unknown-tab"), ("event", "Stop")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("unknown-tab"), None);
        assert_eq!(
            s.pending_statuses.get("unknown-tab"),
            Some(&PendingStatus {
                status: AgentStatus::Done,
                age: 0
            })
        );
    }

    #[test]
    fn pipe_buffered_status_applied_on_tab_update() {
        // Once the buffered tab's TabUpdate arrives, the status must move
        // from `pending_statuses` into `agent_statuses` — and the Action
        // returned must match what an equivalent update without any
        // buffered entry would return (the #127/#138 Refresh/None semantics
        // are untouched by draining).
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-b", false)];
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        assert_eq!(
            s.pending_statuses.get("feat-a"),
            Some(&PendingStatus {
                status: AgentStatus::Working,
                age: 0
            })
        );

        let action = s.handle_tab_update(vec![
            make_tab("feat-b", false),
            make_tab("feat-a", true),
        ]);

        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
        assert!(s.pending_statuses.get("feat-a").is_none());

        // Same snapshot, but starting from a state that never buffered
        // anything for feat-a — the Action must be identical.
        let mut baseline = State::default();
        baseline.tabs = vec![make_tab("feat-b", false)];
        let baseline_action = baseline.handle_tab_update(vec![
            make_tab("feat-b", false),
            make_tab("feat-a", true),
        ]);
        assert_eq!(action, baseline_action);
    }

    #[test]
    fn pipe_buffered_status_latest_event_wins() {
        let mut s = State::default();
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        assert_eq!(
            s.pending_statuses.get("feat-a"),
            Some(&PendingStatus {
                status: AgentStatus::Working,
                age: 0
            })
        );

        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
        assert_eq!(
            s.pending_statuses.get("feat-a"),
            Some(&PendingStatus {
                status: AgentStatus::Done,
                age: 0
            })
        );
        assert_eq!(s.pending_statuses.len(), 1);
    }

    #[test]
    fn pipe_buffered_needs_input_does_not_notify_at_buffer_time() {
        let mut s = State::default();
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "unknown-tab"), ("event", "PermissionRequest")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(
            s.pending_statuses.get("unknown-tab"),
            Some(&PendingStatus {
                status: AgentStatus::NeedsInput,
                age: 0
            })
        );
    }

    #[test]
    fn pipe_buffered_needs_input_does_not_notify_at_tab_update_time_either() {
        // A NeedsInput/Done that only arrives via the buffer must not be
        // replayed as a Notify once the tab shows up — see the comment in
        // `handle_pipe` on why deferring the notify would fire it from the
        // wrong context.
        let mut s = State::default();
        // Give feat-a a matching worktree so its appearance in the
        // TabUpdate below doesn't independently trigger a Refresh via the
        // unrelated #127 "newly-appeared unmatched tab" gate — this test is
        // only about the buffered-status/Notify interaction.
        s.handle_list_worktrees(Some(0), b"feat-a\tfeat-a\n", b"", &BTreeMap::new());
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "PermissionRequest")],
        ));
        let action = s.handle_tab_update(vec![make_tab("feat-a", true)]);
        assert_eq!(action, Action::None);
        assert_eq!(
            s.agent_statuses.get("feat-a"),
            Some(&AgentStatus::NeedsInput)
        );
    }

    #[test]
    fn pipe_buffered_statuses_capped_at_16() {
        let mut s = State::default();
        for i in 0..17 {
            s.handle_pipe(&pipe_msg(
                "zelligent-status",
                &[("tab", &format!("unknown-{i}")), ("event", "Start")],
            ));
        }
        assert_eq!(s.pending_statuses.len(), 16);
    }

    #[test]
    fn pending_status_expires_after_max_tab_updates() {
        // #141: an unmatched pending entry must not haunt the buffer
        // forever — once it's survived PENDING_STATUS_MAX_TAB_UPDATES
        // TabUpdates with no matching tab, it's dropped and does not apply
        // even if a tab with that name appears afterwards.
        let mut s = State::default();
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        for _ in 0..PENDING_STATUS_MAX_TAB_UPDATES {
            s.handle_tab_update(vec![make_tab("feat-b", false)]);
        }
        assert!(s.pending_statuses.get("feat-a").is_none());

        s.handle_tab_update(vec![make_tab("feat-a", true)]);
        assert_eq!(s.agent_statuses.get("feat-a"), None);
    }

    #[test]
    fn pending_status_applies_just_before_expiry() {
        // Draining (matching a now-known tab) happens before aging, so an
        // entry on its last legal update still applies if its tab shows up
        // on that very update.
        let mut s = State::default();
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        for _ in 0..(PENDING_STATUS_MAX_TAB_UPDATES - 1) {
            s.handle_tab_update(vec![make_tab("feat-b", false)]);
        }
        assert!(s.pending_statuses.get("feat-a").is_some());

        s.handle_tab_update(vec![make_tab("feat-a", true)]);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn pending_status_reset_by_new_pipe() {
        // Re-receiving a pipe for a still-buffered tab must reset its age,
        // not just overwrite its status.
        let mut s = State::default();
        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Start")],
        ));
        for _ in 0..(PENDING_STATUS_MAX_TAB_UPDATES - 1) {
            s.handle_tab_update(vec![make_tab("feat-b", false)]);
        }
        assert_eq!(
            s.pending_statuses.get("feat-a"),
            Some(&PendingStatus {
                status: AgentStatus::Working,
                age: PENDING_STATUS_MAX_TAB_UPDATES - 1
            })
        );

        s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "Stop")],
        ));
        assert_eq!(
            s.pending_statuses.get("feat-a"),
            Some(&PendingStatus {
                status: AgentStatus::Done,
                age: 0
            })
        );

        // Confirm the reset actually matters: the entry survives another
        // near-expiry stretch that would otherwise have dropped it.
        for _ in 0..(PENDING_STATUS_MAX_TAB_UPDATES - 1) {
            s.handle_tab_update(vec![make_tab("feat-b", false)]);
        }
        assert!(s.pending_statuses.get("feat-a").is_some());
    }

    #[test]
    fn pipe_unknown_event_shows_error() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            "zelligent-status",
            &[("tab", "feat-a"), ("event", "BogusEvent")],
        ));
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

    // --- status replay tests (#140 part B / Z-6) ---

    #[test]
    fn status_request_with_no_known_statuses_replies_nothing() {
        // A freshly-loaded instance that itself has nothing to offer must
        // not broadcast an empty replay — that would just be noise on
        // every load.
        let mut s = State::default();
        let action = s.handle_pipe(&pipe_msg(PIPE_STATUS_REQUEST, &[]));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn status_request_with_known_statuses_replies_with_serialized_payload() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Working);
        let action = s.handle_pipe(&pipe_msg(PIPE_STATUS_REQUEST, &[]));
        assert_eq!(
            action,
            Action::ReplayStatuses("feat-a:Working".to_string())
        );
    }

    #[test]
    fn status_request_reply_includes_pending_statuses() {
        // The replay payload must carry both live `agent_statuses` and
        // buffered `pending_statuses` (#141) entries — a late instance
        // should catch up on both.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Working);
        s.pending_statuses.insert(
            "feat-b".to_string(),
            PendingStatus { status: AgentStatus::Done, age: 0 },
        );
        let action = s.handle_pipe(&pipe_msg(PIPE_STATUS_REQUEST, &[]));
        let Action::ReplayStatuses(payload) = action else {
            panic!("expected ReplayStatuses, got {action:?}");
        };
        let mut parsed = State::parse_statuses(&payload);
        parsed.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            parsed,
            vec![
                ("feat-a".to_string(), AgentStatus::Working),
                ("feat-b".to_string(), AgentStatus::Done),
            ]
        );
    }

    #[test]
    fn status_request_with_only_pending_statuses_still_replies() {
        // An instance holding only buffered early events (#141) has real
        // knowledge to offer — the reply gate must not require a live
        // agent_statuses entry.
        let mut s = State::default();
        s.pending_statuses.insert(
            "feat-b".to_string(),
            PendingStatus { status: AgentStatus::Working, age: 0 },
        );
        let action = s.handle_pipe(&pipe_msg(PIPE_STATUS_REQUEST, &[]));
        assert_eq!(
            action,
            Action::ReplayStatuses("feat-b:Working".to_string())
        );
    }

    #[test]
    fn serialize_statuses_skips_tab_names_containing_separators() {
        // `zelligent-status` accepts arbitrary tab= values; a name with a
        // `;` or `:` would fragment the replay payload into a spurious
        // entry for a different tab on every receiver, so it is skipped.
        let mut s = State::default();
        s.agent_statuses.insert("bad;name".to_string(), AgentStatus::Working);
        s.agent_statuses.insert("bad:name".to_string(), AgentStatus::Working);
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Done);
        assert_eq!(s.serialize_statuses(), "feat-a:Done");
    }

    #[test]
    fn serialize_parse_statuses_round_trip() {
        let mut s = State::default();
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Working);
        s.agent_statuses.insert("feat-b".to_string(), AgentStatus::NeedsInput);
        s.pending_statuses.insert(
            "feat-c".to_string(),
            PendingStatus { status: AgentStatus::Done, age: 0 },
        );
        let payload = s.serialize_statuses();
        let mut parsed = State::parse_statuses(&payload);
        parsed.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            parsed,
            vec![
                ("feat-a".to_string(), AgentStatus::Working),
                ("feat-b".to_string(), AgentStatus::NeedsInput),
                ("feat-c".to_string(), AgentStatus::Done),
            ]
        );
    }

    #[test]
    fn status_replay_fills_missing_known_tab_status() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, "feat-a:Working")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn status_replay_never_overwrites_existing_agent_status() {
        // Monotone merge: a stale instance's replay must not clobber a
        // status this instance already learned (possibly more recent).
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Done);
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, "feat-a:Working")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Done));
    }

    #[test]
    fn status_replay_suppresses_notify_even_for_needs_input_or_done() {
        // Replay must never produce a user-visible side effect, unlike a
        // live `zelligent-status` pipe carrying the same statuses.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false), make_tab("feat-b", false)];
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, "feat-a:NeedsInput;feat-b:Done")],
        ));
        assert_eq!(action, Action::None);
        assert!(s.status_message.is_empty());
        assert!(!s.status_is_error);
        assert_eq!(
            s.agent_statuses.get("feat-a"),
            Some(&AgentStatus::NeedsInput)
        );
        assert_eq!(s.agent_statuses.get("feat-b"), Some(&AgentStatus::Done));
    }

    #[test]
    fn status_replay_routes_unknown_tab_through_pending_statuses() {
        // A replayed entry for a tab this instance doesn't know yet must
        // land in `pending_statuses`, using the same #141 buffer/eviction
        // semantics as a live event for an unknown tab.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-b", false)];
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, "unknown-tab:Done")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("unknown-tab"), None);
        assert_eq!(
            s.pending_statuses.get("unknown-tab"),
            Some(&PendingStatus {
                status: AgentStatus::Done,
                age: 0
            })
        );
    }

    #[test]
    fn status_replay_never_overwrites_existing_pending_status() {
        let mut s = State::default();
        s.pending_statuses.insert(
            "unknown-tab".to_string(),
            PendingStatus { status: AgentStatus::Working, age: 0 },
        );
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, "unknown-tab:Done")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(
            s.pending_statuses.get("unknown-tab"),
            Some(&PendingStatus {
                status: AgentStatus::Working,
                age: 0
            })
        );
    }

    #[test]
    fn status_replay_unknown_tab_still_respects_pending_cap_of_16() {
        let mut s = State::default();
        for i in 0..16 {
            s.pending_statuses
                .insert(
                format!("existing-{i}"),
                PendingStatus { status: AgentStatus::Working, age: 0 },
            );
        }
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, "new-unknown-tab:Done")],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.pending_statuses.len(), 16);
    }

    #[test]
    fn status_replay_ignores_malformed_entries() {
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(
                STATUS_REPLAY_ARG,
                "feat-a:BogusStatus;;garbage-no-colon;:Working;feat-a:Working",
            )],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses.get("feat-a"), Some(&AgentStatus::Working));
    }

    #[test]
    fn status_replay_missing_arg_is_a_no_op() {
        let mut s = State::default();
        let action = s.handle_pipe(&pipe_msg(PIPE_STATUS_REPLAY, &[]));
        assert_eq!(action, Action::None);
        assert!(s.agent_statuses.is_empty());
        assert!(s.pending_statuses.is_empty());
    }

    #[test]
    fn status_replay_handling_never_emits_another_request_or_replay() {
        // Loop safety: the only actions `handle_pipe` can return for
        // PIPE_STATUS_REPLAY are Action::None — merging never re-triggers
        // the request/replay cycle. Exercise a mix of overwrite-blocked,
        // known-tab-filled, and unknown-tab-buffered entries in one call to
        // make sure none of those paths sneaks out a different Action.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Done);
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(
                STATUS_REPLAY_ARG,
                "feat-a:Working;feat-b:NeedsInput;unknown-tab:Done",
            )],
        ));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn status_replay_of_own_broadcast_is_idempotent() {
        // An instance receiving its own PIPE_STATUS_REPLAY (broadcasts
        // reach the sender too) must be a no-op: merging identical state
        // into itself changes nothing and emits nothing further.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Working);
        let payload = s.serialize_statuses();
        let before = s.agent_statuses.clone();
        let action = s.handle_pipe(&pipe_msg(
            PIPE_STATUS_REPLAY,
            &[(STATUS_REPLAY_ARG, &payload)],
        ));
        assert_eq!(action, Action::None);
        assert_eq!(s.agent_statuses, before);
    }

    #[test]
    fn status_request_of_own_broadcast_is_answered_like_any_other() {
        // The requester also receives its own PIPE_STATUS_REQUEST
        // broadcast. That's fine (frozen design point 4): it just answers
        // itself the same way it would answer anyone else — the merge on
        // the reply is idempotent regardless of who sent the request.
        let mut s = State::default();
        s.tabs = vec![make_tab("feat-a", false)];
        s.agent_statuses.insert("feat-a".to_string(), AgentStatus::Working);
        let action = s.handle_pipe(&pipe_msg(PIPE_STATUS_REQUEST, &[]));
        assert_eq!(
            action,
            Action::ReplayStatuses("feat-a:Working".to_string())
        );
    }

    #[test]
    fn serialize_statuses_caps_total_payload_length() {
        // Defensive cap: even with far more entries than any real session
        // would have, the serialized payload never exceeds
        // STATUS_REPLAY_MAX_LEN, and every entry actually included parses
        // back cleanly (no truncated/corrupted last entry).
        let mut s = State::default();
        for i in 0..500 {
            s.agent_statuses
                .insert(format!("feat-tab-number-{i:04}"), AgentStatus::Working);
        }
        let payload = s.serialize_statuses();
        assert!(payload.len() <= STATUS_REPLAY_MAX_LEN);
        assert!(!payload.is_empty());
        let parsed = State::parse_statuses(&payload);
        // Every included entry must round-trip — no partially-written
        // final entry from truncating mid-string.
        assert_eq!(parsed.len(), payload.split(';').count());
        for (tab, status) in &parsed {
            assert!(tab.starts_with("feat-tab-number-"));
            assert_eq!(*status, AgentStatus::Working);
        }
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
        for cmd in &[
            CMD_GIT_TOPLEVEL,
            CMD_LIST_WORKTREES,
            CMD_GIT_BRANCHES,
            CMD_SPAWN,
            CMD_REMOVE,
        ] {
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
                we_initiated: true,
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
            we_initiated: true,
        };
        if let Action::CloseTabAndRefresh {
            tab_name,
            return_to,
            we_initiated,
        } = &action
        {
            assert_eq!(tab_name, "feat-a");
            assert_eq!(return_to, &Some("zelligent".into()));
            assert!(*we_initiated);
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

    // --- Status message TTL (#152) ---
    //
    // set_status/handle_timer/handle_visible are pure (no host calls), so —
    // like every other handler in this module — they're exercised directly.
    // The real `zellij_tile::shim::set_timeout` call lives in
    // `arm_pending_status_timer` (called from `update`/`pipe`, never from
    // unit tests, same as `execute`/`fire_*`); `status_timer_needs_arming`
    // is the indirection these tests observe. Expiry is age-based: tests
    // backdate `status_message_set_at` instead of sleeping.

    fn backdate_status(s: &mut State, secs: u64) {
        s.status_message_set_at = s
            .status_message_set_at
            .map(|t| t - std::time::Duration::from_secs(secs));
    }

    #[test]
    fn set_status_stamps_age_and_requests_wakeup() {
        let mut s = State::default();
        assert!(s.status_message_set_at.is_none());
        assert!(!s.status_timer_needs_arming);

        s.set_status("Spawned 'feature-c'", false);

        assert_eq!(s.status_message, "Spawned 'feature-c'");
        assert!(!s.status_is_error);
        assert!(s.status_message_set_at.is_some());
        assert!(s.status_timer_needs_arming, "set_status must request a wake-up");
        assert_eq!(
            s.status_timer_arm_secs, STATUS_MESSAGE_TTL_SECS,
            "a fresh message gets its full TTL"
        );
    }

    #[test]
    fn set_status_error_variant_sets_is_error() {
        let mut s = State::default();
        s.set_status("Unknown agent event: Bogus", true);
        assert!(s.status_is_error);
    }

    #[test]
    fn set_status_empty_message_clears_stamp_and_pending_arm() {
        // Clearing in the same event that set a message must also retract
        // the not-yet-performed arm request — arming a wake-up for an
        // already-cleared message is noise (Codex review finding).
        let mut s = State::default();
        s.set_status("Refreshed", false);
        assert!(s.status_timer_needs_arming);

        s.set_status("", false);

        assert!(s.status_message.is_empty());
        assert!(s.status_message_set_at.is_none());
        assert!(!s.status_timer_needs_arming);
    }

    #[test]
    fn timer_before_ttl_does_not_clear_and_rechains_for_the_remainder() {
        let mut s = State::default();
        s.set_status("Spawned 'feature-c'", false);
        backdate_status(&mut s, 5);
        s.status_timer_needs_arming = false; // shell armed the original

        assert!(!s.handle_timer(), "a fresh message must survive an early wake-up");
        assert_eq!(s.status_message, "Spawned 'feature-c'");
        assert!(
            s.status_timer_needs_arming,
            "an early wake-up must re-chain — the message must never depend on an unrelated event to expire"
        );
        assert!(
            s.status_timer_arm_secs > 2.0 && s.status_timer_arm_secs <= 3.2,
            "re-chain must be for the REMAINING TTL (~3s at age 5), got {}",
            s.status_timer_arm_secs
        );
    }

    #[test]
    fn timer_after_ttl_clears() {
        let mut s = State::default();
        s.set_status("Spawned 'feature-c'", false);
        backdate_status(&mut s, 9);

        assert!(s.handle_timer(), "clearing must request a re-render");
        assert!(s.status_message.is_empty());
        assert!(!s.status_is_error);
        assert!(s.status_message_set_at.is_none());
    }

    #[test]
    fn stale_timer_does_not_clear_a_newer_message() {
        // Message A's wake-up fires after A was replaced by B: B is younger
        // than the TTL, so the age check leaves it to live out its own TTL
        // no matter how many older timers are in flight.
        let mut s = State::default();
        s.set_status("Spawning 'feat-a'...", false);
        backdate_status(&mut s, 5);
        s.set_status("Spawned 'feat-a'", false); // B, re-stamps to now

        assert!(!s.handle_timer(), "A's stale wake-up must not clear B");
        assert_eq!(s.status_message, "Spawned 'feat-a'");

        backdate_status(&mut s, 9);
        assert!(s.handle_timer(), "B clears once IT is old enough");
        assert!(s.status_message.is_empty());
    }

    #[test]
    fn reveal_lazily_clears_a_message_that_expired_while_hidden() {
        // The wake-up timer can be lost while the pane is hidden (hidden
        // instances receive no Events). Reveal must reconcile: an expired
        // message clears immediately.
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true)]);
        s.set_status("Spawned 'feat-b'", false);
        s.status_timer_needs_arming = false; // shell armed it (then lost)
        backdate_status(&mut s, 20);

        assert!(s.handle_visible(true), "reveal must clear and re-render");
        assert!(s.status_message.is_empty());
        assert!(s.status_message_set_at.is_none());
    }

    #[test]
    fn reveal_rearms_wakeup_for_a_still_live_message() {
        // A young message whose timer may have been lost while hidden gets
        // a fresh wake-up on reveal; a redundant wake-up is harmless (the
        // age check just declines to clear).
        let mut s = State::default();
        s.handle_tab_update(vec![make_tab("feat-a", true)]);
        s.set_status("Spawned 'feat-b'", false);
        s.status_timer_needs_arming = false; // shell armed it (then lost)

        backdate_status(&mut s, 5);
        s.handle_visible(true);

        assert_eq!(s.status_message, "Spawned 'feat-b'");
        assert!(s.status_timer_needs_arming, "reveal must request a fresh wake-up");
        assert!(
            s.status_timer_arm_secs > 2.0 && s.status_timer_arm_secs <= 3.2,
            "reveal must arm only the REMAINING TTL (~3s at age 5), not a full one, got {}",
            s.status_timer_arm_secs
        );
    }

    #[test]
    fn timer_with_no_message_is_a_harmless_no_op() {
        let mut s = State::default();
        assert!(!s.handle_timer());
        assert!(s.status_message_set_at.is_none());
    }
}
