# Phase 2: Robustness and Features - Research

**Researched:** 2026-01-27
**Domain:** Plugin robustness (error handling, cleanup, platform limits), configuration (aliases, schema), dependency visualization, staleness detection
**Confidence:** HIGH

## Summary

This phase makes the claude-tasks plugin production-ready by addressing platform-specific limitations (FSEvents/inotify), implementing proper watcher cleanup, adding user configuration for aliases, visualizing task dependencies as hierarchy, and detecting stale tasklists.

The `notify` crate's `ErrorKind::MaxFilesWatch` variant handles inotify limits. For FSEvents on macOS, errors surface as `ErrorKind::Io` or `ErrorKind::Generic`. The watcher cleanup follows Rust's RAII pattern - the `notify-debouncer-full` Debouncer stops on drop. For thread cleanup, use `Option<JoinHandle<()>>` with `take()` in `Drop` impl and signal shutdown via channel close.

For dependency visualization, the codebase already has `blocked_by` arrays in ClaudeTask. Building a hierarchy requires topological sorting with cycle detection - `petgraph` provides O(|V|+|E|) algorithms. Staleness uses `std::time::Instant::elapsed()` compared against a threshold.

**Primary recommendation:** Match on `notify::ErrorKind::MaxFilesWatch` for inotify limits and `Io` errors with ENOSPC for macOS. Store `Option<WatcherHandle>` and implement `Drop` to signal shutdown via channel close. Use `toml` crate for alias config. Build dependency tree locally in sync module, flatten cycles to root level with warning annotation.

## Standard Stack

### Core (Already in Cargo.toml)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| notify | 8 | File watching | Already in use; ErrorKind::MaxFilesWatch for limits |
| notify-debouncer-full | 0.7 | Event debouncing | Already in use; auto-stops on drop |
| serde | 1.0 | Serialization | Already in use; required for TOML |

### New Dependencies

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 | TOML parsing/serialization | De facto Rust TOML crate; 176M downloads; serde integration |
| chrono | 0.4 | Human-readable durations | Format staleness duration as "23m" |

### Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| toml | toml_edit | Only when preserving formatting matters (e.g., user-editable config with comments) |
| chrono | humantime | chrono already common; humantime lighter if not using chrono elsewhere |
| local HashSet | petgraph | Only if deps become complex graphs; current blocked_by is simple enough |

**Installation (additions to Cargo.toml):**

```toml
[dependencies]
# Configuration - New
toml = "0.8"

# Optional: Better duration formatting
chrono = { version = "0.4", optional = true, default-features = false }
```

## Architecture Patterns

### Recommended Module Additions

```
src/
├── lib.rs                # Add Drop impl for cleanup
├── watcher.rs            # Add error type handling, shutdown signal
├── config.rs             # NEW: Config loading, alias resolution
├── hierarchy.rs          # NEW: Dependency tree building
├── staleness.rs          # NEW: Staleness tracking
└── errors.rs             # NEW: User-facing error messages
```

### Pattern 1: Platform Error Detection and Reporting

**What:** Match on notify error kinds to detect platform limits and provide clear messages.

**When to use:** In watcher setup and in debouncer callback error handling.

**Example:**
```rust
// Source: notify 8.2.0 docs.rs/notify/latest/notify/enum.ErrorKind.html

use notify::{Error, ErrorKind};

fn handle_watcher_error(error: &Error) -> String {
    match &error.kind {
        ErrorKind::MaxFilesWatch => {
            // Linux inotify limit reached
            "inotify watch limit reached. Run: sudo sysctl fs.inotify.max_user_watches=524288".to_string()
        }
        ErrorKind::Io(io_err) => {
            match io_err.raw_os_error() {
                Some(28) => {  // ENOSPC on Linux
                    "inotify watch limit reached".to_string()
                }
                Some(24) => {  // EMFILE - too many open files (macOS kqueue)
                    "File descriptor limit reached. Run: ulimit -n 8192".to_string()
                }
                _ => format!("File watching error: {}", io_err)
            }
        }
        ErrorKind::Generic(msg) => {
            if msg.contains("os error 38") {
                // Docker on macOS M1
                "Native file watching not available. Using polling fallback.".to_string()
            } else {
                format!("Watcher error: {}", msg)
            }
        }
        ErrorKind::PathNotFound => "Tasklist directory not found".to_string(),
        _ => format!("Watcher error: {:?}", error.kind)
    }
}
```

### Pattern 2: Graceful Watcher Thread Shutdown

**What:** Store watcher handle in `Option`, implement `Drop` to signal shutdown and join thread.

**When to use:** Always. Plugin must clean up watcher thread on unload.

**Example:**
```rust
// Source: Rust Book ch20-03, notify-debouncer-full docs

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct WatcherHandle {
    thread_handle: Option<JoinHandle<()>>,
    shutdown_flag: Arc<AtomicBool>,
}

impl WatcherHandle {
    pub fn shutdown(&mut self) {
        // Signal thread to stop
        self.shutdown_flag.store(true, Ordering::SeqCst);

        // Take ownership and join
        if let Some(handle) = self.thread_handle.take() {
            // Unpark in case thread is parked
            handle.thread().unpark();
            // Join with timeout via is_finished check, or just join
            let _ = handle.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// In watcher thread:
fn watcher_loop(shutdown: Arc<AtomicBool>) {
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        std::thread::park_timeout(std::time::Duration::from_millis(100));
    }
}
```

### Pattern 3: TOML Config with Serde

**What:** Define config struct with serde, load from global and local paths, merge.

**When to use:** For tasklist aliases (PLUG-05, DISC-03).

**Example:**
```rust
// Source: docs.rs/toml/latest/toml/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PluginConfig {
    #[serde(default)]
    pub aliases: HashMap<String, String>,  // UUID -> friendly name
    #[serde(default)]
    pub staleness_threshold_minutes: Option<u64>,  // Default: 15
}

fn load_config() -> PluginConfig {
    let mut config = PluginConfig::default();

    // Load global config
    if let Some(global_path) = dirs::config_dir() {
        let path = global_path.join("totui/claude-tasks.toml");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(global) = toml::from_str::<PluginConfig>(&content) {
                config = global;
            }
        }
    }

    // Merge local config (overrides global)
    let local_path = PathBuf::from(".totui/aliases.toml");
    if let Ok(content) = std::fs::read_to_string(&local_path) {
        if let Ok(local) = toml::from_str::<PluginConfig>(&content) {
            config.aliases.extend(local.aliases);
            if local.staleness_threshold_minutes.is_some() {
                config.staleness_threshold_minutes = local.staleness_threshold_minutes;
            }
        }
    }

    config
}
```

### Pattern 4: Dependency Hierarchy Building

**What:** Build parent-child relationships from blocked_by arrays, detect cycles, flatten deep chains.

**When to use:** When syncing tasks with blockedBy relationships (SYNC-05).

**Example:**
```rust
// Source: CONTEXT.md decisions, local implementation (no petgraph needed for simple case)

use std::collections::{HashMap, HashSet};

struct TaskHierarchy {
    /// task_id -> parent_task_id (single blocker = parent)
    parent_map: HashMap<String, String>,
    /// task_id -> multiple blockers annotation
    multi_blocker_annotations: HashMap<String, String>,
    /// Tasks involved in cycles (show at root with warning)
    cyclic_tasks: HashSet<String>,
}

fn build_hierarchy(tasks: &[ClaudeTask]) -> TaskHierarchy {
    let mut hierarchy = TaskHierarchy::default();
    let task_map: HashMap<&str, &ClaudeTask> = tasks.iter()
        .map(|t| (t.id.as_str(), t))
        .collect();

    // Detect cycles using DFS
    let cyclic = detect_cycles(tasks);
    hierarchy.cyclic_tasks = cyclic;

    for task in tasks {
        if hierarchy.cyclic_tasks.contains(&task.id) {
            // Cyclic tasks stay at root level
            continue;
        }

        match task.blocked_by.len() {
            0 => {
                // No blockers - root level
            }
            1 => {
                // Single blocker - make it parent
                let parent_id = &task.blocked_by[0];
                // Check depth doesn't exceed max (2-3 levels)
                let depth = calculate_depth(parent_id, &hierarchy.parent_map);
                if depth < 3 {
                    hierarchy.parent_map.insert(task.id.clone(), parent_id.clone());
                } else {
                    // Flatten with chain annotation
                    let chain = build_chain_annotation(parent_id, &hierarchy.parent_map, &task_map);
                    hierarchy.multi_blocker_annotations.insert(task.id.clone(), chain);
                }
            }
            _ => {
                // Multiple blockers - root level with annotation
                let names: Vec<&str> = task.blocked_by.iter()
                    .filter_map(|id| task_map.get(id.as_str()).map(|t| t.subject.as_str()))
                    .collect();
                let annotation = format!("Blocked by: {}", names.join(", "));
                hierarchy.multi_blocker_annotations.insert(task.id.clone(), annotation);
            }
        }
    }

    hierarchy
}

fn detect_cycles(tasks: &[ClaudeTask]) -> HashSet<String> {
    // Simple cycle detection using DFS with coloring
    let mut cyclic = HashSet::new();
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    let adj: HashMap<&str, Vec<&str>> = tasks.iter()
        .map(|t| (t.id.as_str(), t.blocked_by.iter().map(|s| s.as_str()).collect()))
        .collect();

    for task in tasks {
        if !visited.contains(&task.id) {
            dfs_cycle(&task.id, &adj, &mut visited, &mut in_stack, &mut cyclic);
        }
    }

    cyclic
}

fn dfs_cycle(
    node: &str,
    adj: &HashMap<&str, Vec<&str>>,
    visited: &mut HashSet<String>,
    in_stack: &mut HashSet<String>,
    cyclic: &mut HashSet<String>,
) {
    visited.insert(node.to_string());
    in_stack.insert(node.to_string());

    if let Some(neighbors) = adj.get(node) {
        for &neighbor in neighbors {
            if !visited.contains(neighbor) {
                dfs_cycle(neighbor, adj, visited, in_stack, cyclic);
            } else if in_stack.contains(neighbor) {
                // Cycle detected - mark all in current stack
                cyclic.insert(node.to_string());
                cyclic.insert(neighbor.to_string());
            }
        }
    }

    in_stack.remove(node);
}
```

### Pattern 5: Staleness Tracking with Instant

**What:** Store last update timestamp, check elapsed time on each load, update header if stale.

**When to use:** For DISP-03 staleness indicator.

**Example:**
```rust
// Source: std::time::Instant documentation

use std::time::{Duration, Instant};
use std::sync::Mutex;

pub struct StalenessTracker {
    last_update: Mutex<Option<Instant>>,
    threshold: Duration,
}

impl StalenessTracker {
    pub fn new(threshold_minutes: u64) -> Self {
        Self {
            last_update: Mutex::new(None),
            threshold: Duration::from_secs(threshold_minutes * 60),
        }
    }

    pub fn record_update(&self) {
        *self.last_update.lock().unwrap() = Some(Instant::now());
    }

    pub fn check_staleness(&self) -> Option<Duration> {
        let guard = self.last_update.lock().unwrap();
        guard.map(|instant| {
            let elapsed = instant.elapsed();
            if elapsed > self.threshold {
                Some(elapsed)
            } else {
                None
            }
        }).flatten()
    }

    pub fn format_staleness(&self) -> Option<String> {
        self.check_staleness().map(|elapsed| {
            let minutes = elapsed.as_secs() / 60;
            if minutes < 60 {
                format!("{}m", minutes)
            } else {
                format!("{}h{}m", minutes / 60, minutes % 60)
            }
        })
    }
}

// Header update:
fn update_header_with_staleness(tasklist_id: &str, alias: Option<&str>, staleness: Option<&str>) -> FfiCommand {
    let name = alias.unwrap_or(tasklist_id);
    let content = match staleness {
        Some(duration) => format!("CLAUDE TASKLIST: {} STALE ({})", name, duration),
        None => format!("CLAUDE TASKLIST: {}", name),
    };
    // ... create UpdateTodo command
}
```

### Anti-Patterns to Avoid

- **Ignoring platform errors silently:** Always surface MaxFilesWatch/EMFILE to user with actionable message.
- **Blocking on thread join in Drop:** Use is_finished() check or timeout, don't hang plugin unload.
- **Hardcoding config paths:** Use dirs::config_dir() for global, current dir for local.
- **Building cycles into tree:** Detect cycles before building hierarchy; show at root with warning.
- **Using SystemTime for staleness:** Use Instant for monotonic elapsed time; SystemTime can jump.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML parsing | Manual string parsing | `toml` crate | Proper spec compliance, error handling |
| Cycle detection | Custom graph traversal | Simple DFS or `petgraph` | Well-tested algorithms, O(V+E) |
| Duration formatting | Division arithmetic | `chrono` or simple fn | Handles edge cases (0m, 1h30m) |
| Config merging | Manual field copying | serde's #[serde(flatten)] or explicit merge | Type safety, defaults |
| Platform error codes | Hard-coded numbers | libc::ENOSPC constants | Cross-platform correctness |

**Key insight:** The dependency visualization looks like it needs a graph library, but the CONTEXT.md constraints (single blocker = parent, multi = annotate, max 3 levels) make a simple local solution appropriate. Petgraph would add complexity without benefit.

## Common Pitfalls

### Pitfall 1: Thread Deadlock on Plugin Unload

**What goes wrong:** Calling `join()` on a watcher thread that's blocked in `thread::park()` or waiting on channel. Plugin unload hangs.

**Why it happens:** The thread has no way to know it should exit; `join()` waits forever.

**How to avoid:** Use shutdown flag (AtomicBool) checked in thread loop. Call `unpark()` before `join()`. Or use channel close as shutdown signal.

**Warning signs:** Plugin unload takes forever, totui becomes unresponsive on exit.

### Pitfall 2: Ignoring Platform-Specific Errors

**What goes wrong:** User hits inotify limit, sees generic "watcher failed" message, doesn't know fix.

**Why it happens:** Not matching on specific ErrorKind variants.

**How to avoid:** Match on MaxFilesWatch, check raw_os_error() for EMFILE/ENOSPC, provide actionable messages.

**Warning signs:** User reports on forums, not in your issue tracker; they don't know it's fixable.

### Pitfall 3: Infinite Loops in Cycle Detection

**What goes wrong:** Cycle detection itself enters infinite loop on cyclic graph.

**Why it happens:** DFS without tracking "in current stack" vs "already visited" states.

**How to avoid:** Use three-color marking (white=unvisited, gray=in-stack, black=done) or in_stack HashSet.

**Warning signs:** Plugin hangs when syncing tasks with circular dependencies.

### Pitfall 4: Staleness Timer Affected by Sleep

**What goes wrong:** Laptop sleeps, wakes up, staleness shows "8h" when it was actually 10 minutes active.

**Why it happens:** `std::time::Instant` uses CLOCK_MONOTONIC which pauses during suspend on some platforms.

**How to avoid:** For this use case, it's actually correct behavior - if no updates came during 8h of real time (even if sleep), it IS stale. Document this behavior.

**Warning signs:** Users confused about staleness duration; not actually a bug for this use case.

### Pitfall 5: Config File Missing vs Empty

**What goes wrong:** Treat missing config file same as empty aliases, but user expects defaults from global.

**Why it happens:** Not distinguishing between "file doesn't exist" and "file exists but section empty".

**How to avoid:** Check file existence separately. Only merge local if file exists; don't override global with empty local.

**Warning signs:** User sets global alias, local project shows UUID instead.

## Code Examples

### Error Message Display Pattern

```rust
// Source: CONTEXT.md decision - brief error messages in status bar

enum PluginError {
    WatchLimitReached(String),  // Platform-specific message
    WatcherFailed(String),
    ConfigParseError(String),
    DirectoryNotFound,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WatchLimitReached(msg) => write!(f, "{}", msg),
            Self::WatcherFailed(msg) => write!(f, "Watch failed: {}", msg),
            Self::ConfigParseError(path) => write!(f, "Invalid config: {}", path),
            Self::DirectoryNotFound => write!(f, "Tasks directory not found"),
        }
    }
}
```

### Alias Resolution

```rust
// Source: CONTEXT.md - "Alias (a1b2c3...)" format

fn format_tasklist_display(uuid: &str, aliases: &HashMap<String, String>) -> String {
    match aliases.get(uuid) {
        Some(alias) => {
            let short_uuid = &uuid[..8.min(uuid.len())];
            format!("{} ({}...)", alias, short_uuid)
        }
        None => uuid.to_string()
    }
}
```

### Auto-Discovery with RecursiveMode

```rust
// Source: notify docs - RecursiveMode::Recursive for auto-discovery

use notify::RecursiveMode;

fn start_watcher_with_discovery(
    tasks_base_dir: &Path,  // ~/.claude/tasks/
    tx: mpsc::Sender<SyncEvent>,
) -> Result<WatcherHandle, String> {
    // Watch the base directory recursively
    // New tasklist folders will automatically be watched
    let mut debouncer = new_debouncer(
        Duration::from_millis(200),
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    // Filter: only process events in UUID-named subdirectories
                    if is_tasklist_event(&event) {
                        let _ = tx.send(SyncEvent::from(event));
                    }
                }
            } else if let Err(errors) = result {
                for error in errors {
                    // Log but don't crash on transient errors
                    eprintln!("claude-tasks: watcher error: {:?}", error);
                }
            }
        },
    ).map_err(|e| format!("Failed to create watcher: {}", e))?;

    debouncer.watch(tasks_base_dir, RecursiveMode::Recursive)
        .map_err(|e| handle_watcher_error(&e))?;

    // ...
}

fn is_tasklist_event(event: &DebouncedEvent) -> bool {
    // Check if event is in a UUID-named folder (tasklist)
    // Pattern: ~/.claude/tasks/{uuid}/*.json
    event.paths.iter().any(|p| {
        p.extension().map(|e| e == "json").unwrap_or(false)
            && p.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .map(|s| s.len() == 36 && s.contains('-'))  // UUID format check
                .unwrap_or(false)
    })
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Explicit shutdown() method | Drop trait + signal flag | Rust idiom | Cleanup guaranteed even on panic |
| Blocking join() in Drop | Timeout or non-blocking | Best practice | Prevents plugin unload hangs |
| config crate ecosystem | toml + serde directly | 2024+ | Simpler, more control |
| petgraph for small graphs | Local HashMap-based | Project-specific | Less dependency, simpler mental model |

**Deprecated/outdated:**
- `confy` crate: Auto-generates paths, less control. Use toml + dirs directly.
- `config` crate: Complex for simple TOML needs. Use toml crate directly.

## Open Questions

1. **FfiConfigSchema implementation details**
   - What we know: `config_schema()` returns FfiConfigSchema, `on_config_loaded()` receives config
   - What's unclear: How totui exposes config UI to users; whether schema drives UI generation
   - Recommendation: Return schema with staleness_threshold and aliases fields; test with totui

2. **Error display mechanism**
   - What we know: CONTEXT.md says "status bar (persistent line at bottom)"
   - What's unclear: How plugin communicates errors to totui for display
   - Recommendation: Check if FfiCommand has error/status variant; if not, use eprintln for now

3. **Alias keybind implementation**
   - What we know: User wants keybind in totui to set alias
   - What's unclear: Whether plugin can register keybinds or if totui must implement
   - Recommendation: Focus on config file support; keybind likely requires totui changes

## Sources

### Primary (HIGH confidence)
- [notify 8.2.0 ErrorKind docs](https://docs.rs/notify/latest/notify/enum.ErrorKind.html) - Error variants verified
- [notify-debouncer-full docs](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/) - Drop behavior verified
- [toml crate docs](https://docs.rs/toml/latest/toml/) - Serde integration patterns
- [Rust Book ch20-03](https://doc.rust-lang.org/book/ch21-03-graceful-shutdown-and-cleanup.html) - Thread cleanup patterns
- [std::time::Instant docs](https://doc.rust-lang.org/std/time/struct.Instant.html) - Monotonic timing

### Secondary (MEDIUM confidence)
- [notify GitHub README](https://github.com/notify-rs/notify) - Platform limits workarounds
- [petgraph toposort docs](https://docs.rs/petgraph/latest/petgraph/algo/fn.toposort.html) - Cycle detection (not needed but verified)

### Tertiary (LOW confidence)
- Stack Overflow / Rust forums - Thread shutdown patterns (cross-referenced with Rust Book)

## Metadata

**Confidence breakdown:**
- Error handling: HIGH - Verified against notify 8.2.0 docs
- Thread cleanup: HIGH - Based on Rust Book official guidance
- Config loading: HIGH - toml crate well-documented
- Dependency hierarchy: MEDIUM - Local implementation based on CONTEXT.md constraints
- Staleness tracking: HIGH - std::time::Instant behavior verified

**Research date:** 2026-01-27
**Valid until:** 2026-02-27 (30 days - stable ecosystem)
