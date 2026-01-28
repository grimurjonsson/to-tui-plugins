---
phase: quick-001
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - claude-tasks/src/hierarchy.rs
  - claude-tasks/src/commands.rs
  - claude-tasks/src/sync.rs
autonomous: true

must_haves:
  truths:
    - "All tasks appear at same indent level (flat list under header)"
    - "Blocked tasks show '(blocked by TaskA, TaskB)' annotation in grey"
    - "Tasks with cycles still show warning annotation"
  artifacts:
    - path: "claude-tasks/src/hierarchy.rs"
      provides: "Flat hierarchy with annotations only (no parent_map)"
    - path: "claude-tasks/src/commands.rs"
      provides: "create_todo_commands_with_hierarchy uses flat parent_id always"
  key_links:
    - from: "hierarchy.rs"
      to: "commands.rs"
      via: "TaskHierarchy.get_annotation()"
      pattern: "hierarchy\\.get_annotation"
---

<objective>
Flatten dependency hierarchy - replace parent-child nesting with flat list and "(blocked by...)" grey annotations.

Purpose: Currently when task A blocks tasks B and C, B and C appear as children of A (nested). This looks like subtasks, which is confusing. Instead, show a flat list where all tasks are at the same level, and blocked tasks have a "(blocked by A)" annotation.

Output: Modified hierarchy.rs and commands.rs that produce flat todo lists with blocked-by annotations instead of parent-child nesting.
</objective>

<execution_context>
@/Users/gimmi/.claude/get-shit-done/workflows/execute-plan.md
</execution_context>

<context>
@claude-tasks/src/hierarchy.rs
@claude-tasks/src/commands.rs
@claude-tasks/src/sync.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Simplify hierarchy.rs to annotations-only</name>
  <files>claude-tasks/src/hierarchy.rs</files>
  <action>
Modify `hierarchy.rs` to remove parent-child relationships:

1. Remove `parent_map` from `TaskHierarchy` struct
2. Remove `get_parent()` method
3. Keep `annotations` and `cyclic_tasks` fields
4. Keep cycle detection unchanged

5. Update `build_hierarchy()`:
   - For tasks with 1+ blockers (not in cycle): create annotation "(blocked by: Name1, Name2)" using task subjects
   - For cyclic tasks: keep existing warning annotation
   - Remove all parent_map logic (depth calculation, chain annotation)

6. Remove these helper functions that are no longer needed:
   - `calculate_depth()`
   - `build_chain_annotation()`

7. Update tests:
   - Remove `test_single_blocker_creates_parent` (no longer applies)
   - Remove `test_depth_limit` (no longer applies)
   - Add `test_single_blocker_creates_annotation` - verify single blocker creates "(blocked by: Task A)" annotation
   - Update `test_multiple_blockers_creates_annotation` - verify format is "(blocked by: A, B)"
   - Keep `test_no_dependencies`, `test_cycle_detection`, `test_missing_blocker_ignored`

Annotation format: "(blocked by: Task A)" or "(blocked by: Task A, Task B)" - parentheses, lowercase, task subjects.
  </action>
  <verify>
    `cargo test -p claude-tasks hierarchy` passes with new annotation-only behavior
  </verify>
  <done>
    hierarchy.rs produces annotations for ALL blocked tasks (single or multiple blockers), no parent_map
  </done>
</task>

<task type="auto">
  <name>Task 2: Update commands.rs for flat structure</name>
  <files>claude-tasks/src/commands.rs</files>
  <action>
Modify `create_todo_commands_with_hierarchy()` in `commands.rs`:

1. Remove parent lookup logic - always use header_id as parent:
   ```rust
   let parent_id = ROption::RSome(RString::from(header_id));
   ```

2. Remove indent level variation - always use indent_level 1:
   ```rust
   let indent_level = 1;
   ```

3. Update content formatting:
   - If task has annotation from hierarchy: append it in grey
   - Format: "{subject} (blocked by: X, Y)" - annotation comes from hierarchy
   - If cyclic: keep existing "{annotation} {subject}" format
   - Remove blocked emoji prefix since annotation now provides the info

4. Update `format_task_content()`:
   - Remove the blocked emoji prefix logic
   - Just return `task.subject.clone()` (annotation handled separately)

5. Update `update_todo_command()` and `needs_update()`:
   - These need to handle the new content format without emoji
   - `needs_update()` should compare subject directly (no emoji prefix)

6. Update tests:
   - `test_create_todo_commands_blocked` - verify no emoji, just subject
   - `test_update_todo_command_blocked` - verify no emoji prefix
   - `test_needs_update_blocked_status_changed` - adjust expected content
   - `test_needs_update_blocked_content_matches` - adjust expected content

Note: The annotation is applied in `create_todo_commands_with_hierarchy` by reading `hierarchy.get_annotation()`.
  </action>
  <verify>
    `cargo test -p claude-tasks commands` passes with flat structure behavior
  </verify>
  <done>
    All todos created at indent_level 1 with header as parent, blocked tasks show annotation suffix
  </done>
</task>

<task type="auto">
  <name>Task 3: Update sync.rs tests and verify integration</name>
  <files>claude-tasks/src/sync.rs</files>
  <action>
Update `sync.rs` to work with flat hierarchy:

1. `needs_update()` function - update expected content comparison:
   - Remove the blocked emoji prefix from expected_content
   - Expected content is just `task.subject.clone()` regardless of blocked status
   - Note: annotation is only applied on create, not stored in todo content for comparison

2. Update tests that check content format:
   - `test_needs_update_blocked_status_changed` - expected content should be subject only
   - `test_needs_update_blocked_content_matches` - expected content should be subject only

3. Run full test suite to verify integration:
   - `cargo test -p claude-tasks`

The key insight: annotations are only visible on initial create (via `create_todo_commands_with_hierarchy`). Updates compare just the subject. This is simpler and matches the flat model.
  </action>
  <verify>
    `cargo test -p claude-tasks` - all 110+ tests pass
  </verify>
  <done>
    sync.rs works with flat hierarchy, full test suite passes
  </done>
</task>

</tasks>

<verification>
- `cargo test -p claude-tasks` passes (all existing tests adapted + new tests)
- `cargo build -p claude-tasks --release` succeeds
- Manual inspection: hierarchy.rs has no parent_map, commands.rs always uses indent_level 1
</verification>

<success_criteria>
- All tasks render at same indent level (flat list)
- Blocked tasks show "(blocked by: TaskName)" annotation
- Cycle warnings still display
- All tests pass
</success_criteria>

<output>
After completion, create `.planning/quick/001-flatten-dependency-hierarchy-to-blocked-by/001-SUMMARY.md`
</output>
