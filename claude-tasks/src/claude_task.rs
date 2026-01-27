//! Claude Code task data structures and parsing.
//!
//! Claude Code stores tasks in `~/.claude/tasks/{uuid}/*.json` with this schema.

use serde::{Deserialize, Serialize};
use totui_plugin_interface::FfiTodoState;

/// A task from Claude Code's task list.
///
/// Maps directly to the JSON schema: `{id, subject, description, activeForm, status, blocks[], blockedBy[]}`
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeTask {
    /// Numeric string ID (e.g., "1", "2")
    pub id: String,
    /// Short title of the task
    pub subject: String,
    /// Detailed description
    pub description: String,
    /// Current activity description (spinner text)
    #[serde(rename = "activeForm")]
    pub active_form: String,
    /// Status: "pending", "in_progress", "completed"
    pub status: String,
    /// Task IDs this task blocks (downstream dependencies)
    #[serde(default)]
    pub blocks: Vec<String>,
    /// Task IDs blocking this task (upstream dependencies)
    #[serde(rename = "blockedBy", default)]
    pub blocked_by: Vec<String>,
}

/// Map Claude task status to totui todo state.
///
/// - "pending" -> Empty ([ ])
/// - "in_progress" -> InProgress ([*])
/// - "completed" -> Checked ([x])
pub fn map_status_to_state(status: &str) -> FfiTodoState {
    match status {
        "pending" => FfiTodoState::Empty,
        "in_progress" => FfiTodoState::InProgress,
        "completed" => FfiTodoState::Checked,
        _ => FfiTodoState::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_task() {
        let json = r#"{
            "id": "1",
            "subject": "Test task",
            "description": "Test description",
            "activeForm": "Testing",
            "status": "pending",
            "blocks": [],
            "blockedBy": []
        }"#;

        let task: ClaudeTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "1");
        assert_eq!(task.subject, "Test task");
        assert_eq!(task.description, "Test description");
        assert_eq!(task.active_form, "Testing");
        assert_eq!(task.status, "pending");
        assert!(task.blocks.is_empty());
        assert!(task.blocked_by.is_empty());
    }

    #[test]
    fn test_parse_claude_task_with_dependencies() {
        let json = r#"{
            "id": "2",
            "subject": "Dependent task",
            "description": "Has dependencies",
            "activeForm": "Waiting",
            "status": "in_progress",
            "blocks": ["3", "4"],
            "blockedBy": ["1"]
        }"#;

        let task: ClaudeTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "2");
        assert_eq!(task.status, "in_progress");
        assert_eq!(task.blocks, vec!["3", "4"]);
        assert_eq!(task.blocked_by, vec!["1"]);
    }

    #[test]
    fn test_parse_claude_task_missing_optional_fields() {
        // blocks and blockedBy should default to empty vecs if missing
        let json = r#"{
            "id": "1",
            "subject": "Minimal task",
            "description": "No deps",
            "activeForm": "",
            "status": "completed"
        }"#;

        let task: ClaudeTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "1");
        assert!(task.blocks.is_empty());
        assert!(task.blocked_by.is_empty());
    }

    #[test]
    fn test_status_mapping() {
        assert!(matches!(
            map_status_to_state("pending"),
            FfiTodoState::Empty
        ));
        assert!(matches!(
            map_status_to_state("in_progress"),
            FfiTodoState::InProgress
        ));
        assert!(matches!(
            map_status_to_state("completed"),
            FfiTodoState::Checked
        ));
        // Unknown status defaults to Empty
        assert!(matches!(
            map_status_to_state("unknown"),
            FfiTodoState::Empty
        ));
    }
}
