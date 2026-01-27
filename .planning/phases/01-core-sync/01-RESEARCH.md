# Phase 01: Core Sync - Research

**Researched:** 2026-01-27
**Domain:** Rust totui plugin with file watching and real-time task synchronization
**Confidence:** HIGH

## Summary

This phase implements a totui plugin that discovers Claude Code tasklists, watches for file changes, and syncs tasks to totui in real-time. The plugin uses the `notify` crate (v8.2.0) with `notify-debouncer-full` (v0.7.0) for cross-platform file system event detection with proper debouncing. Communication between the watcher thread and plugin callbacks uses `std::sync::mpsc` channels.

Claude Code stores tasks in `~/.claude/tasks/{uuid}/*.json` with a simple JSON schema: `{id, subject, description, activeForm, status, blocks[], blockedBy[]}`. The plugin correlates Claude tasks to totui todos via metadata and uses the HostApi's `query_todos_by_metadata()` for efficient lookups.

**Primary recommendation:** Use `notify-debouncer-full` with 200ms timeout, watch directories (not files), spawn watcher thread on `on_config_loaded`, and communicate via mpsc channel checked in `on_event(OnLoad)`.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| totui-plugin-interface | git (main) | Plugin trait and FFI types | Required interface for totui plugins; provides Plugin trait, FfiCommand, HostApi_TO |
| abi_stable | 0.11.3 | FFI-safe plugin interface | Required by totui-plugin-interface for stable ABI across dynamic library boundaries |
| notify | 8.2.0 | Cross-platform file watching | De facto standard for Rust file watching; used by rust-analyzer, deno, cargo-watch; supports FSEvents (macOS), inotify (Linux), ReadDirectoryChangesW (Windows) |
| notify-debouncer-full | 0.7.0 | Event debouncing and batching | Properly merges multiple events per file, handles rename events, normalizes platform differences |
| serde | 1.0.x | Serialization framework | Required for JSON parsing; industry standard |
| serde_json | 1.0.x | JSON parsing | Parse Claude task files (*.json) |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | 1.20.x | UUID generation/parsing | Generate todo IDs; parse Claude tasklist UUIDs |
| dirs | 6.0.x | Home directory expansion | Expand `~` in `~/.claude/tasks/` reliably across platforms |

### Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| notify-debouncer-full | notify-debouncer-mini | Never for this use case; mini lacks rename correlation and platform normalization |
| notify 8.x | raw inotify/FSEvents | Only if Linux/macOS-only AND need maximum control; loses cross-platform |
| std::sync::mpsc | crossbeam-channel | Only if needing MPMC or select! macro; std::sync::mpsc sufficient |
| std::sync::Mutex | parking_lot::Mutex | Only if 1-byte mutex size matters; std::sync simpler |

**Installation:**

```toml
[package]
name = "claude-tasks"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
# Core - Required
totui-plugin-interface = { git = "https://github.com/grimurjonsson/to-tui", branch = "main" }
abi_stable = "0.11"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# File watching - Required
notify = "8"
notify-debouncer-full = "0.7"

# Utilities - Required
uuid = { version = "1.20", features = ["v4"] }
dirs = "6"
```

## Architecture Patterns

### Recommended Project Structure

```
src/
├── lib.rs                # Plugin entry point, Plugin trait impl, module export
├── watcher.rs            # File watcher setup and event processing
├── sync.rs               # Reconciliation logic (Claude tasks <-> totui todos)
├── claude_task.rs        # ClaudeTask struct, JSON parsing
├── commands.rs           # FfiCommand generation helpers
└── state.rs              # Shared state (Mutex<SyncState>)
```

### Pattern 1: Background Watcher with Channel Communication

**What:** Spawn watcher thread on plugin load, communicate via mpsc channel, check channel in event handlers.

**When to use:** Always. totui-plugin-interface is synchronous; file watching requires background thread.

**Example:**
```rust
// Source: totui-plugin-interface Plugin trait + notify docs

use std::sync::{mpsc, Mutex};
use std::thread;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::time::Duration;

struct ClaudeTasksPlugin {
    // Receiver for events from watcher thread
    rx: Mutex<Option<mpsc::Receiver<SyncEvent>>>,
    // Sender clone for watcher thread
    tx: Mutex<Option<mpsc::Sender<SyncEvent>>>,
}

impl Plugin for ClaudeTasksPlugin {
    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>) {
        let (tx, rx) = mpsc::channel::<SyncEvent>();
        *self.rx.lock().unwrap() = Some(rx);

        let tx_clone = tx.clone();
        thread::spawn(move || {
            let mut debouncer = new_debouncer(
                Duration::from_millis(200),
                None,
                move |result: DebounceEventResult| {
                    if let Ok(events) = result {
                        for event in events {
                            let _ = tx_clone.send(SyncEvent::from(event));
                        }
                    }
                },
            ).unwrap();

            let tasks_dir = dirs::home_dir().unwrap().join(".claude/tasks");
            debouncer.watch(&tasks_dir, RecursiveMode::Recursive).unwrap();

            // Keep thread alive
            loop {
                thread::park();
            }
        });
    }

    fn on_event(&self, event: FfiEvent) -> RResult<FfiHookResponse, RString> {
        if let FfiEvent::OnLoad { .. } = event {
            // Check for pending sync events
            if let Some(rx) = self.rx.lock().unwrap().as_ref() {
                while let Ok(sync_event) = rx.try_recv() {
                    // Process sync events, build commands
                }
            }
        }
        RResult::ROk(FfiHookResponse::default())
    }
}
```

### Pattern 2: Metadata-Based Correlation

**What:** Use totui's SetTodoMetadata to track which todos came from which Claude tasks.

**When to use:** Always. This is how you find existing todos for updates vs creates.

**Example:**
```rust
// Create todo with correlation metadata
fn create_todo_command(task: &ClaudeTask, tasklist_id: &str) -> Vec<FfiCommand> {
    let temp_id = format!("claude-{}-{}", tasklist_id, task.id);
    vec![
        FfiCommand::CreateTodo {
            content: task.subject.clone().into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(temp_id.clone().into()),
            state: map_status_to_state(&task.status),
            priority: ROption::RNone,
            indent_level: 0,
        },
        FfiCommand::SetTodoMetadata {
            todo_id: temp_id.into(),
            data: serde_json::json!({
                "source": "claude-tasks",
                "tasklist_id": tasklist_id,
                "task_id": task.id,
                "read_only": true,
            }).to_string().into(),
            merge: false,
        },
    ]
}

// Query existing todos by metadata
fn find_existing_todo(host: &HostApi_TO<'_, RBox<()>>, tasklist_id: &str, task_id: &str) -> Option<FfiTodoItem> {
    let todos = host.query_todos_by_metadata(
        "task_id".into(),
        format!("\"{}\"", task_id).into(),
    );
    todos.into_iter().find(|t| {
        // Verify tasklist_id matches too
        let metadata = host.get_todo_metadata(t.id.clone());
        metadata.contains(&format!("\"tasklist_id\":\"{}\"", tasklist_id))
    })
}
```

### Pattern 3: Initial Sync on Load

**What:** Scan Claude tasks directory and reconcile with existing totui todos on project load.

**When to use:** Always. File watching only catches changes after watch starts; need initial sync.

**Example:**
```rust
fn initial_sync(
    host: &HostApi_TO<'_, RBox<()>>,
    tasklist_path: &Path,
    tasklist_id: &str,
) -> Vec<FfiCommand> {
    let mut commands = vec![];

    // Read all Claude tasks
    let claude_tasks = scan_tasks_directory(tasklist_path);

    // Get all existing synced todos
    let existing_todos: HashMap<String, FfiTodoItem> = host
        .query_todos_by_metadata("source".into(), "\"claude-tasks\"".into())
        .into_iter()
        .filter_map(|t| {
            let meta = host.get_todo_metadata(t.id.clone());
            // Parse task_id from metadata
            extract_task_id(&meta).map(|id| (id, t))
        })
        .collect();

    // Reconcile
    for task in &claude_tasks {
        if let Some(todo) = existing_todos.get(&task.id) {
            // Check if update needed
            if needs_update(&task, &todo) {
                commands.push(create_update_command(&task, &todo.id));
            }
        } else {
            // Create new
            commands.extend(create_todo_command(&task, tasklist_id));
        }
    }

    // Delete todos for removed tasks
    for (task_id, todo) in &existing_todos {
        if !claude_tasks.iter().any(|t| &t.id == task_id) {
            commands.push(FfiCommand::DeleteTodo { id: todo.id.clone() });
        }
    }

    commands
}
```

### Anti-Patterns to Avoid

- **Watching individual files:** Claude Code may use atomic writes (write temp, rename). Watch directories instead.
- **Blocking I/O in on_event():** on_event has timeout constraints. Use background thread for file I/O.
- **Skipping debouncing:** A single file save triggers 3-5 events. Always debounce.
- **Polling instead of watching:** Wastes CPU, misses changes between polls. Use event-based watching.
- **Trusting file paths for identity:** Use metadata with (tasklist_id, task_id) as correlation key.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File watching | Custom inotify/FSEvents wrapper | `notify` crate | Cross-platform, battle-tested, handles edge cases |
| Event debouncing | Manual timer-based debounce | `notify-debouncer-full` | Handles rename correlation, platform normalization |
| Home directory | Manual `$HOME` parsing | `dirs` crate | Handles Windows, macOS, Linux correctly |
| UUID generation | Random string generation | `uuid` crate | Proper v4 UUID format, serde support |
| JSON parsing | Manual string parsing | `serde_json` | Handles escaping, errors, nested structures |

**Key insight:** File watching looks simple but has significant cross-platform complexity (event ordering, rename tracking, debouncing). The notify ecosystem handles these; custom solutions will miss edge cases.

## Common Pitfalls

### Pitfall 1: Reading Partially-Written Files

**What goes wrong:** Watcher emits `Modify` event before write completes. Reading immediately gets empty/truncated JSON.

**Why it happens:** File systems emit notifications when writes begin, not when they complete.

**How to avoid:** Use `notify-debouncer-full` with 200ms timeout. If JSON parse fails, retry after short delay.

**Warning signs:** Intermittent parse failures, empty file content in logs, tests that pass sometimes.

### Pitfall 2: Orphaned Watchers from Atomic Writes

**What goes wrong:** Watch `~/.claude/tasks/{uuid}/1.json` directly. Claude writes to temp file, renames over target. Watcher now points to deleted inode.

**Why it happens:** Atomic writes replace the file inode; file-level watchers become orphaned.

**How to avoid:** Watch parent directory (`~/.claude/tasks/{uuid}/`), filter events by filename.

**Warning signs:** Events stop after first update, "stale watcher" behavior.

### Pitfall 3: Event Flood Without Debouncing

**What goes wrong:** Single save triggers 3-5 events (truncate, write, write, metadata). Processing each causes duplicate work.

**Why it happens:** Editors have different save strategies; all produce multiple events.

**How to avoid:** Use `notify-debouncer-full` with 200ms timeout. One debounced event per file change.

**Warning signs:** 3-5 log entries per save, UI updates multiple times, state inconsistencies.

### Pitfall 4: Blocking Watcher Thread

**What goes wrong:** Perform JSON parsing and state updates in watcher callback. Blocks event loop, misses subsequent events.

**Why it happens:** Callback is called synchronously by notify; blocking delays all further events.

**How to avoid:** Send minimal event to channel, do heavy work in main thread.

**Warning signs:** Delayed events, missed changes during bursts.

## Code Examples

### Claude Task JSON Schema

```rust
// Source: ~/.claude/tasks/{uuid}/*.json (verified 2026-01-27)

#[derive(Debug, Deserialize, Serialize)]
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
```

### Status Mapping

```rust
// Source: totui-plugin-interface types.rs, CONTEXT.md decisions

fn map_status_to_state(status: &str) -> FfiTodoState {
    match status {
        "pending" => FfiTodoState::Empty,        // [ ]
        "in_progress" => FfiTodoState::InProgress, // [*]
        "completed" => FfiTodoState::Checked,    // [x]
        _ => FfiTodoState::Empty,
    }
}
```

### Directory Scanning

```rust
// Source: Rust std::fs, verified against ~/.claude/tasks/ structure

fn scan_tasks_directory(tasklist_path: &Path) -> Vec<ClaudeTask> {
    let mut tasks = vec![];

    if let Ok(entries) = std::fs::read_dir(tasklist_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<ClaudeTask>(&content) {
                        tasks.push(task);
                    }
                }
            }
        }
    }

    // Sort by numeric ID for consistent ordering
    tasks.sort_by(|a, b| {
        a.id.parse::<u32>().unwrap_or(0)
            .cmp(&b.id.parse::<u32>().unwrap_or(0))
    });

    tasks
}
```

### Tasklist Discovery

```rust
// Source: Rust std::fs, DISC-01/DISC-02 requirements

#[derive(Debug)]
struct TasklistInfo {
    id: String,              // UUID folder name
    path: PathBuf,           // Full path
    task_count: usize,       // Number of .json files
    last_modified: SystemTime,
    sample_tasks: Vec<String>, // First 3 task subjects
}

fn discover_tasklists() -> Vec<TasklistInfo> {
    let tasks_dir = dirs::home_dir()
        .expect("Home directory not found")
        .join(".claude/tasks");

    let mut tasklists = vec![];

    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let id = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let tasks = scan_tasks_directory(&path);
                let last_modified = entry.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                tasklists.push(TasklistInfo {
                    id,
                    path,
                    task_count: tasks.len(),
                    last_modified,
                    sample_tasks: tasks.iter()
                        .take(3)
                        .map(|t| t.subject.clone())
                        .collect(),
                });
            }
        }
    }

    // Sort alphabetically by ID (per CONTEXT.md decision)
    tasklists.sort_by(|a, b| a.id.cmp(&b.id));
    tasklists
}
```

### Header Todo Creation

```rust
// Source: DISP-01 requirement, CONTEXT.md decisions

fn create_header_todo(tasklist_id: &str) -> FfiCommand {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Format: "CLAUDE TASKLIST: {id} - last updated: {timestamp}"
    // Timestamp only shown when >1 minute since last sync (clean header otherwise)
    let content = format!("CLAUDE TASKLIST: {}", tasklist_id);

    FfiCommand::CreateTodo {
        content: content.into(),
        parent_id: ROption::RNone,
        temp_id: ROption::RSome(format!("claude-header-{}", tasklist_id).into()),
        state: FfiTodoState::Empty,
        priority: ROption::RNone,
        indent_level: 0,
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| notify 6.x | notify 8.x | MSRV 1.85 | Better debouncer integration, cleaner API |
| notify-debouncer-mini | notify-debouncer-full | notify 7+ | Full debouncer has rename correlation, better platform normalization |
| std::sync::mpsc (old) | std::sync::mpsc (crossbeam internally) | Rust 1.67 | std::sync::mpsc now uses crossbeam implementation; similar performance |

**Deprecated/outdated:**
- `hotwatch` crate: Last updated 2021, abandoned. Use `notify` instead.
- Direct inotify/FSEvents: Only for platform-specific low-level needs. `notify` abstracts correctly.

## Open Questions

1. **Read-only enforcement mechanism**
   - What we know: Metadata can mark todos with `read_only: true`
   - What's unclear: How totui enforces read-only (UI-level? event rejection?)
   - Recommendation: Mark metadata now; Phase 2 addresses UX handling (PLUG-04/PLUG-05)

2. **Hierarchy handling for deeply nested blockedBy chains**
   - What we know: Claude tasks have flat blockedBy[] arrays, not tree structure
   - What's unclear: Best way to represent A->B->C blocking chains as parent-child
   - Recommendation: Use first blockedBy item as parent; note limitation for Phase 2 enhancement

3. **Thread lifecycle on plugin unload**
   - What we know: Plugin should clean up watcher thread
   - What's unclear: Whether totui calls any cleanup method or just drops plugin
   - Recommendation: Store thread handle; implement Drop trait for cleanup (Phase 2 - PLUG-04)

## Sources

### Primary (HIGH confidence)
- `totui-plugin-interface` source at `/Users/gimmi/.cargo/git/checkouts/to-tui-246362fb02deef3e/` - Plugin trait, FfiCommand, HostApi verified
- Claude tasks directory at `~/.claude/tasks/d45035ac-8878-4400-9304-c43d1e9afcbe/` - JSON schema verified
- [notify docs.rs 8.2.0](https://docs.rs/notify/8.2.0/notify/) - API patterns, EventKind
- [notify-debouncer-full docs.rs 0.7.0](https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/) - Debouncer API

### Secondary (MEDIUM confidence)
- `.planning/research/ARCHITECTURE.md` - Prior architecture research
- `.planning/research/STACK.md` - Stack decisions and versions
- `.planning/research/PITFALLS.md` - Pitfall catalog

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Verified against docs.rs and existing jira-claude plugin
- Architecture: HIGH - Based on totui-plugin-interface source code analysis
- Pitfalls: HIGH - Verified against official notify documentation and issue tracker

**Research date:** 2026-01-27
**Valid until:** 2026-02-27 (30 days - stable ecosystem)
