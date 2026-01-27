//! Guidance module for in-context help as todos.
//!
//! Provides functions to create guidance todos that explain plugin state
//! to users. Guidance is displayed when:
//! - No tasklists exist (setup instructions)
//! - Tasklist is empty (waiting for tasks)
//! - Errors occur (recovery guidance)

use abi_stable::std_types::{ROption, RString};
use totui_plugin_interface::{FfiCommand, FfiTodoState};

// ============================================================================
// Guidance Todo IDs (for consistent lifecycle management)
// ============================================================================

/// ID for the guidance header todo (setup required state)
pub const GUIDANCE_HEADER_ID: &str = "claude-guidance-header";
/// ID for "no tasklists found" guidance todo
pub const GUIDANCE_NO_TASKLIST_ID: &str = "claude-guidance-no-tasklist";
/// ID for "start Claude Code" guidance todo
pub const GUIDANCE_START_CLAUDE_ID: &str = "claude-guidance-start-claude";
/// ID for "no tasks yet" guidance todo
pub const GUIDANCE_NO_TASKS_ID: &str = "claude-guidance-no-tasks";
/// ID for "tasks will appear" guidance todo
pub const GUIDANCE_TASKS_WILL_APPEAR_ID: &str = "claude-guidance-tasks-appear";
/// ID for the waiting header todo (empty tasklist state)
pub const GUIDANCE_WAITING_HEADER_ID: &str = "claude-guidance-waiting-header";
/// ID for error header todo
pub const GUIDANCE_ERROR_HEADER_ID: &str = "claude-error-header";
/// ID for error detail todo
pub const GUIDANCE_ERROR_DETAIL_ID: &str = "claude-error-detail";
/// ID for error action todo
pub const GUIDANCE_ERROR_ACTION_ID: &str = "claude-error-action";

/// All guidance IDs for clearing
pub const GUIDANCE_IDS: &[&str] = &[
    GUIDANCE_HEADER_ID,
    GUIDANCE_NO_TASKLIST_ID,
    GUIDANCE_START_CLAUDE_ID,
    GUIDANCE_NO_TASKS_ID,
    GUIDANCE_TASKS_WILL_APPEAR_ID,
    GUIDANCE_WAITING_HEADER_ID,
    GUIDANCE_ERROR_HEADER_ID,
    GUIDANCE_ERROR_DETAIL_ID,
    GUIDANCE_ERROR_ACTION_ID,
];

// ============================================================================
// Message Constants (centralized text)
// ============================================================================

/// Header text for setup required state
pub const MSG_HEADER_SETUP_REQUIRED: &str = "CLAUDE TASKS - Setup Required";
/// Message when no tasklists are found
pub const MSG_NO_TASKLISTS: &str = "No Claude tasklists found in ~/.claude/tasks/";
/// Message instructing to start Claude Code
pub const MSG_START_CLAUDE: &str = "Start a Claude Code session to create a tasklist";
/// Message when tasklist has no tasks yet
pub const MSG_NO_TASKS_YET: &str = "Claude hasn't created any tasks yet";
/// Message that tasks will appear
pub const MSG_TASKS_WILL_APPEAR: &str = "Tasks will appear here as Claude works";

// ============================================================================
// Guidance Creation Functions
// ============================================================================

/// Create guidance todos for "no tasklists exist" state.
///
/// Displays:
/// - Header: "CLAUDE TASKS - Setup Required" (Question state)
/// - Child: "No Claude tasklists found in ~/.claude/tasks/"
/// - Child: "Start a Claude Code session to create a tasklist"
///
/// Returns 4 commands: 3 CreateTodo + 1 SetTodoMetadata
#[allow(clippy::vec_init_then_push)]
pub fn create_no_tasklist_guidance() -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    // Header todo with Question state
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(MSG_HEADER_SETUP_REQUIRED),
        parent_id: ROption::RNone,
        temp_id: ROption::RSome(RString::from(GUIDANCE_HEADER_ID)),
        state: FfiTodoState::Question,
        priority: ROption::RNone,
        indent_level: 0,
    });

    // Child: No tasklists found
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(MSG_NO_TASKLISTS),
        parent_id: ROption::RSome(RString::from(GUIDANCE_HEADER_ID)),
        temp_id: ROption::RSome(RString::from(GUIDANCE_NO_TASKLIST_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 1,
    });

    // Child: Start Claude Code
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(MSG_START_CLAUDE),
        parent_id: ROption::RSome(RString::from(GUIDANCE_HEADER_ID)),
        temp_id: ROption::RSome(RString::from(GUIDANCE_START_CLAUDE_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 1,
    });

    // Set metadata on header
    commands.push(FfiCommand::SetTodoMetadata {
        todo_id: RString::from(GUIDANCE_HEADER_ID),
        data: RString::from(r#"{"source":"claude-tasks","type":"guidance"}"#),
        merge: false,
    });

    commands
}

/// Create guidance todos for "empty tasklist" state.
///
/// Displays:
/// - Header: "CLAUDE TASKLIST: {display_name} - Waiting for tasks"
/// - Child: "Claude hasn't created any tasks yet"
/// - Child: "Tasks will appear here as Claude works"
///
/// Returns 4 commands: 3 CreateTodo + 1 SetTodoMetadata
#[allow(clippy::vec_init_then_push)]
pub fn create_empty_tasklist_guidance(display_name: &str) -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    let header_content = format!("CLAUDE TASKLIST: {} - Waiting for tasks", display_name);

    // Header todo
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(header_content),
        parent_id: ROption::RNone,
        temp_id: ROption::RSome(RString::from(GUIDANCE_WAITING_HEADER_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 0,
    });

    // Child: No tasks yet
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(MSG_NO_TASKS_YET),
        parent_id: ROption::RSome(RString::from(GUIDANCE_WAITING_HEADER_ID)),
        temp_id: ROption::RSome(RString::from(GUIDANCE_NO_TASKS_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 1,
    });

    // Child: Tasks will appear
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(MSG_TASKS_WILL_APPEAR),
        parent_id: ROption::RSome(RString::from(GUIDANCE_WAITING_HEADER_ID)),
        temp_id: ROption::RSome(RString::from(GUIDANCE_TASKS_WILL_APPEAR_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 1,
    });

    // Set metadata on header
    commands.push(FfiCommand::SetTodoMetadata {
        todo_id: RString::from(GUIDANCE_WAITING_HEADER_ID),
        data: RString::from(r#"{"source":"claude-tasks","type":"guidance"}"#),
        merge: false,
    });

    commands
}

/// Create guidance todos for error state with recovery instructions.
///
/// Displays:
/// - Header: {title} (Exclamation state)
/// - Child: {explanation}
/// - Child: "Action: {action}"
///
/// Returns 4 commands: 3 CreateTodo + 1 SetTodoMetadata
#[allow(clippy::vec_init_then_push)]
pub fn create_error_guidance(title: &str, explanation: &str, action: &str) -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    // Header todo with Exclamation state
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(title),
        parent_id: ROption::RNone,
        temp_id: ROption::RSome(RString::from(GUIDANCE_ERROR_HEADER_ID)),
        state: FfiTodoState::Exclamation,
        priority: ROption::RNone,
        indent_level: 0,
    });

    // Child: Explanation
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(explanation),
        parent_id: ROption::RSome(RString::from(GUIDANCE_ERROR_HEADER_ID)),
        temp_id: ROption::RSome(RString::from(GUIDANCE_ERROR_DETAIL_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 1,
    });

    // Child: Action
    let action_content = format!("Action: {}", action);
    commands.push(FfiCommand::CreateTodo {
        content: RString::from(action_content),
        parent_id: ROption::RSome(RString::from(GUIDANCE_ERROR_HEADER_ID)),
        temp_id: ROption::RSome(RString::from(GUIDANCE_ERROR_ACTION_ID)),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 1,
    });

    // Set metadata on header with error flag
    commands.push(FfiCommand::SetTodoMetadata {
        todo_id: RString::from(GUIDANCE_ERROR_HEADER_ID),
        data: RString::from(r#"{"source":"claude-tasks","type":"guidance","error":true}"#),
        merge: false,
    });

    commands
}

/// Clear all guidance todos.
///
/// Returns DeleteTodo commands for all guidance IDs.
/// Safe to call even if some IDs don't exist (totui ignores non-existent deletes).
pub fn clear_guidance() -> Vec<FfiCommand> {
    GUIDANCE_IDS
        .iter()
        .map(|id| FfiCommand::DeleteTodo {
            id: RString::from(*id),
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guidance_ids_count() {
        assert_eq!(GUIDANCE_IDS.len(), 9);
    }

    #[test]
    fn test_create_no_tasklist_guidance_command_count() {
        let commands = create_no_tasklist_guidance();
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn test_create_no_tasklist_guidance_header() {
        let commands = create_no_tasklist_guidance();

        match &commands[0] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                state,
                indent_level,
                parent_id,
                ..
            } => {
                assert_eq!(content.as_str(), MSG_HEADER_SETUP_REQUIRED);
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_HEADER_ID)
                );
                assert!(matches!(state, FfiTodoState::Question));
                assert_eq!(*indent_level, 0);
                assert!(matches!(parent_id, ROption::RNone));
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_no_tasklist_guidance_children() {
        let commands = create_no_tasklist_guidance();

        // Child 1: No tasklists found
        match &commands[1] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                parent_id,
                indent_level,
                ..
            } => {
                assert_eq!(content.as_str(), MSG_NO_TASKLISTS);
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_NO_TASKLIST_ID)
                );
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_HEADER_ID)
                );
                assert_eq!(*indent_level, 1);
            }
            _ => panic!("Expected CreateTodo command"),
        }

        // Child 2: Start Claude Code
        match &commands[2] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                parent_id,
                indent_level,
                ..
            } => {
                assert_eq!(content.as_str(), MSG_START_CLAUDE);
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_START_CLAUDE_ID)
                );
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_HEADER_ID)
                );
                assert_eq!(*indent_level, 1);
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_no_tasklist_guidance_metadata() {
        let commands = create_no_tasklist_guidance();

        match &commands[3] {
            FfiCommand::SetTodoMetadata {
                todo_id,
                data,
                merge,
            } => {
                assert_eq!(todo_id.as_str(), GUIDANCE_HEADER_ID);
                assert!(data.as_str().contains(r#""source":"claude-tasks""#));
                assert!(data.as_str().contains(r#""type":"guidance""#));
                assert!(!merge);
            }
            _ => panic!("Expected SetTodoMetadata command"),
        }
    }

    #[test]
    fn test_create_empty_tasklist_guidance_command_count() {
        let commands = create_empty_tasklist_guidance("MyProject");
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn test_create_empty_tasklist_guidance_header() {
        let commands = create_empty_tasklist_guidance("MyProject");

        match &commands[0] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                state,
                indent_level,
                ..
            } => {
                assert_eq!(
                    content.as_str(),
                    "CLAUDE TASKLIST: MyProject - Waiting for tasks"
                );
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_WAITING_HEADER_ID)
                );
                assert!(matches!(state, FfiTodoState::Empty));
                assert_eq!(*indent_level, 0);
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_empty_tasklist_guidance_children() {
        let commands = create_empty_tasklist_guidance("TestList");

        // Child 1: No tasks yet
        match &commands[1] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                parent_id,
                ..
            } => {
                assert_eq!(content.as_str(), MSG_NO_TASKS_YET);
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_NO_TASKS_ID)
                );
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_WAITING_HEADER_ID)
                );
            }
            _ => panic!("Expected CreateTodo command"),
        }

        // Child 2: Tasks will appear
        match &commands[2] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                parent_id,
                ..
            } => {
                assert_eq!(content.as_str(), MSG_TASKS_WILL_APPEAR);
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_TASKS_WILL_APPEAR_ID)
                );
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_WAITING_HEADER_ID)
                );
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_error_guidance_command_count() {
        let commands = create_error_guidance("Error Title", "Error explanation", "Restart plugin");
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn test_create_error_guidance_header() {
        let commands = create_error_guidance("Watcher Failed", "File system error", "Check path");

        match &commands[0] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                state,
                indent_level,
                ..
            } => {
                assert_eq!(content.as_str(), "Watcher Failed");
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_ERROR_HEADER_ID)
                );
                assert!(matches!(state, FfiTodoState::Exclamation));
                assert_eq!(*indent_level, 0);
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_error_guidance_children() {
        let commands =
            create_error_guidance("Error", "Something went wrong", "Try restarting totui");

        // Child 1: Explanation
        match &commands[1] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                parent_id,
                ..
            } => {
                assert_eq!(content.as_str(), "Something went wrong");
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_ERROR_DETAIL_ID)
                );
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_ERROR_HEADER_ID)
                );
            }
            _ => panic!("Expected CreateTodo command"),
        }

        // Child 2: Action
        match &commands[2] {
            FfiCommand::CreateTodo {
                content,
                temp_id,
                parent_id,
                ..
            } => {
                assert_eq!(content.as_str(), "Action: Try restarting totui");
                assert!(
                    matches!(temp_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_ERROR_ACTION_ID)
                );
                assert!(
                    matches!(parent_id, ROption::RSome(ref s) if s.as_str() == GUIDANCE_ERROR_HEADER_ID)
                );
            }
            _ => panic!("Expected CreateTodo command"),
        }
    }

    #[test]
    fn test_create_error_guidance_metadata() {
        let commands = create_error_guidance("Error", "Detail", "Action");

        match &commands[3] {
            FfiCommand::SetTodoMetadata {
                todo_id,
                data,
                merge,
            } => {
                assert_eq!(todo_id.as_str(), GUIDANCE_ERROR_HEADER_ID);
                assert!(data.as_str().contains(r#""source":"claude-tasks""#));
                assert!(data.as_str().contains(r#""type":"guidance""#));
                assert!(data.as_str().contains(r#""error":true"#));
                assert!(!merge);
            }
            _ => panic!("Expected SetTodoMetadata command"),
        }
    }

    #[test]
    fn test_clear_guidance_command_count() {
        let commands = clear_guidance();
        assert_eq!(commands.len(), GUIDANCE_IDS.len());
    }

    #[test]
    fn test_clear_guidance_deletes_all_ids() {
        let commands = clear_guidance();

        for (i, cmd) in commands.iter().enumerate() {
            match cmd {
                FfiCommand::DeleteTodo { id } => {
                    assert_eq!(id.as_str(), GUIDANCE_IDS[i]);
                }
                _ => panic!("Expected DeleteTodo command"),
            }
        }
    }

    #[test]
    fn test_message_constants() {
        assert!(!MSG_HEADER_SETUP_REQUIRED.is_empty());
        assert!(!MSG_NO_TASKLISTS.is_empty());
        assert!(!MSG_START_CLAUDE.is_empty());
        assert!(!MSG_NO_TASKS_YET.is_empty());
        assert!(!MSG_TASKS_WILL_APPEAR.is_empty());
    }

    #[test]
    fn test_all_guidance_ids_unique() {
        let mut seen = std::collections::HashSet::new();
        for id in GUIDANCE_IDS {
            assert!(seen.insert(*id), "Duplicate guidance ID: {}", id);
        }
    }
}
