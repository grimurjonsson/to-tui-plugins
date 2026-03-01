//! Data models for cloud sync.
//!
//! Defines the types used for cloud API communication and local sync state tracking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Cloud API request/response types
// ============================================================================

/// A project in the cloud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProject {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub modified_at: i64,
}

/// A todo item in the cloud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTodo {
    pub id: String,
    pub project_id: String,
    pub content: String,
    pub description: Option<String>,
    pub state: CloudTodoState,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub parent_id: Option<String>,
    pub position: u32,
    pub created_at: i64,
    pub modified_at: i64,
    pub completed_at: Option<i64>,
    /// Tracks where this todo originated (e.g. "totui", "mobile", "web")
    pub source: Option<String>,
}

/// Todo state, mirroring FfiTodoState.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CloudTodoState {
    Empty,
    InProgress,
    Checked,
}

/// Request body for creating a project.
#[derive(Debug, Serialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

/// Request body for syncing todos (push local state, receive remote state).
#[derive(Debug, Serialize)]
pub struct SyncRequest {
    pub todos: Vec<CloudTodo>,
    /// Timestamp of last successful sync (server returns only changes after this)
    pub last_synced_at: Option<i64>,
    pub client_id: String,
}

/// Response from sync endpoint.
#[derive(Debug, Deserialize)]
pub struct SyncResponse {
    /// Todos that were updated/created on the server (includes remote changes)
    pub todos: Vec<CloudTodo>,
    /// Todos that were deleted on the server since last sync
    pub deleted_ids: Vec<String>,
    /// Server timestamp for this sync
    pub synced_at: i64,
}

/// Response wrapping for API errors.
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: Option<String>,
}

// ============================================================================
// Local sync state
// ============================================================================

/// Metadata attached to each synced todo for tracking sync state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncMetadata {
    pub source: String,
    pub cloud_id: String,
    pub project_id: String,
    pub last_synced_at: i64,
    /// Hash of the content at last sync, for dirty detection
    pub content_hash: Option<String>,
}

impl SyncMetadata {
    /// Parse from JSON metadata string.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Tracks the local state of a synced todo for diffing.
#[derive(Debug, Clone)]
pub struct TrackedTodo {
    pub local_id: String,
    pub cloud_id: String,
    pub content: String,
    pub description: Option<String>,
    pub state: CloudTodoState,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub position: u32,
    pub modified_at: i64,
}

/// The full local sync state, persisted between sessions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncState {
    /// Cloud project ID we're syncing with
    pub project_id: Option<String>,
    /// Cloud project name
    pub project_name: Option<String>,
    /// Last successful sync timestamp (server time)
    pub last_synced_at: Option<i64>,
    /// Unique client ID for this totui instance
    pub client_id: String,
    /// Map of cloud_id -> local todo_id for correlation
    pub id_map: HashMap<String, String>,
    /// Map of local todo_id -> cloud_id (reverse lookup)
    pub reverse_id_map: HashMap<String, String>,
}

impl SyncState {
    pub fn new(client_id: String) -> Self {
        Self {
            client_id,
            ..Default::default()
        }
    }

    /// Register an ID mapping between cloud and local.
    pub fn map_ids(&mut self, cloud_id: &str, local_id: &str) {
        self.id_map
            .insert(cloud_id.to_string(), local_id.to_string());
        self.reverse_id_map
            .insert(local_id.to_string(), cloud_id.to_string());
    }

    /// Look up local ID from cloud ID.
    pub fn local_id(&self, cloud_id: &str) -> Option<&String> {
        self.id_map.get(cloud_id)
    }

    /// Look up cloud ID from local ID.
    pub fn cloud_id(&self, local_id: &str) -> Option<&String> {
        self.reverse_id_map.get(local_id)
    }

    /// Remove mapping for a cloud ID.
    pub fn unmap_cloud_id(&mut self, cloud_id: &str) {
        if let Some(local_id) = self.id_map.remove(cloud_id) {
            self.reverse_id_map.remove(&local_id);
        }
    }

    /// Remove mapping for a local ID.
    pub fn unmap_local_id(&mut self, local_id: &str) {
        if let Some(cloud_id) = self.reverse_id_map.remove(local_id) {
            self.id_map.remove(&cloud_id);
        }
    }
}

/// Persist sync state to disk.
pub fn save_sync_state(state: &SyncState) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?
        .join("totui");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let path = config_dir.join("cloud-sync-state.json");
    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write state: {}", e))?;
    Ok(())
}

/// Load sync state from disk.
pub fn load_sync_state() -> SyncState {
    let path = dirs::config_dir()
        .map(|d| d.join("totui").join("cloud-sync-state.json"))
        .unwrap_or_default();

    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| SyncState::new(generate_client_id()))
    } else {
        SyncState::new(generate_client_id())
    }
}

/// Generate a unique client ID for this totui instance.
fn generate_client_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_metadata_roundtrip() {
        let meta = SyncMetadata {
            source: "cloud-sync".to_string(),
            cloud_id: "abc-123".to_string(),
            project_id: "proj-1".to_string(),
            last_synced_at: 1700000000,
            content_hash: Some("deadbeef".to_string()),
        };

        let json = meta.to_json();
        let parsed = SyncMetadata::from_json(&json).unwrap();
        assert_eq!(parsed.cloud_id, "abc-123");
        assert_eq!(parsed.source, "cloud-sync");
    }

    #[test]
    fn test_sync_state_id_mapping() {
        let mut state = SyncState::new("test-client".to_string());
        state.map_ids("cloud-1", "local-1");
        state.map_ids("cloud-2", "local-2");

        assert_eq!(state.local_id("cloud-1"), Some(&"local-1".to_string()));
        assert_eq!(state.cloud_id("local-2"), Some(&"cloud-2".to_string()));

        state.unmap_cloud_id("cloud-1");
        assert_eq!(state.local_id("cloud-1"), None);
        assert_eq!(state.cloud_id("local-1"), None);
    }

    #[test]
    fn test_cloud_todo_state_serialization() {
        let state = CloudTodoState::InProgress;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let parsed: CloudTodoState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CloudTodoState::InProgress);
    }
}
