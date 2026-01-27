---
phase: quick-001
plan: 01
subsystem: sync
tags: [hierarchy, ux, refactor]

dependency-graph:
  requires: []
  provides:
    - flat-task-list
    - blocked-by-annotation
  affects: []

tech-stack:
  added: []
  patterns:
    - annotation-only-hierarchy

key-files:
  created: []
  modified:
    - claude-tasks/src/hierarchy.rs
    - claude-tasks/src/commands.rs
    - claude-tasks/src/sync.rs

decisions:
  - id: flat-list
    choice: "All tasks at same indent level under header"
    rationale: "Parent-child nesting looked like subtasks, confusing users"
  - id: annotation-format
    choice: "(blocked by: Task A, Task B) suffix"
    rationale: "Lowercase, parentheses, clear indication without emoji prefix"

metrics:
  duration: 3m
  completed: 2026-01-28
---

# Quick Task 001: Flatten Dependency Hierarchy Summary

**One-liner:** Replaced parent-child nesting with flat list and "(blocked by: X)" annotations.

## Objective

Replace confusing parent-child task nesting with a flat list where blocked tasks show "(blocked by: X, Y)" annotation.

## What Was Built

### hierarchy.rs - Annotations-Only Model

Simplified the TaskHierarchy struct to only track annotations:
- Removed `parent_map` (no more parent-child relationships)
- Removed `get_parent()` method
- Removed `calculate_depth()` and `build_chain_annotation()` helpers
- Removed `MAX_DEPTH` constant
- Single blocker now creates annotation instead of parent relationship
- Annotation format: `(blocked by: Task A)` or `(blocked by: Task A, Task B)`

### commands.rs - Flat Structure

Updated `create_todo_commands_with_hierarchy()`:
- Always use header as parent (all tasks at root level)
- Always use `indent_level = 1` (flat list)
- Blocked annotation appended as suffix: `"Subject (blocked by: X)"`
- Removed blocked emoji prefix from `format_task_content()`

### sync.rs - Simplified Comparison

Updated `needs_update()`:
- Compare just subject (no emoji prefix to check)
- Blocked annotation only applied on create, not used in update comparison

## Verification

- `cargo test -p claude-tasks` - 109 tests pass
- `cargo build -p claude-tasks --release` - compiles successfully
- Manual inspection confirms:
  - hierarchy.rs has no parent_map
  - commands.rs always uses indent_level 1
  - All tasks flat under header

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 950ffd7 | refactor(quick-001): simplify hierarchy to annotations-only |
| 2 | 185391f | refactor(quick-001): flatten todo structure in commands |
| 3 | f8207d9 | refactor(quick-001): update sync needs_update for flat model |

## Deviations from Plan

None - plan executed exactly as written.

## Before/After

**Before (confusing subtask appearance):**
```
CLAUDE TASKLIST: My Project
  Task A (blocker)
    Task B (looks like subtask of A)
    Task C (looks like subtask of A)
```

**After (flat list with clear annotations):**
```
CLAUDE TASKLIST: My Project
  Task A
  Task B (blocked by: Task A)
  Task C (blocked by: Task A)
```
