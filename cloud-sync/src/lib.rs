//! Cloud Sync plugin for totui.
//!
//! Syncs totui projects and todos to the cloud for cross-device access.
//! Designed as the foundation for mobile app integration.
//!
//! ## Configuration
//!
//! The plugin is configured through totui's plugin config UI:
//! - `api_url` - Cloud API endpoint (e.g., "https://api.totui.app")
//! - `api_key` - Authentication bearer token
//! - `project_name` - Project name to sync (auto-creates if it doesn't exist)
//! - `sync_interval` - Seconds between automatic syncs (default: 30)
//!
//! ## Cloud API Contract
//!
//! The plugin communicates with any REST API that implements:
//! - `GET    /api/v1/auth/verify`            - Verify API key
//! - `GET    /api/v1/projects`               - List projects
//! - `POST   /api/v1/projects`               - Create project
//! - `GET    /api/v1/projects/{id}/todos`     - List todos
//! - `POST   /api/v1/projects/{id}/sync`      - Bidirectional sync
//! - `PUT    /api/v1/todos/{id}`              - Update single todo
//! - `DELETE /api/v1/todos/{id}`              - Delete todo

#![allow(non_local_definitions)]

pub mod api;
pub mod models;
pub mod sync;

use abi_stable::{
    export_root_module,
    prefix_type::PrefixTypeTrait,
    sabi_trait::TD_Opaque,
    std_types::{RBox, RHashMap, ROption, RResult, RString, RVec},
};
use api::CloudClient;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use sync::{LocalTodoSnapshot, SyncEngine, SyncMessage};
use totui_plugin_interface::{
    FfiCommand, FfiConfigField, FfiConfigSchema, FfiConfigType, FfiConfigValue, FfiEvent,
    FfiEventType, FfiHookResponse, FfiTodoItem, FfiTodoQuery, FfiTodoState, HostApi_TO, Plugin,
    PluginModule, PluginModule_Ref, Plugin_TO, UpdateNotifier,
};

// ============================================================================
// Module export for abi_stable
// ============================================================================

#[export_root_module]
fn get_library() -> PluginModule_Ref {
    PluginModule { create_plugin }.leak_into_prefix()
}

extern "C" fn create_plugin() -> Plugin_TO<'static, RBox<()>> {
    Plugin_TO::from_value(CloudSyncPlugin::new(), TD_Opaque)
}

// ============================================================================
// Plugin state
// ============================================================================

/// Shared notifier for waking the host.
type SharedNotifier = Arc<Mutex<Option<UpdateNotifier>>>;

/// Configuration values loaded from the host.
#[derive(Debug, Clone, Default)]
struct PluginConfig {
    api_url: Option<String>,
    api_key: Option<String>,
    project_name: Option<String>,
    sync_interval: u64,
}

impl PluginConfig {
    fn is_configured(&self) -> bool {
        self.api_url.is_some() && self.api_key.is_some() && self.project_name.is_some()
    }
}

/// The Cloud Sync plugin.
///
/// Syncs todos bidirectionally between totui and a cloud API.
pub struct CloudSyncPlugin {
    /// Sync engine (manages state, performs sync operations)
    engine: Arc<SyncEngine>,
    /// Plugin configuration
    config: Mutex<PluginConfig>,
    /// Channel to send messages to the background sync thread
    sync_tx: Mutex<Option<mpsc::Sender<SyncMessage>>>,
    /// Pending commands to return to the host on next event
    pending_commands: Mutex<Vec<FfiCommand>>,
    /// Notifier callback for waking the host
    notifier: SharedNotifier,
    /// Whether initial sync has been performed
    initialized: Mutex<bool>,
}

impl CloudSyncPlugin {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(SyncEngine::new()),
            config: Mutex::new(PluginConfig {
                sync_interval: 30,
                ..Default::default()
            }),
            sync_tx: Mutex::new(None),
            pending_commands: Mutex::new(Vec::new()),
            notifier: Arc::new(Mutex::new(None)),
            initialized: Mutex::new(false),
        }
    }

    /// Notify the host that we have updates ready.
    fn notify_host(&self) {
        if let Some(notifier) = *self.notifier.lock().unwrap() {
            (notifier.func)();
        }
    }

    /// Start the background sync thread.
    fn start_sync_thread(&self) {
        let config = self.config.lock().unwrap().clone();
        if !config.is_configured() {
            return;
        }

        // Stop existing thread if any
        if let Some(tx) = self.sync_tx.lock().unwrap().take() {
            let _ = tx.send(SyncMessage::Shutdown);
        }

        let (tx, rx) = mpsc::channel::<SyncMessage>();
        *self.sync_tx.lock().unwrap() = Some(tx);

        let engine = Arc::clone(&self.engine);
        let notifier = Arc::clone(&self.notifier);
        let api_url = config.api_url.unwrap();
        let api_key = config.api_key.unwrap();
        let project_name = config.project_name.unwrap();
        let interval = Duration::from_secs(config.sync_interval);

        thread::spawn(move || {
            let client = CloudClient::new(&api_url, &api_key);

            // Ensure project exists
            let project_id = match engine.ensure_project(&client, &project_name) {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!("cloud-sync: Failed to ensure project: {}", e);
                    return;
                }
            };

            loop {
                // Wait for either a sync message or timeout
                match rx.recv_timeout(interval) {
                    Ok(SyncMessage::SyncNow) => {
                        // Triggered sync - pull latest state
                        match engine.pull(&client, &project_id) {
                            Ok(commands) => {
                                if !commands.is_empty() {
                                    // Store commands - they'll be picked up by on_event
                                    // We can't directly access pending_commands from here,
                                    // so we notify the host to poll us
                                    tracing::info!(
                                        "cloud-sync: Pull returned {} commands",
                                        commands.len()
                                    );
                                }
                                // Notify host to call on_event
                                if let Some(n) = *notifier.lock().unwrap() {
                                    (n.func)();
                                }
                            }
                            Err(e) => {
                                tracing::warn!("cloud-sync: Sync failed: {}", e);
                            }
                        }
                    }
                    Ok(SyncMessage::Shutdown) => {
                        tracing::info!("cloud-sync: Sync thread shutting down");
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Periodic sync - just notify host to trigger on_event
                        if let Some(n) = *notifier.lock().unwrap() {
                            (n.func)();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        tracing::info!("cloud-sync: Channel disconnected, stopping sync thread");
                        break;
                    }
                }
            }
        });
    }

    /// Perform initial pull from cloud and return commands.
    fn initial_pull(&self) -> Vec<FfiCommand> {
        let config = self.config.lock().unwrap().clone();
        if !config.is_configured() {
            return self.create_unconfigured_guidance();
        }

        let api_url = config.api_url.unwrap();
        let api_key = config.api_key.unwrap();
        let project_name = config.project_name.unwrap();

        let client = CloudClient::new(&api_url, &api_key);

        // Verify auth first
        match client.verify_auth() {
            Ok(true) => {}
            Ok(false) => {
                return self.create_error_guidance(
                    "Authentication failed",
                    "Check your API key in the plugin configuration",
                );
            }
            Err(e) => {
                return self.create_error_guidance(
                    &format!("Connection failed: {}", e),
                    "Check your API URL and network connection",
                );
            }
        }

        // Ensure project exists
        let project_id = match self.engine.ensure_project(&client, &project_name) {
            Ok(id) => id,
            Err(e) => {
                return self.create_error_guidance(
                    &format!("Failed to set up project: {}", e),
                    "Check your API connection",
                );
            }
        };

        // Pull remote todos
        match self.engine.pull(&client, &project_id) {
            Ok(commands) => {
                *self.initialized.lock().unwrap() = true;
                // Start background sync thread
                self.start_sync_thread();
                commands
            }
            Err(e) => self.create_error_guidance(
                &format!("Initial sync failed: {}", e),
                "The plugin will retry on next load",
            ),
        }
    }

    /// Create guidance commands for unconfigured state.
    fn create_unconfigured_guidance(&self) -> Vec<FfiCommand> {
        let mut commands = Vec::new();

        commands.push(FfiCommand::CreateTodo {
            content: RString::from("CLOUD SYNC: Not configured"),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-guidance-header")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        });

        commands.push(FfiCommand::CreateTodo {
            content: RString::from("Configure API URL, API key, and project name in plugin settings"),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-guidance-hint")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        });

        commands
    }

    /// Create guidance commands for error state.
    fn create_error_guidance(&self, error: &str, hint: &str) -> Vec<FfiCommand> {
        let mut commands = Vec::new();

        commands.push(FfiCommand::CreateTodo {
            content: RString::from(format!("CLOUD SYNC ERROR: {}", error)),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-error-header")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        });

        commands.push(FfiCommand::CreateTodo {
            content: RString::from(hint.to_string()),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-error-hint")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        });

        commands
    }

    /// Perform a push sync: read local todos and push them to cloud.
    fn push_sync(&self, host: &HostApi_TO<'_, RBox<()>>) -> Vec<FfiCommand> {
        let config = self.config.lock().unwrap().clone();
        if !config.is_configured() {
            return Vec::new();
        }

        let api_url = config.api_url.unwrap();
        let api_key = config.api_key.unwrap();

        let project_id = {
            let state = self.engine.state.lock().unwrap();
            match state.project_id.clone() {
                Some(id) => id,
                None => return Vec::new(),
            }
        };

        // Collect local todos from the host
        let local_todos = self.collect_local_todos(host);

        let client = CloudClient::new(&api_url, &api_key);
        match self.engine.sync(&client, &project_id, local_todos) {
            Ok(commands) => commands,
            Err(e) => {
                tracing::warn!("cloud-sync: Push sync failed: {}", e);
                Vec::new()
            }
        }
    }

    /// Collect todos from the host.
    fn collect_local_todos(&self, host: &HostApi_TO<'_, RBox<()>>) -> Vec<LocalTodoSnapshot> {
        let todos = host.query_todos(FfiTodoQuery::default());
        todos
            .iter()
            .map(|item| sync::ffi_todo_to_snapshot(item))
            .collect()
    }
}

impl Default for CloudSyncPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CloudSyncPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudSyncPlugin")
            .field("config", &self.config)
            .field("initialized", &self.initialized)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Plugin trait implementation
// ============================================================================

impl Plugin for CloudSyncPlugin {
    fn name(&self) -> RString {
        "cloud-sync".into()
    }

    fn version(&self) -> RString {
        "0.1.0".into()
    }

    fn min_interface_version(&self) -> RString {
        "0.3.0".into()
    }

    fn generate(&self, _input: RString) -> RResult<RVec<FfiTodoItem>, RString> {
        // This plugin uses execute_with_host() and events instead of generate()
        RResult::ROk(RVec::new())
    }

    fn config_schema(&self) -> FfiConfigSchema {
        let api_url_field = FfiConfigField {
            name: RString::from("api_url"),
            field_type: FfiConfigType::String,
            required: true,
            default: ROption::RNone,
            description: ROption::RSome(RString::from(
                "Cloud API endpoint URL (e.g., https://api.totui.app)",
            )),
            options: RVec::new(),
        };

        let api_key_field = FfiConfigField {
            name: RString::from("api_key"),
            field_type: FfiConfigType::String,
            required: true,
            default: ROption::RNone,
            description: ROption::RSome(RString::from(
                "API authentication key",
            )),
            options: RVec::new(),
        };

        let project_name_field = FfiConfigField {
            name: RString::from("project_name"),
            field_type: FfiConfigType::String,
            required: true,
            default: ROption::RSome(FfiConfigValue::String(RString::from("My Project"))),
            description: ROption::RSome(RString::from(
                "Project name to sync (will be created if it doesn't exist)",
            )),
            options: RVec::new(),
        };

        let sync_interval_field = FfiConfigField {
            name: RString::from("sync_interval"),
            field_type: FfiConfigType::String,
            required: false,
            default: ROption::RSome(FfiConfigValue::String(RString::from("30"))),
            description: ROption::RSome(RString::from(
                "Seconds between automatic syncs (default: 30)",
            )),
            options: RVec::new(),
        };

        FfiConfigSchema {
            fields: vec![
                api_url_field,
                api_key_field,
                project_name_field,
                sync_interval_field,
            ]
            .into_iter()
            .collect(),
            config_required: true,
        }
    }

    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>) {
        let mut plugin_config = self.config.lock().unwrap();

        // Extract config values
        if let Some(FfiConfigValue::String(url)) = config.get(&RString::from("api_url")) {
            let url_str = url.to_string();
            if !url_str.is_empty() {
                plugin_config.api_url = Some(url_str);
            }
        }

        if let Some(FfiConfigValue::String(key)) = config.get(&RString::from("api_key")) {
            let key_str = key.to_string();
            if !key_str.is_empty() {
                plugin_config.api_key = Some(key_str);
            }
        }

        if let Some(FfiConfigValue::String(name)) = config.get(&RString::from("project_name")) {
            let name_str = name.to_string();
            if !name_str.is_empty() {
                plugin_config.project_name = Some(name_str);
            }
        }

        if let Some(FfiConfigValue::String(interval)) =
            config.get(&RString::from("sync_interval"))
        {
            if let Ok(secs) = interval.parse::<u64>() {
                if secs >= 5 {
                    plugin_config.sync_interval = secs;
                }
            }
        }

        // If fully configured, trigger initial sync
        if plugin_config.is_configured() {
            drop(plugin_config);
            // Queue initial pull - will be executed on next on_event
            let commands = self.initial_pull();
            if !commands.is_empty() {
                self.pending_commands.lock().unwrap().extend(commands);
                self.notify_host();
            }
        }
    }

    fn execute_with_host(
        &self,
        _input: RString,
        host: HostApi_TO<'_, RBox<()>>,
    ) -> RResult<RVec<FfiCommand>, RString> {
        // Manual invocation: do a full push+pull sync
        let commands = self.push_sync(&host);
        RResult::ROk(commands.into_iter().collect())
    }

    fn subscribed_events(&self) -> RVec<FfiEventType> {
        let mut events = RVec::new();
        events.push(FfiEventType::OnLoad);
        events
    }

    fn on_event(&self, event: FfiEvent) -> RResult<FfiHookResponse, RString> {
        if let FfiEvent::OnLoad { .. } = event {
            // Return any pending commands (from initial pull or background sync)
            let pending = {
                let mut cmds = self.pending_commands.lock().unwrap();
                std::mem::take(&mut *cmds)
            };

            if !pending.is_empty() {
                return RResult::ROk(FfiHookResponse {
                    commands: pending.into_iter().collect(),
                });
            }
        }

        RResult::ROk(FfiHookResponse::default())
    }

    fn set_notifier(&self, notifier: UpdateNotifier) {
        *self.notifier.lock().unwrap() = Some(notifier);
    }
}

impl Drop for CloudSyncPlugin {
    fn drop(&mut self) {
        // Signal sync thread to stop
        if let Some(tx) = self.sync_tx.lock().unwrap().take() {
            let _ = tx.send(SyncMessage::Shutdown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_defaults() {
        let config = PluginConfig::default();
        assert!(!config.is_configured());
        assert_eq!(config.sync_interval, 0);
    }

    #[test]
    fn test_plugin_config_configured() {
        let config = PluginConfig {
            api_url: Some("https://api.example.com".to_string()),
            api_key: Some("test-key".to_string()),
            project_name: Some("Test".to_string()),
            sync_interval: 30,
        };
        assert!(config.is_configured());
    }

    #[test]
    fn test_plugin_name() {
        let plugin = CloudSyncPlugin::new();
        assert_eq!(plugin.name().as_str(), "cloud-sync");
    }

    #[test]
    fn test_unconfigured_guidance() {
        let plugin = CloudSyncPlugin::new();
        let commands = plugin.create_unconfigured_guidance();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_error_guidance() {
        let plugin = CloudSyncPlugin::new();
        let commands = plugin.create_error_guidance("test error", "try again");
        assert_eq!(commands.len(), 2);
    }
}
