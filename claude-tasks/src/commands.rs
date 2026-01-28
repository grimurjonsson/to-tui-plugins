//! FfiCommand builders for sync operations.
//!
//! Helper functions to generate FfiCommand instances for creating, updating,
//! and deleting todos from Claude tasks.

use crate::claude_task::{map_status_to_state, ClaudeTask};
use abi_stable::std_types::{ROption, RString};
use totui_plugin_interface::{FfiCommand, FfiTodoState};

/// Create a header todo command for a tasklist.
///
/// Header format: "CLAUDE TASKLIST: {display_name}"
/// where display_name is alias or tasklist_id
/// - temp_id: "claude-header-{tasklist_id}"
/// - state: Empty (header is never checked)
/// - indent_level: 0 (top level)
pub fn create_header_command(tasklist_id: &str, display_name: Option<&str>) -> FfiCommand {
    let name = display_name.unwrap_or(tasklist_id);
    FfiCommand::CreateTodo {
        content: RString::from(format!("CLAUDE TASKLIST: {}", name)),
        parent_id: ROption::RNone,
        temp_id: ROption::RSome(RString::from(format!("claude-header-{}", tasklist_id))),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 0,
    }
}

/// Create commands for a new todo from a Claude task.
///
/// Returns two commands:
/// 1. CreateTodo - creates the todo item
/// 2. SetTodoMetadata - sets correlation metadata
///
/// Content format:
/// - Normal task: "{subject}"
/// - Blocked task: "[blocked] {subject}"
pub fn create_todo_commands(
    task: &ClaudeTask,
    tasklist_id: &str,
    header_id: &str,
) -> Vec<FfiCommand> {
    let temp_id = format!("claude-{}-{}", tasklist_id, task.id);

    // Format content - prefix with blocked indicator if task has blockers
    let content = format_task_content(task);

    let create_cmd = FfiCommand::CreateTodo {
        content: RString::from(content),
        parent_id: ROption::RSome(RString::from(header_id)),
        temp_id: ROption::RSome(RString::from(temp_id.clone())),
        state: map_status_to_state(&task.status),
        priority: ROption::RNone,
        indent_level: 1,
    };

    // Build metadata JSON
    let metadata = build_metadata_json(tasklist_id, &task.id, &task.blocked_by);

    let metadata_cmd = FfiCommand::SetTodoMetadata {
        todo_id: RString::from(temp_id),
        data: RString::from(metadata),
        merge: false,
    };

    vec![create_cmd, metadata_cmd]
}

/// Create commands for a new todo with hierarchy context.
///
/// Uses hierarchy to determine:
/// - annotation: appended to content in grey if blocked
///
/// All tasks use header as parent (flat list) at indent_level 1.
///
/// Returns two commands:
/// 1. CreateTodo - creates the todo item at root level
/// 2. SetTodoMetadata - sets correlation metadata
pub fn create_todo_commands_with_hierarchy(
    task: &ClaudeTask,
    tasklist_id: &str,
    header_id: &str,
    hierarchy: &crate::hierarchy::TaskHierarchy,
) -> Vec<FfiCommand> {
    let temp_id = format!("claude-{}-{}", tasklist_id, task.id);

    // Always use header as parent (flat list)
    let parent_id = ROption::RSome(RString::from(header_id));

    // Always indent level 1 (flat list under header)
    let indent_level = 1;

    // Format content with annotation if needed
    let content = if let Some(annotation) = hierarchy.get_annotation(&task.id) {
        // Annotation for blocked tasks or cycles
        if hierarchy.is_cyclic(&task.id) {
            format!("{} {}", annotation, task.subject)
        } else {
            // Blocked tasks: "🔒 Subject (blocked by: A, B)"
            format!("\u{1F512} {} {}", task.subject, annotation)
        }
    } else {
        // Normal content - just subject
        task.subject.clone()
    };

    let create_cmd = FfiCommand::CreateTodo {
        content: RString::from(content),
        parent_id,
        temp_id: ROption::RSome(RString::from(temp_id.clone())),
        state: map_status_to_state(&task.status),
        priority: ROption::RNone,
        indent_level,
    };

    // Build metadata JSON
    let metadata = build_metadata_json(tasklist_id, &task.id, &task.blocked_by);

    let metadata_cmd = FfiCommand::SetTodoMetadata {
        todo_id: RString::from(temp_id),
        data: RString::from(metadata),
        merge: false,
    };

    vec![create_cmd, metadata_cmd]
}

/// Create an update command for an existing todo.
///
/// Updates content and state to match the Claude task.
pub fn update_todo_command(task: &ClaudeTask, existing_todo_id: &str) -> FfiCommand {
    let content = format_task_content(task);

    FfiCommand::UpdateTodo {
        id: RString::from(existing_todo_id),
        content: ROption::RSome(RString::from(content)),
        state: ROption::RSome(map_status_to_state(&task.status)),
        priority: ROption::RNone,
        due_date: ROption::RNone,
        description: ROption::RNone,
    }
}

/// Create a delete command for a todo.
pub fn delete_todo_command(todo_id: &str) -> FfiCommand {
    FfiCommand::DeleteTodo {
        id: RString::from(todo_id),
    }
}

/// Format task content.
///
/// Returns just the subject - blocked annotation is handled separately
/// by hierarchy in `create_todo_commands_with_hierarchy`.
fn format_task_content(task: &ClaudeTask) -> String {
    task.subject.clone()
}

/// Build metadata JSON string for a task.
///
/// Metadata includes:
/// - source: "claude-tasks" (for querying all synced todos)
/// - tasklist_id: UUID of the tasklist
/// - task_id: ID of the task within the tasklist
/// - read_only: true (tasks are managed by Claude)
/// - blocked_by: array of blocking task IDs (if any)
fn build_metadata_json(tasklist_id: &str, task_id: &str, blocked_by: &[String]) -> String {
    let blocked_by_json = if blocked_by.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = blocked_by.iter().map(|id| format!("\"{}\"", id)).collect();
        format!("[{}]", items.join(","))
    };

    format!(
        r#"{{"source":"claude-tasks","tasklist_id":"{}","task_id":"{}","read_only":true,"blocked_by":{}}}"#,
        tasklist_id, task_id, blocked_by_json
    )
}

/// Get the header todo ID for a tasklist.
pub fn header_id(tasklist_id: &str) -> String {
    format!("claude-header-{}", tasklist_id)
}

/// Get the todo temp_id for a task.
pub fn task_todo_id(tasklist_id: &str, task_id: &str) -> String {
    format!("claude-{}-{}", tasklist_id, task_id)
}

/// Create an update command for the header todo with optional staleness indicator.
///
/// Format: "CLAUDE TASKLIST: {name}" or "CLAUDE TASKLIST: {name} \u{23F0} STALE (Xm)"
pub fn update_header_command(
    tasklist_id: &str,
    display_name: Option<&str>,
    staleness: Option<&str>,
) -> FfiCommand {
    let name = display_name.unwrap_or(tasklist_id);
    let content = match staleness {
        Some(duration) => format!("CLAUDE TASKLIST: {} \u{23F0} STALE ({})", name, duration),
        None => format!("CLAUDE TASKLIST: {}", name),
    };

    FfiCommand::UpdateTodo {
        id: RString::from(header_id(tasklist_id)),
        content: ROption::RSome(RString::from(content)),
        state: ROption::RNone,
        priority: ROption::RNone,
        due_date: ROption::RNone,
        description: ROption::RNone,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_task(id: &str, subject: &str, status: &str) -> ClaudeTask {
        ClaudeTask {
            id: id.to_string(),
            subject: subject.to_string(),
            description: String::new(),
            active_form: String::new(),
            status: status.to_string(),
            blocks: vec![],
            blocked_by: vec![],
        }
    }

    fn make_blocked_task(id: &str, subject: &str, blocked_by: Vec<&str>) -> ClaudeTask {
        ClaudeTask {
            id: id.to_string(),
            subject: subject.to_string(),
            description: String::new(),
            active_form: String::new(),
            status: "pending".to_string(),
            blocks: vec![],
            blocked_by: blocked_by.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_create_header_command() {
        let cmd = create_header_command("abc-123", None);

        match cmd {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                state,
                indent_level,
                ..
            } => {
                assert_eq!(content.as_str(), "CLAUDE TASKLIST: abc-123");
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == "claude-header-abc-123")
                );
                assert!(matches!(state, FfiTodoState::Empty));
                assert_eq!(indent_level, 0);
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_header_command_with_alias() {
        let cmd = create_header_command("abc-123", Some("My Tasks"));

        match cmd {
            FfiCommand::CreateTodo { content, .. } => {
                assert_eq!(content.as_str(), "CLAUDE TASKLIST: My Tasks");
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_todo_commands_pending() {
        let task = make_test_task("1", "Test task", "pending");
        let cmds = create_todo_commands(&task, "tasklist-1", "claude-header-tasklist-1");

        assert_eq!(cmds.len(), 2);

        // Check CreateTodo
        match &cmds[0] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                state,
                parent_id,
                indent_level,
                ..
            } => {
                assert_eq!(content.as_str(), "Test task");
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == "claude-tasklist-1-1")
                );
                assert!(matches!(state, FfiTodoState::Empty));
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == "claude-header-tasklist-1")
                );
                assert_eq!(*indent_level, 1);
            }
            _ => panic!("Expected CreateTodo command"),
        }

        // Check SetTodoMetadata
        match &cmds[1] {
            FfiCommand::SetTodoMetadata {
                todo_id,
                data,
                merge,
            } => {
                assert_eq!(todo_id.as_str(), "claude-tasklist-1-1");
                assert!(data.as_str().contains("\"source\":\"claude-tasks\""));
                assert!(data.as_str().contains("\"task_id\":\"1\""));
                assert!(data.as_str().contains("\"read_only\":true"));
                assert!(!merge);
            }
            _ => panic!("Expected SetTodoMetadata command"),
        }
    }

    #[test]
    fn test_create_todo_commands_in_progress() {
        let task = make_test_task("2", "Working on it", "in_progress");
        let cmds = create_todo_commands(&task, "tasklist-1", "header-1");

        match &cmds[0] {
            FfiCommand::CreateTodo { state, .. } => {
                assert!(matches!(state, FfiTodoState::InProgress));
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_todo_commands_completed() {
        let task = make_test_task("3", "Done", "completed");
        let cmds = create_todo_commands(&task, "tasklist-1", "header-1");

        match &cmds[0] {
            FfiCommand::CreateTodo { state, .. } => {
                assert!(matches!(state, FfiTodoState::Checked));
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_todo_commands_blocked() {
        // Basic create_todo_commands just uses subject (no emoji)
        // Blocked annotation is handled by create_todo_commands_with_hierarchy
        let task = make_blocked_task("2", "Blocked task", vec!["1"]);
        let cmds = create_todo_commands(&task, "tasklist-1", "header-1");

        match &cmds[0] {
            FfiCommand::CreateTodo { content, .. } => {
                assert_eq!(content.as_str(), "Blocked task");
            }
            _ => panic!("Expected CreateTodo command"),
        }

        match &cmds[1] {
            FfiCommand::SetTodoMetadata { data, .. } => {
                assert!(data.as_str().contains("\"blocked_by\":[\"1\"]"));
            }
            _ => panic!("Expected SetTodoMetadata command"),
        }
    }

    #[test]
    fn test_update_todo_command() {
        let task = make_test_task("1", "Updated task", "completed");
        let cmd = update_todo_command(&task, "existing-todo-id");

        match cmd {
            FfiCommand::UpdateTodo {
                id, content, state, ..
            } => {
                assert_eq!(id.as_str(), "existing-todo-id");
                assert!(matches!(content, ROption::RSome(ref s) if s.as_str() == "Updated task"));
                assert!(matches!(state, ROption::RSome(FfiTodoState::Checked)));
            }
            _ => panic!("Expected UpdateTodo command"),
        }
    }

    #[test]
    fn test_update_todo_command_blocked() {
        // Update commands use just subject (no emoji prefix)
        let task = make_blocked_task("1", "Now blocked", vec!["2"]);
        let cmd = update_todo_command(&task, "todo-1");

        match cmd {
            FfiCommand::UpdateTodo { content, .. } => {
                assert!(
                    matches!(content, ROption::RSome(ref s) if s.as_str() == "Now blocked")
                );
            }
            _ => panic!("Expected UpdateTodo command"),
        }
    }

    #[test]
    fn test_delete_todo_command() {
        let cmd = delete_todo_command("todo-to-delete");

        match cmd {
            FfiCommand::DeleteTodo { id } => {
                assert_eq!(id.as_str(), "todo-to-delete");
            }
            _ => panic!("Expected DeleteTodo command"),
        }
    }

    #[test]
    fn test_header_id() {
        assert_eq!(header_id("abc"), "claude-header-abc");
    }

    #[test]
    fn test_task_todo_id() {
        assert_eq!(task_todo_id("abc", "1"), "claude-abc-1");
    }

    #[test]
    fn test_metadata_with_multiple_blockers() {
        let task = make_blocked_task("3", "Multi blocked", vec!["1", "2"]);
        let cmds = create_todo_commands(&task, "list-1", "header-1");

        match &cmds[1] {
            FfiCommand::SetTodoMetadata { data, .. } => {
                assert!(data.as_str().contains("\"blocked_by\":[\"1\",\"2\"]"));
            }
            _ => panic!("Expected SetTodoMetadata command"),
        }
    }

    #[test]
    fn test_update_header_command_no_staleness() {
        let cmd = update_header_command("abc-123", Some("MyProject"), None);
        match cmd {
            FfiCommand::UpdateTodo { id, content, .. } => {
                assert_eq!(id.as_str(), "claude-header-abc-123");
                assert!(
                    matches!(content, ROption::RSome(ref s) if s.as_str() == "CLAUDE TASKLIST: MyProject")
                );
            }
            _ => panic!("Expected UpdateTodo"),
        }
    }

    #[test]
    fn test_update_header_command_with_staleness() {
        let cmd = update_header_command("abc-123", Some("MyProject"), Some("23m"));
        match cmd {
            FfiCommand::UpdateTodo { content, .. } => {
                let content_str = match content {
                    ROption::RSome(s) => s.as_str().to_string(),
                    _ => panic!("Expected content"),
                };
                assert!(content_str.contains("STALE"));
                assert!(content_str.contains("23m"));
                assert!(content_str.contains("\u{23F0}")); // alarm clock emoji
            }
            _ => panic!("Expected UpdateTodo"),
        }
    }

    #[test]
    fn test_update_header_command_no_alias() {
        let cmd = update_header_command("abc-123", None, Some("1h5m"));
        match cmd {
            FfiCommand::UpdateTodo { content, .. } => {
                let content_str = match content {
                    ROption::RSome(s) => s.as_str().to_string(),
                    _ => panic!("Expected content"),
                };
                assert!(content_str.contains("abc-123"));
                assert!(content_str.contains("STALE (1h5m)"));
            }
            _ => panic!("Expected UpdateTodo"),
        }
    }
}
