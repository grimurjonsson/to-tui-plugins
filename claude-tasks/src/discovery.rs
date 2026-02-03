//! Tasklist discovery and metadata collection.
//!
//! Discovers Claude Code tasklist folders in `~/.claude/tasks/` and provides
//! metadata about each tasklist including task count, last modified time,
//! and sample task subjects.

use crate::claude_task::ClaudeTask;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Information about a discovered Claude tasklist folder.
#[derive(Debug, Clone)]
pub struct TasklistInfo {
    /// UUID folder name (e.g., "d45035ac-8878-4400-9304-c43d1e9afcbe")
    pub id: String,
    /// Full path to tasklist folder
    pub path: PathBuf,
    /// Number of .json task files
    pub task_count: usize,
    /// Last modified time of the directory
    pub last_modified: SystemTime,
    /// First 3 task subjects (ordered by numeric task id ascending)
    pub sample_tasks: Vec<String>,
}

/// Discover all tasklist folders in `~/.claude/tasks/`.
///
/// Returns an empty Vec if the tasks directory doesn't exist (not an error).
/// Empty tasklists (with no tasks) are excluded.
/// Tasklists are sorted by most recently modified first.
pub fn discover_tasklists() -> Vec<TasklistInfo> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };

    let tasks_dir = home.join(".claude/tasks");

    if !tasks_dir.exists() {
        return vec![];
    }

    let Ok(entries) = std::fs::read_dir(&tasks_dir) else {
        return vec![];
    };

    let mut tasklists = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let tasks = scan_tasks_directory(&path);
        let task_count = tasks.len();

        let last_modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let sample_tasks: Vec<String> = tasks.iter().take(3).map(|t| t.subject.clone()).collect();

        tasklists.push(TasklistInfo {
            id,
            path,
            task_count,
            last_modified,
            sample_tasks,
        });
    }

    // Filter out empty tasklists
    tasklists.retain(|t| t.task_count > 0);

    // Sort by most recently modified first
    tasklists.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    tasklists
}

/// Scan a tasklist directory and return all valid ClaudeTask entries.
///
/// Reads all .json files in the directory, parses each as ClaudeTask,
/// and returns them sorted by numeric id (ascending).
/// Parse failures are silently skipped.
pub fn scan_tasks_directory(path: &Path) -> Vec<ClaudeTask> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return vec![];
    };

    let mut tasks = Vec::new();

    for entry in entries.flatten() {
        let file_path = entry.path();

        // Only process .json files
        let is_json = file_path.extension().map(|e| e == "json").unwrap_or(false);

        if !is_json {
            continue;
        }

        // Try to read and parse, skip failures silently
        let Ok(content) = std::fs::read_to_string(&file_path) else {
            continue;
        };

        let Ok(task) = serde_json::from_str::<ClaudeTask>(&content) else {
            continue;
        };

        tasks.push(task);
    }

    // Sort by numeric ID for consistent ordering
    tasks.sort_by(|a, b| {
        a.id.parse::<u32>()
            .unwrap_or(0)
            .cmp(&b.id.parse::<u32>().unwrap_or(0))
    });

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_task(dir: &Path, id: &str, subject: &str) {
        let task_json = format!(
            r#"{{
            "id": "{}",
            "subject": "{}",
            "description": "Test description",
            "activeForm": "",
            "status": "pending"
        }}"#,
            id, subject
        );
        let path = dir.join(format!("{}.json", id));
        let mut file = fs::File::create(path).unwrap();
        file.write_all(task_json.as_bytes()).unwrap();
    }

    #[test]
    fn test_scan_tasks_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = scan_tasks_directory(temp_dir.path());
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_scan_tasks_directory_with_tasks() {
        let temp_dir = TempDir::new().unwrap();
        create_test_task(temp_dir.path(), "1", "First task");
        create_test_task(temp_dir.path(), "2", "Second task");
        create_test_task(temp_dir.path(), "3", "Third task");

        let tasks = scan_tasks_directory(temp_dir.path());
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[1].id, "2");
        assert_eq!(tasks[2].id, "3");
    }

    #[test]
    fn test_scan_tasks_directory_sorted_numerically() {
        let temp_dir = TempDir::new().unwrap();
        create_test_task(temp_dir.path(), "10", "Tenth task");
        create_test_task(temp_dir.path(), "2", "Second task");
        create_test_task(temp_dir.path(), "1", "First task");

        let tasks = scan_tasks_directory(temp_dir.path());
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[1].id, "2");
        assert_eq!(tasks[2].id, "10");
    }

    #[test]
    fn test_scan_tasks_directory_skips_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        create_test_task(temp_dir.path(), "1", "Valid task");

        // Create invalid JSON file
        let invalid_path = temp_dir.path().join("invalid.json");
        let mut file = fs::File::create(invalid_path).unwrap();
        file.write_all(b"not valid json").unwrap();

        let tasks = scan_tasks_directory(temp_dir.path());
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "1");
    }

    #[test]
    fn test_scan_tasks_directory_skips_non_json() {
        let temp_dir = TempDir::new().unwrap();
        create_test_task(temp_dir.path(), "1", "Valid task");

        // Create non-JSON file
        let txt_path = temp_dir.path().join("notes.txt");
        let mut file = fs::File::create(txt_path).unwrap();
        file.write_all(b"some notes").unwrap();

        let tasks = scan_tasks_directory(temp_dir.path());
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let tasks = scan_tasks_directory(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(tasks.is_empty());
    }
}
