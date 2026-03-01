//! Sync engine for cloud sync plugin.
//!
//! Handles bidirectional synchronization between local totui todos and the cloud.
//! Uses last-write-wins conflict resolution with server as the authority.

use crate::api::{ApiClientError, CloudClient};
use crate::models::*;
use abi_stable::std_types::{ROption, RString};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use totui_plugin_interface::{FfiCommand, FfiPriority, FfiTodoState};

/// The sync engine manages bidirectional sync between totui and the cloud.
pub struct SyncEngine {
    pub state: Mutex<SyncState>,
}

impl SyncEngine {
    /// Create a new sync engine, loading persisted state.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(load_sync_state()),
        }
    }

    /// Ensure a project exists in the cloud, creating it if needed.
    /// Returns the project ID.
    pub fn ensure_project(
        &self,
        client: &CloudClient,
        project_name: &str,
    ) -> Result<String, ApiClientError> {
        // Check if we already have a project ID cached
        {
            let state = self.state.lock().unwrap();
            if let Some(ref id) = state.project_id {
                if state.project_name.as_deref() == Some(project_name) {
                    return Ok(id.clone());
                }
            }
        }

        // Look for existing project by name
        let projects = client.list_projects()?;
        if let Some(existing) = projects.iter().find(|p| p.name == project_name) {
            let mut state = self.state.lock().unwrap();
            state.project_id = Some(existing.id.clone());
            state.project_name = Some(project_name.to_string());
            let _ = save_sync_state(&state);
            return Ok(existing.id.clone());
        }

        // Create new project
        let project = client.create_project(project_name)?;
        let mut state = self.state.lock().unwrap();
        state.project_id = Some(project.id.clone());
        state.project_name = Some(project_name.to_string());
        let _ = save_sync_state(&state);
        Ok(project.id)
    }

    /// Perform a full sync cycle.
    ///
    /// 1. Collects local todos from the host
    /// 2. Pushes them to the cloud
    /// 3. Receives remote changes
    /// 4. Returns FfiCommands to apply remote changes locally
    pub fn sync(
        &self,
        client: &CloudClient,
        project_id: &str,
        local_todos: Vec<LocalTodoSnapshot>,
    ) -> Result<Vec<FfiCommand>, ApiClientError> {
        let (last_synced_at, client_id) = {
            let state = self.state.lock().unwrap();
            (state.last_synced_at, state.client_id.clone())
        };

        // Build cloud todos from local state
        let cloud_todos = self.local_to_cloud(project_id, &local_todos);

        // Sync with server
        let request = SyncRequest {
            todos: cloud_todos,
            last_synced_at,
            client_id,
        };

        let response = client.sync_todos(project_id, &request)?;

        // Process response and generate commands
        let commands = self.process_sync_response(&response, &local_todos);

        // Update sync state
        {
            let mut state = self.state.lock().unwrap();
            state.last_synced_at = Some(response.synced_at);
            let _ = save_sync_state(&state);
        }

        Ok(commands)
    }

    /// Pull remote todos (initial load or refresh).
    /// Returns commands to create/update local todos from cloud state.
    pub fn pull(
        &self,
        client: &CloudClient,
        project_id: &str,
    ) -> Result<Vec<FfiCommand>, ApiClientError> {
        let remote_todos = client.get_todos(project_id)?;

        let mut commands = Vec::new();
        let mut state = self.state.lock().unwrap();

        // Create header
        let header_content = format!(
            "CLOUD SYNC: {}",
            state
                .project_name
                .as_deref()
                .unwrap_or("Synced Project")
        );
        let header_id = format!("cloud-sync-header-{}", project_id);

        commands.push(FfiCommand::CreateTodo {
            content: RString::from(header_content),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(RString::from(header_id)),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        });

        for todo in &remote_todos {
            let local_id = format!("cloud-{}-{}", project_id, todo.id);

            // Track the mapping
            state.map_ids(&todo.id, &local_id);

            let metadata = SyncMetadata {
                source: "cloud-sync".to_string(),
                cloud_id: todo.id.clone(),
                project_id: project_id.to_string(),
                last_synced_at: now_timestamp(),
                content_hash: Some(content_hash(&todo.content)),
            };

            commands.push(FfiCommand::CreateTodo {
                content: RString::from(todo.content.clone()),
                parent_id: ROption::RNone,
                temp_id: ROption::RSome(RString::from(local_id.clone())),
                state: cloud_state_to_ffi(&todo.state),
                priority: priority_str_to_ffi(todo.priority.as_deref()),
                indent_level: 0,
            });

            commands.push(FfiCommand::SetTodoMetadata {
                todo_id: RString::from(local_id),
                data: RString::from(metadata.to_json()),
                merge: false,
            });
        }

        state.last_synced_at = Some(now_timestamp());
        let _ = save_sync_state(&state);

        Ok(commands)
    }

    /// Convert local todos to cloud format for pushing.
    fn local_to_cloud(
        &self,
        project_id: &str,
        local_todos: &[LocalTodoSnapshot],
    ) -> Vec<CloudTodo> {
        let state = self.state.lock().unwrap();
        let mut cloud_todos = Vec::new();

        for local in local_todos {
            let cloud_id = state
                .cloud_id(&local.id)
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            cloud_todos.push(CloudTodo {
                id: cloud_id,
                project_id: project_id.to_string(),
                content: local.content.clone(),
                description: local.description.clone(),
                state: ffi_state_to_cloud(&local.state),
                priority: local.priority.clone(),
                due_date: local.due_date.clone(),
                parent_id: None,
                position: local.position,
                created_at: local.created_at,
                modified_at: local.modified_at,
                completed_at: local.completed_at,
                source: Some("totui".to_string()),
            });
        }

        cloud_todos
    }

    /// Process sync response from the server.
    /// Generates FfiCommands to apply remote changes locally.
    fn process_sync_response(
        &self,
        response: &SyncResponse,
        local_todos: &[LocalTodoSnapshot],
    ) -> Vec<FfiCommand> {
        let mut commands = Vec::new();
        let mut state = self.state.lock().unwrap();

        // Build a lookup of local todos by their cloud ID
        let local_by_cloud_id: HashMap<String, &LocalTodoSnapshot> = local_todos
            .iter()
            .filter_map(|t| {
                state.cloud_id(&t.id).map(|cid| (cid.clone(), t))
            })
            .collect();

        // Process remote todos (creates and updates)
        for remote in &response.todos {
            let metadata = SyncMetadata {
                source: "cloud-sync".to_string(),
                cloud_id: remote.id.clone(),
                project_id: remote.project_id.clone(),
                last_synced_at: response.synced_at,
                content_hash: Some(content_hash(&remote.content)),
            };

            if let Some(local) = local_by_cloud_id.get(&remote.id) {
                // Existing todo - check if remote is newer
                if remote.modified_at > local.modified_at {
                    commands.push(FfiCommand::UpdateTodo {
                        id: RString::from(local.id.clone()),
                        content: ROption::RSome(RString::from(remote.content.clone())),
                        state: ROption::RSome(cloud_state_to_ffi(&remote.state)),
                        priority: priority_str_to_ffi(remote.priority.as_deref()),
                        due_date: remote
                            .due_date
                            .as_ref()
                            .map(|d| RString::from(d.clone()))
                            .into(),
                        description: remote
                            .description
                            .as_ref()
                            .map(|d| RString::from(d.clone()))
                            .into(),
                    });

                    commands.push(FfiCommand::SetTodoMetadata {
                        todo_id: RString::from(local.id.clone()),
                        data: RString::from(metadata.to_json()),
                        merge: true,
                    });
                }
            } else {
                // New remote todo - create locally
                let local_id = format!(
                    "cloud-{}-{}",
                    remote.project_id, remote.id
                );
                state.map_ids(&remote.id, &local_id);

                commands.push(FfiCommand::CreateTodo {
                    content: RString::from(remote.content.clone()),
                    parent_id: ROption::RNone,
                    temp_id: ROption::RSome(RString::from(local_id.clone())),
                    state: cloud_state_to_ffi(&remote.state),
                    priority: priority_str_to_ffi(remote.priority.as_deref()),
                    indent_level: 0,
                });

                commands.push(FfiCommand::SetTodoMetadata {
                    todo_id: RString::from(local_id),
                    data: RString::from(metadata.to_json()),
                    merge: false,
                });
            }
        }

        // Process deletions
        for deleted_id in &response.deleted_ids {
            if let Some(local_id) = state.local_id(deleted_id).cloned() {
                commands.push(FfiCommand::DeleteTodo {
                    id: RString::from(local_id),
                });
                state.unmap_cloud_id(deleted_id);
            }
        }

        commands
    }
}

// ============================================================================
// Snapshot of a local todo (captured from HostApi)
// ============================================================================

/// A snapshot of a local todo, captured from the host for sync purposes.
#[derive(Debug, Clone)]
pub struct LocalTodoSnapshot {
    pub id: String,
    pub content: String,
    pub description: Option<String>,
    pub state: FfiTodoState,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub position: u32,
    pub created_at: i64,
    pub modified_at: i64,
    pub completed_at: Option<i64>,
    pub metadata_json: Option<String>,
}

impl LocalTodoSnapshot {
    /// Check if this todo was synced by the cloud-sync plugin.
    pub fn is_cloud_synced(&self) -> bool {
        self.metadata_json
            .as_ref()
            .and_then(|json| SyncMetadata::from_json(json))
            .map(|m| m.source == "cloud-sync")
            .unwrap_or(false)
    }

    /// Get the cloud sync metadata if present.
    pub fn sync_metadata(&self) -> Option<SyncMetadata> {
        self.metadata_json
            .as_ref()
            .and_then(|json| SyncMetadata::from_json(json))
            .filter(|m| m.source == "cloud-sync")
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

/// Convert CloudTodoState to FfiTodoState.
pub fn cloud_state_to_ffi(state: &CloudTodoState) -> FfiTodoState {
    match state {
        CloudTodoState::Empty => FfiTodoState::Empty,
        CloudTodoState::InProgress => FfiTodoState::InProgress,
        CloudTodoState::Checked => FfiTodoState::Checked,
    }
}

/// Convert FfiTodoState to CloudTodoState.
pub fn ffi_state_to_cloud(state: &FfiTodoState) -> CloudTodoState {
    match state {
        FfiTodoState::Empty
        | FfiTodoState::Question
        | FfiTodoState::Exclamation => CloudTodoState::Empty,
        FfiTodoState::InProgress => CloudTodoState::InProgress,
        FfiTodoState::Checked
        | FfiTodoState::Cancelled => CloudTodoState::Checked,
    }
}

/// Generate a simple hash of content for dirty tracking.
pub fn content_hash(content: &str) -> String {
    // Simple FNV-1a hash - fast and good enough for dirty checking
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in content.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Get current Unix timestamp in seconds.
pub fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Convert an FfiTodoItem to a LocalTodoSnapshot.
pub fn ffi_todo_to_snapshot(item: &totui_plugin_interface::FfiTodoItem) -> LocalTodoSnapshot {
    LocalTodoSnapshot {
        id: item.id.to_string(),
        content: item.content.to_string(),
        description: match &item.description {
            ROption::RSome(d) => Some(d.to_string()),
            ROption::RNone => None,
        },
        state: item.state.clone(),
        priority: match &item.priority {
            ROption::RSome(p) => Some(ffi_priority_to_str(p).to_string()),
            ROption::RNone => None,
        },
        due_date: match &item.due_date {
            ROption::RSome(d) => Some(d.to_string()),
            ROption::RNone => None,
        },
        position: item.position,
        created_at: item.created_at,
        modified_at: item.modified_at,
        completed_at: match item.completed_at {
            ROption::RSome(t) => Some(t),
            ROption::RNone => None,
        },
        metadata_json: None, // Would need HostApi to get metadata
    }
}

/// Convert FfiPriority to string representation.
fn ffi_priority_to_str(priority: &FfiPriority) -> &str {
    match priority {
        FfiPriority::P0 => "p0",
        FfiPriority::P1 => "p1",
        FfiPriority::P2 => "p2",
    }
}

/// Convert priority string to FfiPriority.
fn priority_str_to_ffi(priority: Option<&str>) -> ROption<FfiPriority> {
    match priority {
        Some("p0") | Some("P0") => ROption::RSome(FfiPriority::P0),
        Some("p1") | Some("P1") => ROption::RSome(FfiPriority::P1),
        Some("p2") | Some("P2") => ROption::RSome(FfiPriority::P2),
        _ => ROption::RNone,
    }
}

// ============================================================================
// Background sync thread
// ============================================================================

/// Messages sent to the sync thread.
pub enum SyncMessage {
    /// Trigger a sync cycle now
    SyncNow,
    /// Stop the sync thread
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash() {
        let h1 = content_hash("hello");
        let h2 = content_hash("hello");
        let h3 = content_hash("world");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_state_conversion_roundtrip() {
        let states = vec![
            CloudTodoState::Empty,
            CloudTodoState::InProgress,
            CloudTodoState::Checked,
        ];
        for state in states {
            let ffi = cloud_state_to_ffi(&state);
            let back = ffi_state_to_cloud(&ffi);
            assert_eq!(state, back);
        }
    }

    #[test]
    fn test_local_todo_snapshot_cloud_detection() {
        let synced = LocalTodoSnapshot {
            id: "test".to_string(),
            content: "test".to_string(),
            description: None,
            state: FfiTodoState::Empty,
            priority: None,
            due_date: None,
            position: 0,
            created_at: 0,
            modified_at: 0,
            completed_at: None,
            metadata_json: Some(
                r#"{"source":"cloud-sync","cloud_id":"abc","project_id":"p1","last_synced_at":0}"#
                    .to_string(),
            ),
        };
        assert!(synced.is_cloud_synced());

        let not_synced = LocalTodoSnapshot {
            metadata_json: Some(r#"{"source":"other-plugin"}"#.to_string()),
            ..synced.clone()
        };
        assert!(!not_synced.is_cloud_synced());
    }
}
