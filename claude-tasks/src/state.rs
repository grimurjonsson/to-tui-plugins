//! Shared state types for file watcher and sync engine.
//!
//! SyncState holds the current plugin state (selected tasklist, header todo ID).
//! SyncEvent represents events from the file watcher thread.
//! GuidanceState tracks what guidance UI is currently displayed.

use crate::config::PluginConfig;
use crate::staleness::StalenessTracker;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use totui_plugin_interface::FfiCommand;

/// Events from the file watcher thread.
///
/// These are sent via mpsc channel from the watcher thread to the main plugin.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// A task file was created or modified
    FileChanged(PathBuf),
    /// A task file was removed
    FileRemoved(PathBuf),
    /// Initial scan complete (sent after watching starts)
    InitialScan,
}

/// Current guidance state for UX flow.
///
/// Determines what guidance todos (if any) should be displayed.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum GuidanceState {
    /// No guidance shown - normal operation
    #[default]
    None,
    /// No tasklists exist at all
    NoTasklists,
    /// Tasklist exists but has no tasks yet
    EmptyTasklist,
    /// Error occurred with recovery guidance shown
    Error,
}

/// Shared state for the sync engine.
///
/// Wrapped in Mutex for thread-safe access between watcher thread and plugin callbacks.
#[derive(Debug, Default)]
pub struct SyncState {
    /// Path to the currently selected tasklist folder (e.g., ~/.claude/tasks/{uuid}/)
    pub selected_tasklist: Option<PathBuf>,
    /// ID of the header todo for this tasklist (for updates)
    pub header_todo_id: Option<String>,
    /// Set of task IDs that have been synced to totui.
    /// Used to determine if we should create vs update without querying HostApi.
    pub known_tasks: HashSet<String>,
    /// Plugin configuration (aliases, staleness threshold)
    pub config: PluginConfig,
    /// Staleness tracker for detecting stale tasklists
    pub staleness_tracker: StalenessTracker,
    /// Current guidance state for UX flow
    pub guidance_state: GuidanceState,
    /// Whether guidance todos are currently displayed
    pub guidance_shown: bool,
    /// Commands to return on next on_event call.
    /// Used for guidance commands created during on_config_loaded.
    pub pending_commands: Vec<FfiCommand>,
}

impl SyncState {
    /// Mark a task as known (synced to totui)
    pub fn mark_task_known(&mut self, task_id: &str) {
        self.known_tasks.insert(task_id.to_string());
    }

    /// Check if a task is known (already synced)
    pub fn is_task_known(&self, task_id: &str) -> bool {
        self.known_tasks.contains(task_id)
    }

    /// Remove a task from known set (after deletion)
    pub fn forget_task(&mut self, task_id: &str) {
        self.known_tasks.remove(task_id);
    }

    /// Clear all known tasks (for resync)
    pub fn clear_known_tasks(&mut self) {
        self.known_tasks.clear();
    }

    /// Set guidance state and mark guidance as shown.
    pub fn set_guidance(&mut self, state: GuidanceState) {
        self.guidance_state = state;
        self.guidance_shown = true;
    }

    /// Clear guidance state and mark as not shown.
    pub fn clear_guidance(&mut self) {
        self.guidance_state = GuidanceState::None;
        self.guidance_shown = false;
    }

    /// Check if guidance is currently shown.
    pub fn is_guidance_shown(&self) -> bool {
        self.guidance_shown
    }

    /// Take and clear pending commands.
    /// Returns the pending commands and clears the internal list.
    pub fn take_pending_commands(&mut self) -> Vec<FfiCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Check if there are pending commands to return.
    pub fn has_pending_commands(&self) -> bool {
        !self.pending_commands.is_empty()
    }
}

/// Thread-safe wrapper for SyncState.
pub type SharedSyncState = Mutex<SyncState>;

/// Create a new shared sync state with default values.
pub fn new_shared_state() -> SharedSyncState {
    Mutex::new(SyncState::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_event_file_changed() {
        let event = SyncEvent::FileChanged(PathBuf::from("/home/user/.claude/tasks/abc/1.json"));
        match event {
            SyncEvent::FileChanged(path) => {
                assert!(path.to_string_lossy().contains("1.json"));
            }
            _ => panic!("Expected FileChanged"),
        }
    }

    #[test]
    fn test_sync_state_default() {
        let state = SyncState::default();
        assert!(state.selected_tasklist.is_none());
        assert!(state.header_todo_id.is_none());
    }

    #[test]
    fn test_shared_state_mutex() {
        let shared = new_shared_state();
        {
            let mut state = shared.lock().unwrap();
            state.selected_tasklist = Some(PathBuf::from("/test/path"));
        }
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.selected_tasklist, Some(PathBuf::from("/test/path")));
        }
    }

    #[test]
    fn test_known_tasks_tracking() {
        let mut state = SyncState::default();
        assert!(!state.is_task_known("1"));

        state.mark_task_known("1");
        assert!(state.is_task_known("1"));

        state.forget_task("1");
        assert!(!state.is_task_known("1"));
    }

    #[test]
    fn test_clear_known_tasks() {
        let mut state = SyncState::default();
        state.mark_task_known("1");
        state.mark_task_known("2");

        state.clear_known_tasks();
        assert!(!state.is_task_known("1"));
        assert!(!state.is_task_known("2"));
    }

    #[test]
    fn test_guidance_state_default() {
        let state = GuidanceState::default();
        assert_eq!(state, GuidanceState::None);
    }

    #[test]
    fn test_sync_state_guidance_defaults() {
        let state = SyncState::default();
        assert_eq!(state.guidance_state, GuidanceState::None);
        assert!(!state.guidance_shown);
    }

    #[test]
    fn test_set_guidance() {
        let mut state = SyncState::default();
        state.set_guidance(GuidanceState::NoTasklists);

        assert_eq!(state.guidance_state, GuidanceState::NoTasklists);
        assert!(state.guidance_shown);
    }

    #[test]
    fn test_clear_guidance() {
        let mut state = SyncState::default();
        state.set_guidance(GuidanceState::Error);

        state.clear_guidance();

        assert_eq!(state.guidance_state, GuidanceState::None);
        assert!(!state.guidance_shown);
    }

    #[test]
    fn test_is_guidance_shown() {
        let mut state = SyncState::default();
        assert!(!state.is_guidance_shown());

        state.set_guidance(GuidanceState::EmptyTasklist);
        assert!(state.is_guidance_shown());

        state.clear_guidance();
        assert!(!state.is_guidance_shown());
    }

    #[test]
    fn test_guidance_state_variants() {
        // Test all variants exist and can be compared
        let none = GuidanceState::None;
        let no_tasklists = GuidanceState::NoTasklists;
        let empty = GuidanceState::EmptyTasklist;
        let error = GuidanceState::Error;

        assert_ne!(none, no_tasklists);
        assert_ne!(no_tasklists, empty);
        assert_ne!(empty, error);
        assert_ne!(error, none);
    }

    #[test]
    fn test_pending_commands_default() {
        let state = SyncState::default();
        assert!(state.pending_commands.is_empty());
        assert!(!state.has_pending_commands());
    }

    #[test]
    fn test_pending_commands_take() {
        use abi_stable::std_types::RString;
        use totui_plugin_interface::FfiCommand;

        let mut state = SyncState::default();

        // Add a command
        state.pending_commands.push(FfiCommand::DeleteTodo {
            id: RString::from("test-id"),
        });
        assert!(state.has_pending_commands());

        // Take commands - should return and clear
        let cmds = state.take_pending_commands();
        assert_eq!(cmds.len(), 1);
        assert!(!state.has_pending_commands());
        assert!(state.pending_commands.is_empty());
    }

    #[test]
    fn test_pending_commands_take_empty() {
        let mut state = SyncState::default();

        // Take from empty - should return empty vec
        let cmds = state.take_pending_commands();
        assert!(cmds.is_empty());
    }
}
