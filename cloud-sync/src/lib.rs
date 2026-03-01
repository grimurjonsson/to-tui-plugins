//! Cloud Sync plugin for totui.
//!
//! Syncs totui projects and todos to the cloud for cross-device access.
//! Designed as the foundation for mobile app integration.
//!
//! ## Configuration
//!
//! The plugin is configured through totui's plugin config UI:
//! - `api_url` - Cloud API endpoint (e.g., "https://api.totui.app")
//! - `project_name` - Project name to sync (auto-creates if it doesn't exist)
//! - `sync_interval` - Seconds between automatic syncs (default: 30)
//!
//! ## Authentication
//!
//! Uses OAuth 2.0 Device Authorization (RFC 8628). When the plugin starts
//! without a stored token, it initiates a device code flow:
//! 1. Plugin requests a device code from the server
//! 2. User opens the provided URL in a browser to sign in
//! 3. Plugin polls until auth completes, then stores the token locally
//!
//! ## Cloud API Contract
//!
//! The plugin communicates with any REST API that implements:
//! - `POST   /api/v1/auth/device`              - Initiate device code flow
//! - `POST   /api/v1/auth/device/token`         - Poll for device token
//! - `GET    /api/v1/auth/verify`               - Verify auth token
//! - `GET    /api/v1/projects`                  - List projects
//! - `POST   /api/v1/projects`                  - Create project
//! - `GET    /api/v1/projects/{id}/todos`        - List todos
//! - `POST   /api/v1/projects/{id}/sync`         - Bidirectional sync
//! - `PUT    /api/v1/todos/{id}`                 - Update single todo
//! - `DELETE /api/v1/todos/{id}`                 - Delete todo

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
use models::DeviceCodeResponse;
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
    project_name: Option<String>,
    sync_interval: u64,
}

impl PluginConfig {
    fn is_configured(&self) -> bool {
        self.api_url.is_some() && self.project_name.is_some()
    }
}

/// The Cloud Sync plugin.
///
/// Syncs todos bidirectionally between totui and a cloud API.
/// Authentication is handled automatically via device code flow.
pub struct CloudSyncPlugin {
    /// Sync engine (manages state, performs sync operations)
    engine: Arc<SyncEngine>,
    /// Plugin configuration
    config: Mutex<PluginConfig>,
    /// Channel to send messages to the background sync thread
    sync_tx: Mutex<Option<mpsc::Sender<SyncMessage>>>,
    /// Pending commands to return to the host on next event (shared with thread)
    pending_commands: Arc<Mutex<Vec<FfiCommand>>>,
    /// Notifier callback for waking the host
    notifier: SharedNotifier,
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
            pending_commands: Arc::new(Mutex::new(Vec::new())),
            notifier: Arc::new(Mutex::new(None)),
        }
    }

    /// Notify the host that we have updates ready.
    fn notify_host(&self) {
        notify(&self.notifier);
    }

    /// Start the background thread that handles auth + sync.
    ///
    /// The thread runs in two phases:
    /// 1. **Auth phase**: If no stored token, initiates device code flow and
    ///    polls until the user authenticates via browser.
    /// 2. **Sync phase**: Periodically pulls remote changes and applies them.
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
        let pending = Arc::clone(&self.pending_commands);
        let api_url = config.api_url.unwrap();
        let project_name = config.project_name.unwrap();
        let sync_interval = Duration::from_secs(config.sync_interval);

        thread::spawn(move || {
            sync_thread_main(rx, engine, pending, notifier, api_url, project_name, sync_interval);
        });
    }

    /// Perform a push sync: read local todos and push them to cloud.
    fn push_sync(&self, host: &HostApi_TO<'_, RBox<()>>) -> Vec<FfiCommand> {
        let config = self.config.lock().unwrap().clone();
        if !config.is_configured() {
            return Vec::new();
        }

        let api_url = config.api_url.unwrap();

        // Get token from stored state
        let token = {
            let state = self.engine.state.lock().unwrap();
            match state.auth_token.clone() {
                Some(t) => t,
                None => {
                    return create_error_guidance(
                        "Not signed in yet",
                        "Complete sign-in in browser first",
                    );
                }
            }
        };

        let project_id = {
            let state = self.engine.state.lock().unwrap();
            match state.project_id.clone() {
                Some(id) => id,
                None => return Vec::new(),
            }
        };

        let local_todos = self.collect_local_todos(host);
        let client = CloudClient::new(&api_url, &token);

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
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Background thread: auth + sync
// ============================================================================

/// Main loop for the background sync thread.
///
/// Handles two phases:
/// 1. Authentication via device code flow (if no stored token)
/// 2. Periodic sync with the cloud
fn sync_thread_main(
    rx: mpsc::Receiver<SyncMessage>,
    engine: Arc<SyncEngine>,
    pending: Arc<Mutex<Vec<FfiCommand>>>,
    notifier: SharedNotifier,
    api_url: String,
    project_name: String,
    sync_interval: Duration,
) {
    // ---- Phase 1: Authenticate ----
    let token = {
        let stored = engine.state.lock().unwrap().auth_token.clone();
        if let Some(token) = stored {
            token
        } else {
            match run_device_auth(&rx, &engine, &pending, &notifier, &api_url) {
                Some(token) => token,
                None => return, // Shutdown or unrecoverable error
            }
        }
    };

    // ---- Phase 2: Initial pull ----
    let client = CloudClient::new(&api_url, &token);

    let project_id = match engine.ensure_project(&client, &project_name) {
        Ok(id) => id,
        Err(e) => {
            if matches!(e, api::ApiClientError::Unauthorized) {
                // Token expired — clear it so next restart re-auths
                let mut state = engine.state.lock().unwrap();
                state.auth_token = None;
                let _ = models::save_sync_state(&state);
                drop(state);
                queue_commands(
                    &pending,
                    &notifier,
                    create_error_guidance(
                        "Session expired",
                        "Restart the plugin to sign in again",
                    ),
                );
            } else {
                queue_commands(
                    &pending,
                    &notifier,
                    create_error_guidance(
                        &format!("Failed to set up project: {}", e),
                        "Check your connection",
                    ),
                );
            }
            return;
        }
    };

    match engine.pull(&client, &project_id) {
        Ok(commands) => queue_commands(&pending, &notifier, commands),
        Err(e) => {
            queue_commands(
                &pending,
                &notifier,
                create_error_guidance(
                    &format!("Initial sync failed: {}", e),
                    "Will retry on next interval",
                ),
            );
        }
    }

    // ---- Phase 3: Periodic sync ----
    loop {
        match rx.recv_timeout(sync_interval) {
            Ok(SyncMessage::SyncNow) | Err(mpsc::RecvTimeoutError::Timeout) => {
                match engine.pull(&client, &project_id) {
                    Ok(commands) => {
                        if !commands.is_empty() {
                            queue_commands(&pending, &notifier, commands);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("cloud-sync: Periodic sync failed: {}", e);
                    }
                }
            }
            Ok(SyncMessage::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                tracing::info!("cloud-sync: Sync thread shutting down");
                break;
            }
        }
    }
}

/// Run the device code authorization flow.
///
/// Returns the auth token on success, or `None` if the thread was shut down
/// or an unrecoverable error occurred.
fn run_device_auth(
    rx: &mpsc::Receiver<SyncMessage>,
    engine: &SyncEngine,
    pending: &Arc<Mutex<Vec<FfiCommand>>>,
    notifier: &SharedNotifier,
    api_url: &str,
) -> Option<String> {
    // Request device code from server
    let device_resp = match api::request_device_code(api_url) {
        Ok(resp) => resp,
        Err(e) => {
            queue_commands(
                pending,
                notifier,
                create_error_guidance(
                    &format!("Failed to start sign-in: {}", e),
                    "Check your API URL and network connection",
                ),
            );
            return None;
        }
    };

    // Show auth guidance to user
    queue_commands(pending, notifier, create_auth_guidance(&device_resp));

    // Poll for token
    let poll_interval = Duration::from_secs(device_resp.interval.max(5));
    loop {
        match rx.recv_timeout(poll_interval) {
            Ok(SyncMessage::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            _ => {}
        }

        match api::poll_device_token(api_url, &device_resp.device_code) {
            Ok(Some(token)) => {
                // Store token
                {
                    let mut state = engine.state.lock().unwrap();
                    state.auth_token = Some(token.clone());
                    let _ = models::save_sync_state(&state);
                }

                // Clear auth guidance
                queue_commands(
                    pending,
                    notifier,
                    vec![
                        FfiCommand::DeleteTodo {
                            id: RString::from("cloud-sync-auth-header"),
                        },
                        FfiCommand::DeleteTodo {
                            id: RString::from("cloud-sync-auth-url"),
                        },
                        FfiCommand::DeleteTodo {
                            id: RString::from("cloud-sync-auth-code"),
                        },
                    ],
                );

                return Some(token);
            }
            Ok(None) => {
                // Still waiting — continue polling
            }
            Err(e) => {
                tracing::warn!("cloud-sync: Auth poll error: {}", e);
            }
        }
    }
}

// ============================================================================
// Guidance helpers (free functions for use from background thread)
// ============================================================================

/// Notify the host via the shared notifier.
fn notify(notifier: &SharedNotifier) {
    if let Some(n) = *notifier.lock().unwrap() {
        (n.func)();
    }
}

/// Push commands to the shared pending list and wake the host.
fn queue_commands(
    pending: &Arc<Mutex<Vec<FfiCommand>>>,
    notifier: &SharedNotifier,
    commands: Vec<FfiCommand>,
) {
    if commands.is_empty() {
        return;
    }
    pending.lock().unwrap().extend(commands);
    notify(notifier);
}

/// Create guidance todos for the device code sign-in flow.
fn create_auth_guidance(device_resp: &DeviceCodeResponse) -> Vec<FfiCommand> {
    vec![
        FfiCommand::CreateTodo {
            content: RString::from("CLOUD SYNC: Sign in to get started"),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-auth-header")),
            state: FfiTodoState::Question,
            priority: ROption::RNone,
            indent_level: 0,
        },
        FfiCommand::CreateTodo {
            content: RString::from(format!("Open: {}", device_resp.verification_uri)),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-auth-url")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
        FfiCommand::CreateTodo {
            content: RString::from(format!("Code: {}", device_resp.user_code)),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-auth-code")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
    ]
}

/// Create guidance for unconfigured state.
fn create_unconfigured_guidance() -> Vec<FfiCommand> {
    vec![
        FfiCommand::CreateTodo {
            content: RString::from("CLOUD SYNC: Not configured"),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-guidance-header")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        },
        FfiCommand::CreateTodo {
            content: RString::from(
                "Set API URL and project name in plugin settings",
            ),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-guidance-hint")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
    ]
}

/// Create guidance for error state.
fn create_error_guidance(error: &str, hint: &str) -> Vec<FfiCommand> {
    vec![
        FfiCommand::CreateTodo {
            content: RString::from(format!("CLOUD SYNC ERROR: {}", error)),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-error-header")),
            state: FfiTodoState::Exclamation,
            priority: ROption::RNone,
            indent_level: 0,
        },
        FfiCommand::CreateTodo {
            content: RString::from(hint.to_string()),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from("cloud-sync-error-hint")),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
    ]
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
            fields: vec![api_url_field, project_name_field, sync_interval_field]
                .into_iter()
                .collect(),
            config_required: true,
        }
    }

    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>) {
        let mut plugin_config = self.config.lock().unwrap();

        if let Some(FfiConfigValue::String(url)) = config.get(&RString::from("api_url")) {
            let url_str = url.to_string();
            if !url_str.is_empty() {
                plugin_config.api_url = Some(url_str);
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

        if plugin_config.is_configured() {
            drop(plugin_config);
            // Start background thread — handles auth + initial pull + periodic sync
            self.start_sync_thread();
        } else {
            drop(plugin_config);
            let commands = create_unconfigured_guidance();
            self.pending_commands.lock().unwrap().extend(commands);
            self.notify_host();
        }
    }

    fn execute_with_host(
        &self,
        _input: RString,
        host: HostApi_TO<'_, RBox<()>>,
    ) -> RResult<RVec<FfiCommand>, RString> {
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
        let commands = create_unconfigured_guidance();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_error_guidance() {
        let commands = create_error_guidance("test error", "try again");
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_auth_guidance() {
        let device_resp = DeviceCodeResponse {
            device_code: "dev-123".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://totui.app/activate".to_string(),
            interval: 5,
            expires_in: 600,
        };
        let commands = create_auth_guidance(&device_resp);
        assert_eq!(commands.len(), 3);
    }
}
