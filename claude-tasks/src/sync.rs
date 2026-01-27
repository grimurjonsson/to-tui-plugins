//! Sync reconciliation logic between Claude tasks and totui todos.
//!
//! Handles initial sync, file change processing, and file removal processing.
//! Uses metadata-based correlation to track which todos came from which Claude tasks.

use crate::claude_task::{map_status_to_state, ClaudeTask};
use crate::commands::{
    create_header_command, create_todo_commands, create_todo_commands_with_hierarchy,
    delete_todo_command, header_id, task_todo_id, update_todo_command,
};
use crate::discovery::scan_tasks_directory;
use crate::hierarchy::build_hierarchy;
use abi_stable::std_types::RBox;
use std::collections::HashMap;
use std::path::Path;
use totui_plugin_interface::{FfiCommand, FfiTodoItem, HostApi_TO};

/// Perform initial sync between Claude tasks and totui todos.
///
/// This is called when the plugin starts watching a tasklist:
/// 1. Scan all Claude task files in the directory
/// 2. Query existing synced todos from totui
/// 3. Create header if not exists
/// 4. For each Claude task: create or update corresponding todo
/// 5. For orphaned todos (no matching Claude task): delete
///
/// Returns a list of FfiCommands to execute.
pub fn initial_sync(
    host: &HostApi_TO<'_, RBox<()>>,
    tasklist_path: &Path,
    tasklist_id: &str,
) -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    // Read all Claude tasks from the directory
    let claude_tasks = scan_tasks_directory(tasklist_path);

    // Query existing synced todos
    let existing_todos = host.query_todos_by_metadata("source".into(), "\"claude-tasks\"".into());

    // Build map of existing todos by task_id (filter to this tasklist)
    let mut existing_by_task_id: HashMap<String, FfiTodoItem> = HashMap::new();
    let mut header_exists = false;

    for todo in existing_todos {
        let metadata = host.get_todo_metadata(todo.id.clone());
        let metadata_str = metadata.as_str();

        // Check if this todo belongs to our tasklist
        if !metadata_str.contains(&format!("\"tasklist_id\":\"{}\"", tasklist_id)) {
            continue;
        }

        // Check if this is the header
        if todo.id.as_str() == header_id(tasklist_id) {
            header_exists = true;
            continue;
        }

        // Extract task_id from metadata
        if let Some(task_id) = extract_task_id_from_metadata(metadata_str) {
            existing_by_task_id.insert(task_id, todo);
        }
    }

    // Create header if it doesn't exist
    let hdr_id = header_id(tasklist_id);
    if !header_exists {
        commands.push(create_header_command(tasklist_id, None));
    }

    // Track which task IDs we've seen (to detect orphans)
    let mut seen_task_ids: Vec<String> = Vec::new();

    // Process each Claude task
    for task in &claude_tasks {
        seen_task_ids.push(task.id.clone());

        if let Some(existing_todo) = existing_by_task_id.get(&task.id) {
            // Check if update needed
            if needs_update(task, existing_todo) {
                commands.push(update_todo_command(task, existing_todo.id.as_str()));
            }
        } else {
            // Create new todo
            commands.extend(create_todo_commands(task, tasklist_id, &hdr_id));
        }
    }

    // Delete orphaned todos (exist in totui but not in Claude tasks)
    for (task_id, todo) in &existing_by_task_id {
        if !seen_task_ids.contains(task_id) {
            commands.push(delete_todo_command(todo.id.as_str()));
        }
    }

    commands
}

/// Process a file change event (create or modify).
///
/// Reads the changed task file and either creates a new todo or updates
/// the existing one.
pub fn process_file_change(
    host: &HostApi_TO<'_, RBox<()>>,
    file_path: &Path,
    tasklist_id: &str,
) -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    // Extract task_id from filename (e.g., "1.json" -> "1")
    let Some(task_id) = extract_task_id_from_path(file_path) else {
        return commands;
    };

    // Read and parse the task file
    let Ok(content) = std::fs::read_to_string(file_path) else {
        return commands;
    };

    let Ok(task) = serde_json::from_str::<ClaudeTask>(&content) else {
        return commands;
    };

    // Look for existing todo with this task_id
    let existing_todo = find_todo_by_task_id(host, tasklist_id, &task_id);

    let hdr_id = header_id(tasklist_id);

    if let Some(todo) = existing_todo {
        // Update if changed
        if needs_update(&task, &todo) {
            commands.push(update_todo_command(&task, todo.id.as_str()));
        }
    } else {
        // Create new todo
        commands.extend(create_todo_commands(&task, tasklist_id, &hdr_id));
    }

    commands
}

/// Process a file removal event.
///
/// Finds the corresponding todo and deletes it.
pub fn process_file_removal(
    host: &HostApi_TO<'_, RBox<()>>,
    file_path: &Path,
    tasklist_id: &str,
) -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    // Extract task_id from filename
    let Some(task_id) = extract_task_id_from_path(file_path) else {
        return commands;
    };

    // Find existing todo with this task_id
    if let Some(todo) = find_todo_by_task_id(host, tasklist_id, &task_id) {
        commands.push(delete_todo_command(todo.id.as_str()));
    }

    commands
}

/// Check if a todo needs to be updated based on the Claude task.
///
/// Compares content (subject only) and state (status).
/// Blocked annotation is only applied on create via hierarchy, not stored for comparison.
pub fn needs_update(task: &ClaudeTask, existing: &FfiTodoItem) -> bool {
    // Check state
    let expected_state = map_status_to_state(&task.status);
    if existing.state != expected_state {
        return true;
    }

    // Check content - just subject (annotation is only on create)
    if existing.content.as_str() != task.subject {
        return true;
    }

    false
}

/// Extract task_id from file path.
///
/// Example: "/path/to/1.json" -> Some("1")
pub fn extract_task_id_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Extract task_id from metadata JSON string.
///
/// Looks for "task_id":"<value>" pattern.
pub fn extract_task_id_from_metadata(metadata: &str) -> Option<String> {
    // Simple JSON parsing - look for "task_id":"<value>"
    let key = "\"task_id\":\"";
    let start = metadata.find(key)? + key.len();
    let end = metadata[start..].find('"')? + start;
    Some(metadata[start..end].to_string())
}

/// Find a todo by task_id and tasklist_id.
fn find_todo_by_task_id(
    host: &HostApi_TO<'_, RBox<()>>,
    tasklist_id: &str,
    task_id: &str,
) -> Option<FfiTodoItem> {
    // Query by task_id
    let todos = host.query_todos_by_metadata("task_id".into(), format!("\"{}\"", task_id).into());

    // Filter to the correct tasklist
    for todo in todos {
        let metadata = host.get_todo_metadata(todo.id.clone());
        if metadata
            .as_str()
            .contains(&format!("\"tasklist_id\":\"{}\"", tasklist_id))
        {
            return Some(todo);
        }
    }

    None
}

// ============================================================================
// HostApi-free sync functions (for on_event which lacks HostApi access)
// ============================================================================

/// Process initial scan using local state tracking (no HostApi needed).
///
/// Reads all Claude task files and generates create commands with hierarchy.
/// Tracks created tasks in known_tasks set.
/// Returns commands and set of task_ids found.
///
/// If `alias` is provided, the header will display the alias instead of the UUID.
///
/// Hierarchy rules:
/// - Single blocker within depth limit: task becomes child of blocker
/// - Multiple blockers: task at root with "Blocked by: A, B" annotation
/// - Circular dependency: task at root with cycle warning
/// - Depth > 3: flatten to root with chain annotation
pub fn process_initial_scan_local(
    tasklist_path: &Path,
    tasklist_id: &str,
    alias: Option<&str>,
) -> (Vec<FfiCommand>, Vec<String>) {
    let mut commands = Vec::new();
    let mut task_ids = Vec::new();

    // Read all Claude tasks from directory
    let claude_tasks = scan_tasks_directory(tasklist_path);

    // Build dependency hierarchy
    let hierarchy = build_hierarchy(&claude_tasks);

    // Create header with optional alias
    let hdr_id = header_id(tasklist_id);
    commands.push(create_header_command(tasklist_id, alias));

    // Create todos for all tasks using hierarchy-aware command builder
    for task in &claude_tasks {
        task_ids.push(task.id.clone());
        commands.extend(create_todo_commands_with_hierarchy(
            task, tasklist_id, &hdr_id, &hierarchy,
        ));
    }

    (commands, task_ids)
}

/// Process file change using local state tracking (no HostApi needed).
///
/// If task_id is known, generates update command.
/// If task_id is new, generates create commands.
/// Returns (commands, task_id).
pub fn process_file_change_local(
    file_path: &Path,
    tasklist_id: &str,
    is_known: bool,
) -> Option<(Vec<FfiCommand>, String)> {
    // Extract task_id from filename
    let task_id = extract_task_id_from_path(file_path)?;

    // Read and parse task file
    let content = std::fs::read_to_string(file_path).ok()?;
    let task: ClaudeTask = serde_json::from_str(&content).ok()?;

    let mut commands = Vec::new();
    let hdr_id = header_id(tasklist_id);

    if is_known {
        // Update existing - use predictable todo ID
        let todo_id = task_todo_id(tasklist_id, &task_id);
        commands.push(update_todo_command(&task, &todo_id));
    } else {
        // Create new
        commands.extend(create_todo_commands(&task, tasklist_id, &hdr_id));
    }

    Some((commands, task_id))
}

/// Process file removal using predictable todo ID (no HostApi needed).
///
/// Returns (command, task_id) if file was a task file.
pub fn process_file_removal_local(
    file_path: &Path,
    tasklist_id: &str,
) -> Option<(FfiCommand, String)> {
    let task_id = extract_task_id_from_path(file_path)?;
    let todo_id = task_todo_id(tasklist_id, &task_id);
    Some((delete_todo_command(&todo_id), task_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::ROption;
    use totui_plugin_interface::FfiTodoState;

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

    fn make_test_todo(id: &str, content: &str, state: FfiTodoState) -> FfiTodoItem {
        FfiTodoItem {
            id: id.into(),
            content: content.into(),
            state,
            priority: ROption::RNone,
            due_date: ROption::RNone,
            description: ROption::RNone,
            parent_id: ROption::RNone,
            indent_level: 1,
            created_at: 0,
            modified_at: 0,
            completed_at: ROption::RNone,
            position: 0,
        }
    }

    #[test]
    fn test_extract_task_id_from_path() {
        assert_eq!(
            extract_task_id_from_path(Path::new("/path/to/1.json")),
            Some("1".to_string())
        );
        assert_eq!(
            extract_task_id_from_path(Path::new("/path/to/123.json")),
            Some("123".to_string())
        );
        assert_eq!(
            extract_task_id_from_path(Path::new("task.json")),
            Some("task".to_string())
        );
    }

    #[test]
    fn test_extract_task_id_from_metadata() {
        let metadata = r#"{"source":"claude-tasks","task_id":"1","read_only":true}"#;
        assert_eq!(
            extract_task_id_from_metadata(metadata),
            Some("1".to_string())
        );

        let metadata2 = r#"{"task_id":"123","tasklist_id":"abc"}"#;
        assert_eq!(
            extract_task_id_from_metadata(metadata2),
            Some("123".to_string())
        );

        assert_eq!(extract_task_id_from_metadata("{}"), None);
        assert_eq!(extract_task_id_from_metadata("invalid"), None);
    }

    #[test]
    fn test_needs_update_no_change() {
        let task = make_test_task("1", "Test task", "pending");
        let todo = make_test_todo("todo-1", "Test task", FfiTodoState::Empty);

        assert!(!needs_update(&task, &todo));
    }

    #[test]
    fn test_needs_update_state_changed() {
        let task = make_test_task("1", "Test task", "completed");
        let todo = make_test_todo("todo-1", "Test task", FfiTodoState::Empty);

        assert!(needs_update(&task, &todo));
    }

    #[test]
    fn test_needs_update_content_changed() {
        let task = make_test_task("1", "Updated task", "pending");
        let todo = make_test_todo("todo-1", "Old task", FfiTodoState::Empty);

        assert!(needs_update(&task, &todo));
    }

    #[test]
    fn test_needs_update_blocked_status_changed() {
        // Task was unblocked, now blocked - but content comparison is just subject
        // So if subject matches, no update needed (annotation is only on create)
        let task = make_blocked_task("1", "Test task", vec!["2"]);
        let todo = make_test_todo("todo-1", "Test task", FfiTodoState::Empty);

        // Subject matches, state matches - no update needed
        assert!(!needs_update(&task, &todo));
    }

    #[test]
    fn test_needs_update_blocked_content_matches() {
        // Blocked task with subject matching todo content
        let task = make_blocked_task("1", "Blocked task", vec!["2"]);
        let todo = make_test_todo("todo-1", "Blocked task", FfiTodoState::Empty);

        assert!(!needs_update(&task, &todo));
    }

    #[test]
    fn test_needs_update_in_progress() {
        let task = make_test_task("1", "Working", "in_progress");
        let todo = make_test_todo("todo-1", "Working", FfiTodoState::InProgress);

        assert!(!needs_update(&task, &todo));
    }

    #[test]
    fn test_needs_update_in_progress_mismatch() {
        let task = make_test_task("1", "Working", "in_progress");
        let todo = make_test_todo("todo-1", "Working", FfiTodoState::Empty);

        assert!(needs_update(&task, &todo));
    }

    // ========================================================================
    // Tests for HostApi-free sync functions
    // ========================================================================

    #[test]
    fn test_process_initial_scan_local_empty_dir() {
        // Create temp dir with no tasks
        let dir = tempfile::tempdir().unwrap();
        let (cmds, task_ids) = process_initial_scan_local(dir.path(), "test-list", None);

        // Should have header command only, no tasks
        assert_eq!(cmds.len(), 1); // just header
        assert!(task_ids.is_empty());
        match &cmds[0] {
            FfiCommand::CreateTodo { content, .. } => {
                assert!(content.as_str().contains("CLAUDE TASKLIST"));
            }
            _ => panic!("Expected CreateTodo for header"),
        }
    }

    #[test]
    fn test_process_initial_scan_local_with_tasks() {
        let dir = tempfile::tempdir().unwrap();
        // Create test task file
        let task = ClaudeTask {
            id: "1".to_string(),
            subject: "Test".to_string(),
            description: String::new(),
            active_form: String::new(),
            status: "pending".to_string(),
            blocks: vec![],
            blocked_by: vec![],
        };
        std::fs::write(
            dir.path().join("1.json"),
            serde_json::to_string(&task).unwrap(),
        )
        .unwrap();

        let (cmds, task_ids) = process_initial_scan_local(dir.path(), "test-list", None);

        // Should have header + create + metadata commands
        assert!(cmds.len() >= 2); // header + at least create
        assert_eq!(task_ids, vec!["1"]);
    }

    #[test]
    fn test_process_file_change_local_known_task() {
        let dir = tempfile::tempdir().unwrap();
        let task_path = dir.path().join("1.json");
        let task = ClaudeTask {
            id: "1".to_string(),
            subject: "Updated task".to_string(),
            description: String::new(),
            active_form: String::new(),
            status: "completed".to_string(),
            blocks: vec![],
            blocked_by: vec![],
        };
        std::fs::write(&task_path, serde_json::to_string(&task).unwrap()).unwrap();

        let result = process_file_change_local(&task_path, "tasklist-1", true);
        let (cmds, task_id) = result.unwrap();

        assert_eq!(task_id, "1");
        assert_eq!(cmds.len(), 1); // single update command
        match &cmds[0] {
            FfiCommand::UpdateTodo { id, .. } => {
                assert_eq!(id.as_str(), "claude-tasklist-1-1");
            }
            _ => panic!("Expected UpdateTodo for known task"),
        }
    }

    #[test]
    fn test_process_file_change_local_unknown_task() {
        let dir = tempfile::tempdir().unwrap();
        let task_path = dir.path().join("2.json");
        let task = ClaudeTask {
            id: "2".to_string(),
            subject: "New task".to_string(),
            description: String::new(),
            active_form: String::new(),
            status: "pending".to_string(),
            blocks: vec![],
            blocked_by: vec![],
        };
        std::fs::write(&task_path, serde_json::to_string(&task).unwrap()).unwrap();

        let result = process_file_change_local(&task_path, "tasklist-1", false);
        let (cmds, task_id) = result.unwrap();

        assert_eq!(task_id, "2");
        assert_eq!(cmds.len(), 2); // create + metadata commands
        match &cmds[0] {
            FfiCommand::CreateTodo { temp_id, .. } => {
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == "claude-tasklist-1-2")
                );
            }
            _ => panic!("Expected CreateTodo for unknown task"),
        }
    }

    #[test]
    fn test_process_file_removal_local() {
        let result = process_file_removal_local(Path::new("/path/to/1.json"), "tasklist-1");

        let (cmd, task_id) = result.unwrap();
        assert_eq!(task_id, "1");
        match cmd {
            FfiCommand::DeleteTodo { id } => {
                assert_eq!(id.as_str(), "claude-tasklist-1-1");
            }
            _ => panic!("Expected DeleteTodo"),
        }
    }

    #[test]
    fn test_process_file_removal_local_no_extension() {
        // File without extension (no stem to extract) - uses directory name as stem
        let result = process_file_removal_local(Path::new("/"), "tasklist-1");
        assert!(result.is_none());
    }
}
