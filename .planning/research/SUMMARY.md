# Project Research Summary

**Project:** claude-tasks totui plugin
**Domain:** Real-time file-watching sync plugin for todo application (Rust)
**Researched:** 2026-01-27
**Confidence:** HIGH

## Executive Summary

This is a read-only sync plugin for the totui todo manager that watches Claude Code's task files (`~/.claude/tasks/{uuid}/*.json`) and displays them in real-time. Expert implementations use the `notify` crate with debouncing to handle file system events, avoiding common pitfalls like race conditions on partial writes and event flooding. The recommended approach uses `notify-debouncer-full` watching parent directories (not individual files), with a synchronous threading model that matches totui's plugin interface.

The core technical challenge is robust file watching across platforms (macOS FSEvents, Linux inotify, Windows ReadDirectoryChanges) while handling the rapid file updates that Claude Code generates during agent operations. The architecture follows an event-driven polling hybrid: file watcher for changes, initial directory scan for existing state, and metadata-based correlation to track which totui todos came from which Claude tasks. The plugin should be read-only by design to avoid data conflicts—Claude Code is the source of truth.

Critical risks include race conditions from reading partially-written files (mitigate with 100-300ms debouncing), event flooding from editor save patterns (use notify-debouncer-full), and cross-platform behavior differences (test on Linux + macOS early). The Linux inotify watch limit requires documentation and graceful degradation. Starting with MVP features (single tasklist, basic sync, read-only) validates the concept before adding complexity like multiple tasklists or dependency visualization.

## Key Findings

### Recommended Stack

The stack centers on Rust 1.92.0 with the notify ecosystem for cross-platform file watching. The totui-plugin-interface requires `abi_stable` 0.11.3 for FFI safety, and synchronous (non-async) architecture eliminates tokio/async-std complexity.

**Core technologies:**
- **notify 8.2.0**: Cross-platform file watching — de facto standard, used by rust-analyzer and deno, abstracts inotify/FSEvents/ReadDirectoryChanges
- **notify-debouncer-full 0.7.0**: Event debouncing with rename tracking — prevents event flood, merges rapid writes into single events, handles atomic write patterns correctly
- **totui-plugin-interface (git main)**: Plugin trait and FFI types — required interface for totui, provides Plugin trait, FfiTodoItem, HostApi
- **serde + serde_json 1.0.x**: JSON parsing — parse Claude task files, industry standard
- **uuid 1.20.0**: UUID parsing — parse Claude task folder names (session IDs)

**Critical constraints:**
- notify 8.x requires Rust 1.85+ (satisfied by project's 1.92.0)
- abi_stable version must exactly match totui-plugin-interface (currently 0.11.x)
- Synchronous threading model required (no tokio/async runtime)
- Linux inotify limits (~8192 watches) require documentation or fallback to PollWatcher

### Expected Features

Research identified clear feature tiers: table stakes for MVP, competitive differentiators for post-launch, and anti-features to explicitly avoid.

**Must have (table stakes):**
- Tasklist Discovery — scan `~/.claude/tasks/` for UUID directories, users need to see available tasklists
- Tasklist Selection — choose which session to watch, display metadata (task count, last modified)
- Real-time File Watching — core value prop, instant visibility of task changes
- Task Sync (Create/Update) — parse JSON, map to FfiTodoItem, handle status changes (pending/in_progress/completed)
- Sync Status Indicator — header todo shows "CLAUDE TASKLIST: {name} - synced: {timestamp}" so users know sync is active
- Read-Only Enforcement — metadata marker prevents editing, avoids data loss confusion

**Should have (competitive):**
- Dependency Mapping — show blockedBy relationships as parent-child hierarchy
- Stale Detection — visual indicator when no updates for configurable time (answers "is Claude still working?")
- Tasklist Naming/Aliases — UUIDs are unfriendly, config mapping to human-readable names
- Intelligent Debouncing — batch rapid file changes into single sync, use notify-debouncer-full

**Defer (v2+):**
- Multiple Tasklist Support — watch multiple Claude sessions simultaneously (high complexity, needs separate projects per tasklist)
- Auto-Discovery of New Tasklists — watch parent directory for new sessions (conflicts with simple single-watcher model)
- Selective Task Filtering — watch only specific statuses (nice-to-have, not essential)

**Anti-features (deliberately NOT building):**
- Write-back to Claude tasks — bidirectional sync creates conflicts, unclear ownership, data corruption risk
- Task creation from totui — breaks Claude's workflow model, user should launch Claude Code instead
- Historical task tracking — scope creep, unclear what "history" means for Claude sessions
- Aggressive caching — task files are tiny, caching adds staleness risk

### Architecture Approach

The architecture uses an event-driven polling hybrid with clear separation between file watching, sync logic, and plugin integration. File watcher (notify + debouncer) monitors the filesystem, event processor maps FS events to domain events, sync engine reconciles states, and command generator produces FfiCommand batches for totui.

**Major components:**
1. **File Watcher** (watcher/) — Monitor `~/.claude/tasks/` using notify-debouncer-full, debounce 100-300ms, watch parent directories not individual files to handle atomic writes
2. **Sync Engine** (sync/) — Reconciliation logic comparing Claude task state vs totui todo state, pure functions for testability, metadata-based correlation using (session_id, task_id) pairs
3. **State Manager** (sync/state.rs) — Track known state from both sources in `Mutex<HashMap<TaskId, TaskState>>`, shared between watcher thread and plugin callbacks
4. **Event Processor** (watcher/events.rs) — Convert FS events (Create/Modify/Remove) to domain events, parse JSON, handle partial writes with retry logic
5. **Command Generator** (commands.rs) — Thin layer mapping domain decisions to FfiCommand (CreateTodo, UpdateTodo, DeleteTodo, SetTodoMetadata)

**Threading model:** Synchronous with channels. Spawn watcher thread on plugin load, use `std::sync::mpsc::channel` for watcher → plugin communication, check channel with `try_recv()` in `on_event()`, return FfiCommand arrays. No async runtime.

**Build order:** Core types + parser (Claude task structs, JSON) → State manager (pure Rust data structures) → File watcher (notify setup, event mapping) → Sync engine (reconciliation, depends on prior phases) → Plugin integration (wire into Plugin trait).

### Critical Pitfalls

Research revealed seven critical pitfalls, all well-documented with mitigation strategies:

1. **Reading Partially-Written Files** — Race condition where watcher fires before write completes, causing parse failures. Mitigate with 100-300ms debouncing, retry logic with exponential backoff, check file size stability. Address in Phase 1 (Core Watcher).

2. **Not Using Debouncers** — Single file save triggers 3-5 events (truncate, write, metadata), causing duplicate work and state corruption. Use `notify-debouncer-full` (not mini) for proper event merging and rename tracking. Address in Phase 1 (Core Watcher).

3. **Watching Individual Files** — Atomic writes (temp file → rename) orphan file watchers. Watch parent directories and filter by filename. Address in Phase 1 (Core Watcher) as architectural decision.

4. **Cross-Platform Event Differences** — FSEvents (macOS) batches events, inotify (Linux) is per-operation, different granularity and recursion support. Test on Linux + macOS, use debouncer to normalize, design for "file changed, reload it" not specific event types. Address in Phase 1 (Core Watcher) with CI testing.

5. **Tokio/Async Runtime Conflicts** — notify's crossbeam-channel interferes with tokio, causing hung tasks. Since totui-plugin-interface is synchronous, avoid async entirely or disable notify default features. Address in Phase 1 (Core Watcher) via dependency configuration.

6. **Linux inotify Watch Limits** — System default ~8192 watches exhausted by multiple file-watching apps. Document limit increase (`fs.inotify.max_user_watches=524288`), provide clear error messages, consider PollWatcher fallback. Address in Phase 2 (Robustness).

7. **Watched Directory Deletion** — Deleting watched `~/.claude/tasks/{uuid}/` causes undefined behavior (silent failure, continuous errors, resource leaks). Watch parent directory, handle removal events, verify paths still exist periodically. Address in Phase 2 (Robustness).

## Implications for Roadmap

Based on architecture dependencies and pitfall prevention, suggested three-phase approach:

### Phase 1: Core Watcher and Basic Sync
**Rationale:** Establish file watching infrastructure correctly from the start. All critical pitfalls (#1-5) must be addressed here—changing watcher architecture later is expensive.

**Delivers:** Single tasklist selection, real-time file watching with debouncing, basic task sync (create/update from Claude → totui), sync status indicator, read-only enforcement.

**Addresses Features:** Tasklist Discovery, Tasklist Selection, Real-time File Watching, Task Sync (Create/Update), Sync Status Indicator, Read-Only Enforcement (all P1 table stakes).

**Avoids Pitfalls:**
- #1 (partial reads) via notify-debouncer-full with 100-300ms timeout
- #2 (event flood) via debouncer choice
- #3 (orphan watchers) via directory watching
- #4 (cross-platform) via CI testing on Linux + macOS
- #5 (async conflicts) via synchronous-only architecture

**Stack Elements:** notify 8.2.0, notify-debouncer-full 0.7.0, serde/serde_json, uuid, totui-plugin-interface, abi_stable.

**Architecture:** File Watcher + Event Processor + State Manager (basic HashMap) + Command Generator + Plugin integration.

**Validation:** MVP functional—can watch one tasklist, see tasks appear/update in real-time, verify no parse errors under rapid writes, confirm read-only markers prevent edits.

### Phase 2: Robustness and Error Handling
**Rationale:** Address production-readiness concerns (inotify limits, directory lifecycle, error recovery) before adding feature complexity.

**Delivers:** Graceful handling of inotify limits, dynamic directory watching (handle deletion/recreation), retry logic for transient errors, clear error messages and status indicators, initial sync on plugin load (scan existing files before starting watcher).

**Addresses Features:** Improves reliability of Phase 1 features, no new user-facing features but essential for production use.

**Avoids Pitfalls:**
- #6 (inotify limits) via documentation, error messages, fallback to PollWatcher
- #7 (directory deletion) via parent directory watching, cleanup on Remove events

**Stack Elements:** Same as Phase 1, potentially add PollWatcher as fallback backend.

**Architecture:** Enhanced State Manager (lifecycle tracking), improved Event Processor (retry with backoff), watcher restart logic.

**Validation:** Plugin survives watched directory deletion, provides useful error on inotify exhaustion, recovers from transient filesystem errors, initial scan matches filesystem state.

### Phase 3: Advanced Features
**Rationale:** With robust core established, add competitive differentiators. These are independent features that can be prioritized based on user feedback.

**Delivers:** Dependency mapping (blockedBy → parent_id hierarchy), stale detection (timeout-based indicator), tasklist naming/aliases (UUID → friendly names), rich metadata display (activeForm, owner, timestamps in todo description).

**Addresses Features:** Dependency Mapping (P2), Stale Detection (P2), Tasklist Naming (P2), Rich Metadata (P3).

**Stack Elements:** Same as prior phases.

**Architecture:** Enhanced Command Generator (dependency mapping logic), State Manager (track last update timestamps for staleness), Config (alias mappings).

**Validation:** Blocking relationships visible as hierarchy, stale tasks marked after configured timeout, aliases resolve correctly, metadata enriches todo descriptions.

### Deferred (Post-Launch)
- **Multiple Tasklist Support:** High complexity (separate projects or prefix grouping), defer until users run multiple Claude sessions concurrently
- **Auto-Discovery:** Conflicts with simple watcher model, add when manual selection becomes friction
- **Selective Filtering:** Low value until tasklists exceed ~50 tasks

### Phase Ordering Rationale

- **Phase 1 before Phase 2:** Must establish correct watcher patterns before hardening—architectural changes are expensive after initial implementation. All file-watching pitfalls must be addressed in Phase 1 or they become technical debt.

- **Phase 2 before Phase 3:** Production robustness required before adding features. Users will encounter inotify limits and directory lifecycle issues in real use; these must work reliably before adding nice-to-haves.

- **MVP stops at Phase 2:** Phases 1-2 deliver complete, reliable one-way sync. Phase 3 features are competitive differentiators, not requirements for validation. Launch after Phase 2, gather feedback, prioritize Phase 3 features based on actual user needs.

### Research Flags

**Phases needing deeper research during planning:**
- **Phase 3 (Advanced Features):** Dependency mapping requires understanding totui's parent_id semantics and rendering. If parent_id doesn't support DAGs (directed acyclic graphs), need alternative approach for blockedBy relationships. May need `/gsd:research-phase` to verify totui hierarchy behavior.

**Phases with standard patterns (skip research-phase):**
- **Phase 1 (Core Watcher):** File watching with notify is well-documented pattern. Official docs + debouncer examples provide clear implementation path. PITFALLS.md already covers edge cases comprehensively.
- **Phase 2 (Robustness):** Error handling patterns are standard Rust. inotify limit documentation exists. No novel integration needed.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | **HIGH** | All libraries verified via official docs, versions confirmed on docs.rs, totui-plugin-interface inspected in local cargo checkout. MSRV requirements clear. |
| Features | **MEDIUM** | Claude task JSON schema verified via example files. Table stakes vs differentiators derived from file-watching sync patterns (Obsidian Tasks, Taskwarrior, todosync), not Claude-specific research. User expectations inferred from general sync plugin UX. |
| Architecture | **HIGH** | Component structure follows standard Rust patterns. totui-plugin-interface source code reviewed, Plugin trait and FFI types confirmed. Threading model dictated by interface (synchronous). Build order follows dependency graph. |
| Pitfalls | **HIGH** | All critical pitfalls sourced from notify GitHub issues, official docs warnings, and Deno/other project issue trackers. Mitigation strategies verified in notify-debouncer-full docs and fswatch documentation. Cross-platform differences documented in parcel-bundler watcher issues. |

**Overall confidence:** **HIGH**

The stack, architecture, and pitfalls are backed by authoritative sources (official docs, library source code, verified issues). Features have medium confidence because user expectations are extrapolated from comparable products rather than Claude-specific user research. However, the table stakes features are clearly necessary (file watching, task sync) and differentiators can be validated post-launch.

### Gaps to Address

- **Totui parent_id hierarchy semantics:** Research didn't verify whether totui supports DAG dependencies (task blocked by multiple tasks) or only tree hierarchies (single parent). Phase 3 dependency mapping depends on this. Validate during Phase 3 planning via totui source code inspection or testing.

- **Claude task file atomicity guarantees:** Assumed Claude Code uses atomic writes (temp file + rename) but not verified. If Claude writes in-place, may need stronger partial-write handling (checksum validation, JSON streaming parser). Monitor during Phase 1 development, add mitigation if parse errors occur despite debouncing.

- **Totui plugin unload lifecycle:** Research didn't cover totui's plugin unload mechanism. Need to verify whether Plugin trait has drop/shutdown hook or if watcher cleanup happens via RAII. Check during Phase 1 implementation, ensure watcher thread joins cleanly on plugin unload.

- **Multiple sessions UX:** Deferred feature (multiple tasklist support) needs UX research if prioritized. Unclear whether users want separate projects per tasklist, grouped todos with prefixes, or tabbed interface. Gather feedback during Phase 2 usage before committing to approach.

## Sources

### Primary (HIGH confidence)
- [notify crate docs.rs v8.2.0](https://docs.rs/notify/8.2.0/notify/) — File watching API, MSRV, platform backends
- [notify-debouncer-full docs.rs v0.7.0](https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/) — Debouncing features, rename tracking
- [notify-rs/notify GitHub](https://github.com/notify-rs/notify) — Issues #380 (tokio conflicts), #165-166 (directory deletion), known problems docs
- [abi_stable docs.rs v0.11.3](https://docs.rs/abi_stable/0.11.3/abi_stable/) — FFI safety requirements, version compatibility
- [totui-plugin-interface source](https://github.com/grimurjonsson/to-tui/tree/main/crates/totui-plugin-interface) — Plugin trait, HostApi, FfiCommand types
- [serde_json docs.rs v1.0.149](https://docs.rs/serde_json/1.0.149/serde_json/) — JSON parsing API
- [uuid docs.rs v1.20.0](https://docs.rs/uuid/1.20.0/uuid/) — UUID parsing with serde feature

### Secondary (MEDIUM confidence)
- [Deno Issue #13035: File watcher race condition](https://github.com/denoland/deno/issues/13035) — Partial write race conditions
- [watchexec: Linux inotify limits](https://watchexec.github.io/docs/inotify-limits.html) — Resource limits, sysctl configuration
- [fsnotify Go library docs](https://pkg.go.dev/github.com/fsnotify/fsnotify) — Atomic write patterns, orphan watchers
- [parcel-bundler watcher issue #171](https://github.com/parcel-bundler/watcher/issues/171) — FSEvents vs inotify differences
- [Claude Code Swarm Orchestration gist](https://gist.github.com/kieranklaassen/4f2aba89594a4aea4ad64d753984b2ea) — Task JSON schema structure
- [JetBrains: Inotify Watches Limit](https://intellij-support.jetbrains.com/hc/en-us/articles/15268113529362-Inotify-Watches-Limit-Linux) — Production inotify issues

### Tertiary (LOW confidence)
- [Todosync](https://github.com/RichardGomer/todosync) — Multi-source sync patterns (different domain but relevant patterns)
- [Taskwarrior](https://github.com/GothenburgBitFactory/taskwarrior) — CLI task management UX patterns
- [Obsidian Tasks](https://github.com/obsidian-tasks-group/obsidian-tasks) — Comparable file-watching sync plugin UX
- [OutSystems Read-Only Data Patterns](https://success.outsystems.com/documentation/11/developing_an_application/use_data/offline/offline_data_sync_patterns/read_only_data/) — Sync architecture general patterns

---
*Research completed: 2026-01-27*
*Ready for roadmap: yes*
