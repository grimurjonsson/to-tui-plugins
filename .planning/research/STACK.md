# Stack Research

**Domain:** Rust totui plugin with file watching and data sync
**Researched:** 2026-01-27
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.92.0 (stable) | Plugin implementation | Required by totui-plugin-interface; matches existing jira-claude plugin |
| abi_stable | 0.11.3 | FFI-safe plugin interface | Required by totui-plugin-interface for stable ABI across dynamic library boundaries |
| totui-plugin-interface | git (main) | Plugin trait and FFI types | Required interface for totui plugins; provides Plugin trait, FfiTodoItem, HostApi |
| notify | 8.2.0 | Cross-platform file watching | De facto standard for Rust file watching; used by rust-analyzer, deno, cargo-watch; supports inotify (Linux), FSEvents (macOS), ReadDirectoryChanges (Windows) |
| serde | 1.0.x | Serialization framework | Required for JSON parsing; already used in jira-claude; industry standard |
| serde_json | 1.0.149 | JSON parsing | Parse Claude task files (*.json); already used in jira-claude |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| notify-debouncer-mini | 0.7.0 | Event debouncing | Use when file changes trigger multiple rapid events; prevents duplicate processing |
| uuid | 1.20.0 | UUID generation/parsing | Generate todo IDs; parse Claude task UUIDs from folder names |
| parking_lot | 0.12.x | Fast synchronization primitives | If sharing state between watcher thread and plugin callbacks; 1-byte Mutex vs larger std::sync |
| crossbeam-channel | 0.5.x | MPMC channels | Only if needing multiple receivers or select! macro; std::sync::mpsc sufficient for single-consumer |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo build | Standard Rust build | Use `--release` for production plugins |
| cargo clippy | Linting | Catches common mistakes; run before commits |
| cargo test | Unit testing | Plugin logic can be tested without FFI boundary |

## Installation

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
notify-debouncer-mini = "0.7"

# Utilities - Required
uuid = { version = "1.20", features = ["v4"] }

# Synchronization - Optional (if sharing state across threads)
# parking_lot = "0.12"
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| notify 8.x | watchexec | If you need CLI-like file watching with glob patterns and process spawning; overkill for library use |
| notify 8.x | inotify (direct) | If Linux-only and need maximum control; loses cross-platform support |
| notify-debouncer-mini | notify-debouncer-full | If you need rename correlation across events or path adjustment for queued events; adds complexity |
| std::sync::mpsc | crossbeam-channel | If you need multiple consumers (MPMC), select! macro, or timeout operations; std::sync::mpsc uses crossbeam internally since Rust 1.67 |
| std::sync::Mutex | parking_lot::Mutex | If you need 1-byte mutex size, deadlock detection, or fair locking; std::sync is simpler and sufficient for most cases |
| serde_json | simd-json | If parsing large JSON files (>1MB) frequently; Claude task files are small |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| async-watcher | Adds tokio dependency; totui-plugin-interface is synchronous | notify with std::thread |
| tokio/async-std | Plugin interface is synchronous; async adds complexity without benefit | std::thread for background watcher |
| hotwatch | Abandoned; last update 2021; notify is actively maintained | notify 8.x |
| fs_extra | For file operations, not watching | notify |
| inotify crate directly | Linux-only; loses macOS/Windows support | notify (abstracts platform differences) |
| polling crate | Low-level; notify already handles platform abstraction | notify |

## Stack Patterns by Variant

**If you need debouncing (recommended):**
- Use `notify-debouncer-mini` with configurable timeout
- Prevents rapid-fire events from triggering duplicate syncs
- 500ms-1000ms debounce typical for file editors

**If you need immediate notification:**
- Use raw `notify` without debouncer
- Handle duplicate/rapid events in your code
- Useful if you need to track intermediate states

**If Claude tasks folder doesn't exist initially:**
- Watch parent directory (`~/.claude/`) for folder creation
- Switch to watching `~/.claude/tasks/{uuid}/` once target exists
- Use `notify::RecursiveMode::Recursive` for nested watching

## Threading Model

The totui-plugin-interface is **synchronous** - no async runtime. For file watching:

```
+----------------+     channel      +------------------+
| Watcher Thread | --------------> | Plugin Callbacks |
| (notify loop)  |   std::sync::   | (on_event, etc.) |
+----------------+     mpsc        +------------------+
        |                                    |
        v                                    v
   File System                         FfiCommands
   Events                             to Host API
```

**Pattern:**
1. Spawn watcher thread on plugin load (via `on_config_loaded` or first `execute_with_host`)
2. Use `std::sync::mpsc::channel` for watcher -> plugin communication
3. In `on_event(OnLoad)` or `on_event(OnModify)`, check channel with `try_recv()`
4. Return `FfiCommand::CreateTodo` / `FfiCommand::UpdateTodo` / `FfiCommand::DeleteTodo`

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| notify 8.x | Rust 1.85+ | MSRV increased in notify 8.x; verify your Rust toolchain |
| abi_stable 0.11.x | Rust 1.61+ | Supports older Rust but features may require newer |
| serde 1.0.x | All Rust editions | Stable API for years |
| uuid 1.20.x | serde 1.0.x | Enable `serde` feature for serialization |

## Critical Constraints

### notify MSRV
**IMPORTANT:** notify 8.x requires Rust 1.85+. The codebase uses Rust 1.92.0, so this is compatible. If targeting older Rust, use notify 6.x (MSRV 1.60).

### abi_stable Compatibility
Each 0.y.0 version of abi_stable defines its own ABI. The plugin **must** use the same abi_stable version as totui-plugin-interface. Currently 0.11.x.

### File Watcher Limitations

| Platform | Limitation | Workaround |
|----------|------------|------------|
| NFS/Network mounts | May not emit events | Use `PollWatcher` backend |
| WSL watching Windows paths | Events may not fire | Use `PollWatcher` backend |
| Docker on macOS M1 | "Function not implemented" error | Use `PollWatcher` backend |
| macOS | FSEvents has ~1s latency | Acceptable for task sync use case |

### Claude Task File Location
`~/.claude/tasks/{uuid}/*.json` - Expand `~` using `dirs` crate or `std::env::var("HOME")`.

## Sources

- [notify crate docs.rs](https://docs.rs/notify/8.2.0/notify/) - Version 8.2.0 verified
- [notify-rs/notify GitHub](https://github.com/notify-rs/notify) - MSRV 1.85, CC Zero license
- [notify-debouncer-mini docs.rs](https://docs.rs/notify-debouncer-mini/0.7.0/notify_debouncer_mini/) - Version 0.7.0 verified
- [notify-debouncer-full docs.rs](https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/) - Version 0.7.0 verified
- [serde_json docs.rs](https://docs.rs/serde_json/1.0.149/serde_json/) - Version 1.0.149 verified
- [uuid docs.rs](https://docs.rs/uuid/1.20.0/uuid/) - Version 1.20.0 verified
- [abi_stable docs.rs](https://docs.rs/abi_stable/0.11.3/abi_stable/) - Version 0.11.3 verified, MSRV 1.61
- [totui-plugin-interface source](https://github.com/grimurjonsson/to-tui/tree/main/crates/totui-plugin-interface) - Plugin trait is synchronous
- [Rust channel comparison](https://codeandbitters.com/rust-channel-comparison/) - std::sync::mpsc uses crossbeam since Rust 1.67
- [parking_lot GitHub](https://github.com/Amanieu/parking_lot) - MSRV 1.84, performance comparison

---
*Stack research for: claude-tasks totui plugin*
*Researched: 2026-01-27*
