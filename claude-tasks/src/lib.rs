//! Claude Tasks plugin for totui.
//!
//! Syncs Claude Code's native task lists into totui in real-time.
//!
//! This plugin watches a selected Claude tasklist folder for file changes
//! and syncs tasks to totui todos using the Plugin trait interface.

#![allow(non_local_definitions)]

pub mod claude_task;
pub mod commands;
pub mod config;
pub mod discovery;
pub mod errors;
pub mod guidance;
pub mod hierarchy;
pub mod staleness;
pub mod state;
pub mod sync;
pub mod watcher;

use abi_stable::{
    export_root_module,
    prefix_type::PrefixTypeTrait,
    sabi_trait::TD_Opaque,
    std_types::{RBox, RHashMap, ROption, RResult, RString, RVec},
};
use config::{format_tasklist_display, generate_tasklist_options, load_config};
use guidance::{clear_guidance, create_empty_tasklist_guidance, create_no_tasklist_guidance};
use state::{new_shared_state, GuidanceState, SharedSyncState, SyncEvent};
use std::sync::mpsc;
use std::sync::Mutex;
use totui_plugin_interface::{
    FfiCommand, FfiConfigField, FfiConfigSchema, FfiConfigType, FfiConfigValue, FfiEvent,
    FfiEventType, FfiHookResponse, FfiTodoItem, HostApi_TO, Plugin, PluginModule, PluginModule_Ref,
    Plugin_TO,
};
use watcher::WatcherHandle;

// ============================================================================
// Module export for abi_stable
// ============================================================================

#[export_root_module]
fn get_library() -> PluginModule_Ref {
    PluginModule { create_plugin }.leak_into_prefix()
}

extern "C" fn create_plugin() -> Plugin_TO<'static, RBox<()>> {
    Plugin_TO::from_value(ClaudeTasksPlugin::new(), TD_Opaque)
}

// ============================================================================
// Plugin implementation
// ============================================================================

/// The Claude Tasks plugin.
///
/// Watches Claude Code task files and syncs them to totui in real-time.
pub struct ClaudeTasksPlugin {
    /// Receiver for events from watcher thread
    rx: Mutex<Option<mpsc::Receiver<SyncEvent>>>,
    /// Sender for watcher thread (kept to allow thread communication)
    tx: Mutex<Option<mpsc::Sender<SyncEvent>>>,
    /// Shared state for sync engine
    state: SharedSyncState,
    /// Handle to the file watcher thread
    watcher_handle: Mutex<Option<WatcherHandle>>,
}

impl ClaudeTasksPlugin {
    /// Create a new ClaudeTasksPlugin instance.
    pub fn new() -> Self {
        Self {
            rx: Mutex::new(None),
            tx: Mutex::new(None),
            state: new_shared_state(),
            watcher_handle: Mutex::new(None),
        }
    }
}

impl Default for ClaudeTasksPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeTasksPlugin {
    /// Process all pending sync events using local state tracking.
    ///
    /// Unlike process_sync_events, this doesn't need HostApi - it uses
    /// local known_tasks tracking to determine create vs update.
    fn process_sync_events_local(&self) -> Vec<FfiCommand> {
        let mut commands = Vec::new();

        // Get tasklist info from state
        let (tasklist_path, tasklist_id) = {
            let state = self.state.lock().unwrap();
            match &state.selected_tasklist {
                Some(path) => {
                    let id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    (path.clone(), id)
                }
                None => return commands,
            }
        };

        // Drain all pending events from channel
        let rx_guard = self.rx.lock().unwrap();
        let Some(rx) = rx_guard.as_ref() else {
            return commands;
        };

        // Collect events first (to minimize lock time on state)
        let mut events = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(event) => events.push(event),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    eprintln!("claude-tasks: Watcher channel disconnected");
                    break;
                }
            }
        }
        drop(rx_guard);

        // Record update if we received any events
        if !events.is_empty() {
            let mut state = self.state.lock().unwrap();
            state.staleness_tracker.record_update();
        }

        // Only clear guidance when real tasks arrive (FileChanged events)
        // InitialScan and FileRemoved don't indicate new tasks arriving
        let has_file_changed_events = events
            .iter()
            .any(|e| matches!(e, SyncEvent::FileChanged(_)));
        let should_clear_guidance = {
            let state = self.state.lock().unwrap();
            state.is_guidance_shown() && has_file_changed_events
        };

        if should_clear_guidance {
            // Prepend guidance clear commands
            let clear_cmds = clear_guidance();
            commands.extend(clear_cmds);

            // Update state
            let mut state = self.state.lock().unwrap();
            state.clear_guidance();
            eprintln!("claude-tasks: Clearing guidance - FileChanged events indicate real tasks arrived");
        }

        // Process events
        for event in events {
            match event {
                SyncEvent::InitialScan => {
                    // Get alias from config
                    let alias = {
                        let state = self.state.lock().unwrap();
                        state.config.get_alias(&tasklist_id).map(|s| s.to_string())
                    };

                    let (cmds, task_ids) =
                        sync::process_initial_scan_local(&tasklist_path, &tasklist_id, alias.as_deref());
                    commands.extend(cmds);

                    // Mark all tasks as known
                    let mut state = self.state.lock().unwrap();
                    state.clear_known_tasks();
                    for id in task_ids {
                        state.mark_task_known(&id);
                    }
                }
                SyncEvent::FileChanged(path) => {
                    let is_known = {
                        let state = self.state.lock().unwrap();
                        // Extract task_id from path to check if known
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|id| state.is_task_known(id))
                            .unwrap_or(false)
                    };

                    if let Some((cmds, task_id)) =
                        sync::process_file_change_local(&path, &tasklist_id, is_known)
                    {
                        commands.extend(cmds);

                        // Mark as known if new
                        if !is_known {
                            let mut state = self.state.lock().unwrap();
                            state.mark_task_known(&task_id);
                        }
                    }
                }
                SyncEvent::FileRemoved(path) => {
                    if let Some((cmd, task_id)) =
                        sync::process_file_removal_local(&path, &tasklist_id)
                    {
                        commands.push(cmd);

                        // Forget the task
                        let mut state = self.state.lock().unwrap();
                        state.forget_task(&task_id);
                    }
                }
            }
        }

        commands
    }
}

impl std::fmt::Debug for ClaudeTasksPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeTasksPlugin")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Plugin for ClaudeTasksPlugin {
    fn name(&self) -> RString {
        "claude-tasks".into()
    }

    fn version(&self) -> RString {
        "0.1.0".into()
    }

    fn min_interface_version(&self) -> RString {
        "0.1.0".into()
    }

    fn generate(&self, _input: RString) -> RResult<RVec<FfiTodoItem>, RString> {
        // This plugin uses execute_with_host() with file watching instead of generate()
        RResult::ROk(RVec::new())
    }

    fn config_schema(&self) -> FfiConfigSchema {
        // Load config to resolve aliases for display
        let plugin_config = load_config();

        // Generate options from discovered tasklists
        let options = generate_tasklist_options(&plugin_config);

        // Convert to RVec<RString> - use UUID as the value, display string shown to user
        // Format: "display_string|uuid" so totui can parse both
        let option_strings: RVec<RString> = options
            .iter()
            .map(|(display, uuid)| RString::from(format!("{}|{}", display, uuid)))
            .collect();

        // Create the tasklist field
        let tasklist_field = FfiConfigField {
            name: RString::from("tasklist"),
            field_type: FfiConfigType::Select,
            required: false, // Will auto-select first if not specified
            default: ROption::RNone,
            description: ROption::RSome(RString::from("Select which Claude tasklist to sync")),
            options: option_strings,
        };

        FfiConfigSchema {
            fields: vec![tasklist_field].into_iter().collect(),
            config_required: false,
        }
    }

    fn execute_with_host(
        &self,
        _input: RString,
        _host: HostApi_TO<'_, RBox<()>>,
    ) -> RResult<RVec<FfiCommand>, RString> {
        // Process sync events using local state (same as on_event)
        // This allows manual plugin invocation to also work
        let commands = self.process_sync_events_local();
        RResult::ROk(commands.into_iter().collect())
    }

    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>) {
        // Load plugin configuration (global + local merged)
        let plugin_config = load_config();

        // Discover available tasklists
        let tasklists = discovery::discover_tasklists();

        if tasklists.is_empty() {
            eprintln!("claude-tasks: No tasklists found - showing setup guidance");
            let mut state = self.state.lock().unwrap();
            state.pending_commands = create_no_tasklist_guidance();
            state.set_guidance(GuidanceState::NoTasklists);
            return;
        }

        // Check if user selected a specific tasklist via config
        let selected = if let Some(FfiConfigValue::String(uuid)) =
            config.get(&RString::from("tasklist"))
        {
            let uuid_str = uuid.as_str();
            // Find tasklist with matching UUID
            tasklists
                .iter()
                .find(|t| t.id == uuid_str)
                .cloned()
                .unwrap_or_else(|| {
                    eprintln!(
                        "claude-tasks: Configured tasklist {} not found, falling back to first",
                        uuid_str
                    );
                    tasklists[0].clone()
                })
        } else {
            // Fall back to first tasklist if not specified
            tasklists[0].clone()
        };

        let display_name = format_tasklist_display(&selected.id, &plugin_config);
        eprintln!(
            "claude-tasks: Watching tasklist: {} ({} tasks)",
            display_name, selected.task_count
        );

        // Store selected tasklist path and config in state
        // Initialize staleness tracker with configured threshold
        {
            let mut state = self.state.lock().unwrap();
            state.selected_tasklist = Some(selected.path.clone());
            state.staleness_tracker =
                staleness::StalenessTracker::new(plugin_config.staleness_threshold());
            state.config = plugin_config;
        }

        // Create mpsc channel for watcher -> plugin communication
        let (tx, rx) = mpsc::channel::<SyncEvent>();

        // Store receiver
        *self.rx.lock().unwrap() = Some(rx);

        // Clone tx for InitialScan send (before moving to watcher)
        let tx_for_initial = tx.clone();

        // Store sender
        *self.tx.lock().unwrap() = Some(tx.clone());

        // Start file watcher
        match watcher::start_watcher(selected.path.clone(), tx) {
            Ok(handle) => {
                *self.watcher_handle.lock().unwrap() = Some(handle);
                eprintln!(
                    "claude-tasks: Watcher started for {}",
                    selected.path.display()
                );

                // Check if tasklist has any tasks - if empty, show waiting guidance
                let tasks = discovery::scan_tasks_directory(&selected.path);
                if tasks.is_empty() {
                    let mut state = self.state.lock().unwrap();
                    state.pending_commands = create_empty_tasklist_guidance(&display_name);
                    state.set_guidance(GuidanceState::EmptyTasklist);
                    eprintln!("claude-tasks: Empty tasklist - showing waiting guidance");
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                eprintln!("claude-tasks: Failed to start watcher: {}", error_msg);
                let mut state = self.state.lock().unwrap();
                state.pending_commands = guidance::create_error_guidance(
                    "CLAUDE TASKS - Watcher Failed",
                    &error_msg,
                    "Restart totui to retry",
                );
                state.set_guidance(GuidanceState::Error);
                return;
            }
        }

        // Send InitialScan event to trigger first sync
        if let Err(e) = tx_for_initial.send(SyncEvent::InitialScan) {
            eprintln!("claude-tasks: Failed to send InitialScan: {}", e);
        }
    }

    fn subscribed_events(&self) -> RVec<FfiEventType> {
        // Subscribe to OnLoad events to check for watcher updates
        let mut events = RVec::new();
        events.push(FfiEventType::OnLoad);
        events
    }

    fn on_event(&self, event: FfiEvent) -> RResult<FfiHookResponse, RString> {
        if let FfiEvent::OnLoad { .. } = event {
            // Check for pending guidance commands first
            let pending = {
                let mut state = self.state.lock().unwrap();
                state.take_pending_commands()
            };

            if !pending.is_empty() {
                eprintln!(
                    "claude-tasks: Returning {} pending guidance commands",
                    pending.len()
                );
                return RResult::ROk(FfiHookResponse {
                    commands: pending.into_iter().collect(),
                });
            }

            // Process pending sync events and return commands
            let mut commands = self.process_sync_events_local();

            // Check staleness and update header if needed
            let staleness_info = {
                let state = self.state.lock().unwrap();
                if let Some(ref tasklist_path) = state.selected_tasklist {
                    let tasklist_id = tasklist_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let alias = state.config.get_alias(&tasklist_id).map(|s| s.to_string());
                    let staleness = state.staleness_tracker.format_staleness();
                    let is_tracking = state.staleness_tracker.is_tracking();
                    Some((tasklist_id, alias, staleness, is_tracking))
                } else {
                    None
                }
            };

            // Add header update if stale, or if transitioning from stale to fresh
            // Only update header if we're actively tracking (have received at least one update)
            if let Some((tasklist_id, alias, staleness, is_tracking)) = staleness_info {
                if is_tracking && (staleness.is_some() || !commands.is_empty()) {
                    // Update header with or without staleness indicator
                    commands.push(commands::update_header_command(
                        &tasklist_id,
                        alias.as_deref(),
                        staleness.as_deref(),
                    ));
                }
            }

            if !commands.is_empty() {
                eprintln!(
                    "claude-tasks: on_event returning {} commands",
                    commands.len()
                );
            }

            return RResult::ROk(FfiHookResponse {
                commands: commands.into_iter().collect(),
            });
        }

        RResult::ROk(FfiHookResponse::default())
    }
}

#[cfg(test)]
mod tests {
    use super::state::SyncEvent;
    use std::path::PathBuf;

    #[test]
    fn test_file_changed_event_detection() {
        // FileChanged should be detected
        let events_with_change = vec![
            SyncEvent::InitialScan,
            SyncEvent::FileChanged(PathBuf::from("/test/1.json")),
        ];
        let has_file_changed = events_with_change
            .iter()
            .any(|e| matches!(e, SyncEvent::FileChanged(_)));
        assert!(has_file_changed, "Should detect FileChanged event");

        // InitialScan only should not trigger
        let events_initial_only = vec![SyncEvent::InitialScan];
        let has_file_changed = events_initial_only
            .iter()
            .any(|e| matches!(e, SyncEvent::FileChanged(_)));
        assert!(
            !has_file_changed,
            "InitialScan alone should not trigger clearing"
        );

        // FileRemoved only should not trigger
        let events_removed_only = vec![SyncEvent::FileRemoved(PathBuf::from("/test/1.json"))];
        let has_file_changed = events_removed_only
            .iter()
            .any(|e| matches!(e, SyncEvent::FileChanged(_)));
        assert!(
            !has_file_changed,
            "FileRemoved alone should not trigger clearing"
        );
    }
}
