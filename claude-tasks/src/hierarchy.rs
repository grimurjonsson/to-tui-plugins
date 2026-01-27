//! Dependency hierarchy building for Claude tasks.
//!
//! Builds annotations for blocked tasks (flat list, no parent-child nesting).
//! - Any blockers = "(blocked by: Name1, Name2)" annotation
//! - Cycles = warning annotation at root level

use crate::claude_task::ClaudeTask;
use std::collections::{HashMap, HashSet};

/// Computed hierarchy for a set of tasks.
#[derive(Debug, Default)]
pub struct TaskHierarchy {
    /// task_id -> annotation text (blocked by or cycle warning)
    pub annotations: HashMap<String, String>,
    /// Tasks involved in circular dependencies
    pub cyclic_tasks: HashSet<String>,
}

impl TaskHierarchy {
    /// Get annotation for this task (blocked by or cycle warning).
    pub fn get_annotation(&self, task_id: &str) -> Option<&str> {
        self.annotations.get(task_id).map(|s| s.as_str())
    }

    /// Check if task is in a cycle.
    pub fn is_cyclic(&self, task_id: &str) -> bool {
        self.cyclic_tasks.contains(task_id)
    }
}

/// Build hierarchy from a set of tasks.
///
/// Rules:
/// - Any blockers: flat list with "(blocked by: Name1, Name2)" annotation
/// - Circular dependency: warning annotation
pub fn build_hierarchy(tasks: &[ClaudeTask]) -> TaskHierarchy {
    let mut hierarchy = TaskHierarchy::default();

    // Build task lookup map
    let task_map: HashMap<&str, &ClaudeTask> = tasks
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();

    // First pass: detect cycles
    let cyclic = detect_cycles(tasks);
    hierarchy.cyclic_tasks = cyclic;

    // Second pass: build annotations
    for task in tasks {
        // Skip tasks in cycles - they get warning annotation
        if hierarchy.cyclic_tasks.contains(&task.id) {
            hierarchy.annotations.insert(
                task.id.clone(),
                "\u{26A0} Circular dependency".to_string(),
            );
            continue;
        }

        // Any non-empty blocked_by creates annotation
        if !task.blocked_by.is_empty() {
            let names: Vec<&str> = task.blocked_by.iter()
                .filter_map(|id| task_map.get(id.as_str()).map(|t| t.subject.as_str()))
                .collect();

            if !names.is_empty() {
                let annotation = format!("(blocked by: {})", names.join(", "));
                hierarchy.annotations.insert(task.id.clone(), annotation);
            }
        }
    }

    hierarchy
}

/// Detect circular dependencies using DFS with coloring.
///
/// Returns set of task IDs involved in cycles.
fn detect_cycles(tasks: &[ClaudeTask]) -> HashSet<String> {
    let mut cyclic = HashSet::new();
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    // Build adjacency list (task -> tasks it's blocked by)
    let adj: HashMap<&str, Vec<&str>> = tasks
        .iter()
        .map(|t| (t.id.as_str(), t.blocked_by.iter().map(|s| s.as_str()).collect()))
        .collect();

    for task in tasks {
        if !visited.contains(task.id.as_str()) {
            dfs_cycle(&task.id, &adj, &mut visited, &mut in_stack, &mut cyclic);
        }
    }

    cyclic
}

/// DFS to find cycles. Marks all nodes in cycle.
fn dfs_cycle<'a>(
    node: &'a str,
    adj: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    in_stack: &mut HashSet<&'a str>,
    cyclic: &mut HashSet<String>,
) {
    visited.insert(node);
    in_stack.insert(node);

    if let Some(neighbors) = adj.get(node) {
        for &neighbor in neighbors {
            if !visited.contains(neighbor) {
                dfs_cycle(neighbor, adj, visited, in_stack, cyclic);
            } else if in_stack.contains(neighbor) {
                // Cycle detected - mark both nodes
                cyclic.insert(node.to_string());
                cyclic.insert(neighbor.to_string());
            }
        }
    }

    in_stack.remove(node);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str, subject: &str, blocked_by: Vec<&str>) -> ClaudeTask {
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
    fn test_no_dependencies() {
        let tasks = vec![
            make_task("1", "Task A", vec![]),
            make_task("2", "Task B", vec![]),
        ];
        let hierarchy = build_hierarchy(&tasks);

        assert!(hierarchy.get_annotation("1").is_none());
        assert!(hierarchy.get_annotation("2").is_none());
        assert!(hierarchy.annotations.is_empty());
    }

    #[test]
    fn test_single_blocker_creates_annotation() {
        let tasks = vec![
            make_task("1", "Task A", vec![]),
            make_task("2", "Task B", vec!["1"]),
        ];
        let hierarchy = build_hierarchy(&tasks);

        assert!(hierarchy.get_annotation("1").is_none());
        let annotation = hierarchy.get_annotation("2").unwrap();
        assert_eq!(annotation, "(blocked by: Task A)");
    }

    #[test]
    fn test_multiple_blockers_creates_annotation() {
        let tasks = vec![
            make_task("1", "Task A", vec![]),
            make_task("2", "Task B", vec![]),
            make_task("3", "Task C", vec!["1", "2"]),
        ];
        let hierarchy = build_hierarchy(&tasks);

        let annotation = hierarchy.get_annotation("3").unwrap();
        assert_eq!(annotation, "(blocked by: Task A, Task B)");
    }

    #[test]
    fn test_cycle_detection() {
        let tasks = vec![
            make_task("1", "Task A", vec!["2"]),
            make_task("2", "Task B", vec!["1"]),
        ];
        let hierarchy = build_hierarchy(&tasks);

        assert!(hierarchy.is_cyclic("1"));
        assert!(hierarchy.is_cyclic("2"));
        assert!(hierarchy.get_annotation("1").unwrap().contains("Circular"));
    }

    #[test]
    fn test_missing_blocker_ignored() {
        let tasks = vec![
            make_task("1", "Task A", vec!["999"]), // blocker doesn't exist
        ];
        let hierarchy = build_hierarchy(&tasks);

        // Should not crash, no annotation created (blocker not found in task_map)
        assert!(hierarchy.get_annotation("1").is_none());
    }
}
