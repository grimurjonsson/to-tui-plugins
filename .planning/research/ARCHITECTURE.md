# Architecture Research

**Domain:** File-watching sync plugin for totui (Rust)
**Researched:** 2026-01-27
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
+-----------------------------------------------------------------------+
|                          claude-tasks Plugin                           |
+-----------------------------------------------------------------------+
|                                                                        |
|  +-----------------------+     +----------------------------------+   |
|  |    File Watcher       |     |         Sync Engine              |   |
|  |  (notify + debouncer) |     |    (reconciliation logic)        |   |
|  +----------+------------+     +----------------+-----------------+   |
|             |                                   |                      |
|             | FS events                         | sync decisions       |
|             v                                   v                      |
|  +-----------------------+     +----------------------------------+   |
|  |    Event Processor    |<--->|        State Manager             |   |
|  |   (maps FS -> domain) |     | (tracks what we know about both) |   |
|  +----------+------------+     +----------------+-----------------+   |
|             |                                   |                      |
|             +-----------------------------------+                      |
|                            |                                           |
|                            v                                           |
|                  +--------------------+                                |
|                  |  Command Generator |                                |
|                  | (FfiCommand batch) |                                |
|                  +---------+----------+                                |
|                            |                                           |
+----------------------------+-------------------------------------------+
                             |
                             | FfiCommand[], on_event() responses
                             v
+-----------------------------------------------------------------------+
|                         totui Host                                     |
|  - execute_with_host() calls                                          |
|  - on_event() dispatches (OnAdd, OnModify, OnDelete, OnComplete)      |
|  - HostApi: query_todos(), get_todo_metadata(), etc.                  |
+-----------------------------------------------------------------------+
                             |
                             v
+-----------------------------------------------------------------------+
|                         totui Data Store                              |
|  - Todo items with metadata                                           |
|  - Project-scoped state                                               |
+-----------------------------------------------------------------------+

+-----------------------------------------------------------------------+
|                    Claude Code Tasks Directory                         |
|  ~/.claude/tasks/{uuid}/*.json                                        |
|  - 1.json, 2.json, etc.                                               |
|  - {id, subject, description, status, blocks[], blockedBy[]}          |
+-----------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| File Watcher | Monitor `~/.claude/tasks/` for changes | `notify` crate with debouncer |
| Event Processor | Convert FS events to domain events | Match on paths, parse JSON |
| State Manager | Track known state from both sources | `HashMap<TaskId, TaskState>` in `Mutex` |
| Sync Engine | Decide what changes to apply | Compare states, generate diffs |
| Command Generator | Produce FfiCommand batches | Map domain decisions to totui API |

## Recommended Project Structure

```
src/
├── lib.rs               # Plugin entry point, Plugin trait impl
├── watcher/             # File watching subsystem
│   ├── mod.rs           # Watcher initialization and management
│   └── events.rs        # FS event -> domain event mapping
├── sync/                # Synchronization logic
│   ├── mod.rs           # Sync engine orchestration
│   ├── state.rs         # State tracking for both sources
│   └── reconcile.rs     # Diff calculation and conflict resolution
├── claude_tasks/        # Claude Code tasks domain
│   ├── mod.rs           # Task type definitions
│   ├── parser.rs        # JSON parsing for task files
│   └── schema.rs        # Task JSON schema/types
├── commands.rs          # FfiCommand generation helpers
└── config.rs            # Plugin configuration types
```

### Structure Rationale

- **watcher/:** Isolates file system concerns. Can be tested independently with mock events.
- **sync/:** Core business logic. Pure functions for reconciliation make testing easy.
- **claude_tasks/:** Domain model for Claude Code tasks. Decoupled from sync logic.
- **commands.rs:** Thin layer mapping domain decisions to FFI commands.

## Architectural Patterns

### Pattern 1: Event-Driven Polling Hybrid

**What:** Use file watcher for change detection, but poll on plugin initialization and OnLoad events.
**When to use:** Always. File watching catches changes, but initial sync requires polling.
**Trade-offs:**
- Pro: Responsive to changes, catches external edits
- Con: Complexity of managing watcher lifecycle

**Example:**
```rust
// On plugin load (execute_with_host with init signal or OnLoad event)
fn initial_sync(&self, host: &HostApi_TO<'_, RBox<()>>) -> Vec<FfiCommand> {
    let claude_tasks = self.scan_tasks_directory();
    let totui_todos = host.query_todos_by_metadata("source".into(), "\"claude-tasks\"".into());
    self.reconcile(claude_tasks, totui_todos)
}

// On file watcher event (called from on_event or background)
fn on_file_change(&self, path: &Path, kind: EventKind) -> Option<DomainEvent> {
    match kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            let task = parse_task_file(path)?;
            Some(DomainEvent::TaskUpdated(task))
        }
        EventKind::Remove(_) => {
            let task_id = extract_task_id(path)?;
            Some(DomainEvent::TaskRemoved(task_id))
        }
        _ => None,
    }
}
```

### Pattern 2: Metadata-Based Correlation

**What:** Use totui's SetTodoMetadata to track which todos came from which Claude tasks.
**When to use:** Always. This is how you correlate Claude tasks to totui todos.
**Trade-offs:**
- Pro: Clean separation, survives todo content edits
- Con: Requires metadata queries on every sync

**Example:**
```rust
// When creating a todo from a Claude task
FfiCommand::CreateTodo {
    content: task.subject.into(),
    parent_id: ROption::RNone,
    temp_id: ROption::RSome(format!("claude-{}", task.id).into()),
    state: map_status_to_state(&task.status),
    priority: ROption::RNone,
    indent_level: 0,
}

// Followed by metadata to track the source
FfiCommand::SetTodoMetadata {
    todo_id: temp_id, // Host resolves this to real UUID
    data: json!({
        "source": "claude-tasks",
        "session_id": session_uuid,
        "task_id": task.id,
        "file_path": task.file_path,
    }).to_string().into(),
    merge: false,
}
```

### Pattern 3: Debounced File Watching

**What:** Use notify-debouncer-full to batch rapid file changes into single events.
**When to use:** Always. Claude Code writes tasks rapidly during agent operations.
**Trade-offs:**
- Pro: Prevents sync thrashing, reduces API calls
- Con: Slight delay (500ms-2s) before sync

**Example:**
```rust
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::time::Duration;

fn setup_watcher(tx: Sender<DomainEvent>) -> Result<Debouncer<...>, Error> {
    let mut debouncer = new_debouncer(
        Duration::from_secs(1),  // Wait 1s after last change
        None,                    // No file ID cache needed
        move |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    for event in events {
                        if let Some(domain_event) = process_event(&event) {
                            let _ = tx.send(domain_event);
                        }
                    }
                }
                Err(errors) => {
                    for err in errors {
                        eprintln!("Watcher error: {:?}", err);
                    }
                }
            }
        },
    )?;

    debouncer.watcher().watch(
        Path::new(&*shellexpand::tilde("~/.claude/tasks")),
        RecursiveMode::Recursive,
    )?;

    Ok(debouncer)
}
```

## Data Flow

### Sync Direction: Claude Tasks -> totui

```
[Claude Code writes task file]
    |
    v
[File Watcher] -- debounced event --> [Event Processor]
    |
    v
[Parse JSON] --> ClaudeTask struct
    |
    v
[Query existing todo by metadata] -- HostApi::query_todos_by_metadata -->
    |
    |-- Found existing todo:
    |       v
    |   [Compare states] --> [Generate UpdateTodo if changed]
    |
    |-- No existing todo:
            v
        [Generate CreateTodo + SetTodoMetadata]
```

### Sync Direction: totui -> Claude Tasks (Future/Optional)

```
[totui OnModify/OnComplete event via on_event()]
    |
    v
[Check metadata for "source": "claude-tasks"]
    |
    |-- Not from us: ignore
    |
    |-- From us:
            v
        [Read current task file]
            |
            v
        [Update task status/content]
            |
            v
        [Write updated JSON]
```

**Note:** Bidirectional sync adds significant complexity (conflict resolution, write-back locking). Recommend starting with one-way sync (Claude -> totui) for MVP.

### Key Data Flows

1. **Initialization:** On OnLoad event, scan ~/.claude/tasks/, query totui for existing synced todos, reconcile differences.
2. **File Change:** Debounced watcher fires, parse changed file, find corresponding todo, emit UpdateTodo or CreateTodo commands.
3. **Session Discovery:** New UUID directories in ~/.claude/tasks/ trigger scanning that session's tasks.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1-10 sessions | Simple: scan all sessions on load, watch entire ~/.claude/tasks/ |
| 10-100 sessions | Add session filtering: config option to watch specific sessions only |
| 100+ sessions | Archive old sessions: don't watch sessions older than N days |

### Scaling Priorities

1. **First bottleneck:** Too many file watches. Mitigation: Only watch active sessions (determined by modified_time of directory).
2. **Second bottleneck:** Slow initial sync with many todos. Mitigation: Use query_todos_by_metadata() batch queries, not per-task queries.

## Anti-Patterns

### Anti-Pattern 1: Blocking File I/O in on_event()

**What people do:** Parse task files synchronously during on_event() handling.
**Why it's wrong:** on_event() has timeout constraints. File I/O can block if filesystem is slow.
**Do this instead:** Queue events for processing, return quickly from on_event().

### Anti-Pattern 2: Polling Instead of Watching

**What people do:** Use execute_with_host() with a timer to periodically scan the task directory.
**Why it's wrong:** Misses changes between polls, wastes resources when nothing changed.
**Do this instead:** Use notify crate for event-driven change detection.

### Anti-Pattern 3: No Deduplication of Commands

**What people do:** Generate CreateTodo + SetTodoMetadata every time a task file is seen.
**Why it's wrong:** Creates duplicates in totui, metadata lookup fails for temp_ids.
**Do this instead:** Always query for existing todo by metadata before creating.

### Anti-Pattern 4: Trusting File Paths for Identity

**What people do:** Use file path as the todo identifier.
**Why it's wrong:** Claude Code may reorganize tasks, rename sessions, etc.
**Do this instead:** Use metadata with (session_id, task_id) as the correlation key.

### Anti-Pattern 5: Synchronous Watcher in Plugin Init

**What people do:** Start watcher in plugin constructor, blocking totui startup.
**Why it's wrong:** Plugin load should be fast. Watcher setup involves I/O.
**Do this instead:** Defer watcher setup to first execute_with_host() or on_config_loaded().

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Claude Code tasks | File watcher on `~/.claude/tasks/` | JSON files, debounce required |
| totui Host API | HostApi_TO trait object | Query via execute_with_host(), respond via on_event() |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Watcher -> Sync Engine | Channel (std::sync::mpsc or crossbeam) | Debounced events |
| Sync Engine -> Commands | Direct function call | Pure transformation |
| State Manager | Mutex<HashMap> | Shared state between event sources |

## Claude Tasks JSON Schema

Based on actual file inspection:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct ClaudeTask {
    /// Numeric string ID (e.g., "1", "2")
    pub id: String,
    /// Short title of the task
    pub subject: String,
    /// Detailed description
    pub description: String,
    /// Current activity description
    #[serde(rename = "activeForm")]
    pub active_form: String,
    /// Status: "pending", "in_progress", "completed", etc.
    pub status: String,
    /// Task IDs this task blocks (downstream dependencies)
    pub blocks: Vec<String>,
    /// Task IDs blocking this task (upstream dependencies)
    #[serde(rename = "blockedBy")]
    pub blocked_by: Vec<String>,
}
```

**Directory structure:**
```
~/.claude/tasks/
└── {session-uuid}/        # e.g., d45035ac-8878-4400-9304-c43d1e9afcbe
    ├── 1.json
    ├── 2.json
    └── ...
```

## Build Order Implications

Based on component dependencies:

1. **Phase 1: Core Types + Parser** - ClaudeTask struct, JSON parsing, no external deps
2. **Phase 2: State Manager** - Track known state, pure Rust data structures
3. **Phase 3: File Watcher** - notify/debouncer setup, event mapping
4. **Phase 4: Sync Engine** - Reconciliation logic, depends on phases 1-3
5. **Phase 5: Plugin Integration** - Wire everything into Plugin trait

**Rationale:** Each phase builds on the previous, enabling incremental testing. Parser can be tested with fixture files. State manager can be tested with mock data. Watcher can be tested in isolation. Full sync requires all pieces.

## Sources

- [notify crate documentation](https://docs.rs/notify/latest/notify/) - HIGH confidence, official docs
- [notify-debouncer-full documentation](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/) - HIGH confidence, official docs
- totui-plugin-interface source code at `/Users/gimmi/.cargo/git/checkouts/to-tui-246362fb02deef3e/4b669e2/crates/totui-plugin-interface/` - HIGH confidence, actual code
- Actual Claude Code tasks files at `~/.claude/tasks/` - HIGH confidence, direct observation
- [Rust forum on file monitoring](https://users.rust-lang.org/t/solved-spawning-threads-for-file-monitoring/11896) - MEDIUM confidence

---
*Architecture research for: claude-tasks totui plugin*
*Researched: 2026-01-27*
