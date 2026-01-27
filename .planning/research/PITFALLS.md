# Pitfalls Research

**Domain:** File-watching sync plugin (Rust notify crate watching JSON files)
**Researched:** 2026-01-27
**Confidence:** HIGH (verified via official docs, GitHub issues, and multiple credible sources)

## Critical Pitfalls

### Pitfall 1: Reading Partially-Written Files (Race Condition)

**What goes wrong:**
The file watcher emits a `Modify` event before the write operation completes. Your code immediately reads the file and gets empty or truncated JSON content, causing parse failures or incorrect state.

**Why it happens:**
File systems emit notifications when writes begin, not when they complete. When an external process (like Claude Code) writes to a file, it may truncate the file first then write new content. The watcher sees the truncate and fires an event before new content arrives.

**How to avoid:**
1. Use debouncing with a 50-200ms delay before reading files
2. Implement retry logic: if JSON parse fails or file is empty, retry after a short delay with exponential backoff
3. Check file size stability: wait until file size stops changing
4. Consider watching for "close" events when available

**Warning signs:**
- Intermittent JSON parse failures
- Empty file content in logs
- Tests that pass sometimes and fail others
- Errors more common on slower systems or under load

**Phase to address:**
Phase 1 (Core Watcher) - Must implement debouncing from the start

**Sources:**
- [Deno Issue #13035: File watcher race condition](https://github.com/denoland/deno/issues/13035)
- [notify docs: debouncer recommendation](https://docs.rs/notify/latest/notify/)

---

### Pitfall 2: Not Using Debouncers (Event Flood)

**What goes wrong:**
A single file save from an editor like VSCode triggers 3-5 events in rapid succession (truncate, write, write, metadata update). Processing each event individually causes duplicate work, UI flicker, and potential state corruption from interleaved operations.

**Why it happens:**
Developers assume "one save = one event" but file systems and editors don't work that way. Different editors have different save strategies (truncate+write vs temp-file+rename), each producing multiple events.

**How to avoid:**
Use `notify-debouncer-full` (not `notify-debouncer-mini`) because:
- It properly merges multiple events per file into one
- It handles rename events correctly (important for atomic writes)
- It tracks file system IDs for reliable cross-platform behavior
- It emits only one Remove event for directory deletions on Linux

Configure debounce timeout of 100-300ms for JSON files.

**Warning signs:**
- Seeing 3-5 log entries per file save
- UI updates multiple times for single change
- State inconsistencies when processing events out of order

**Phase to address:**
Phase 1 (Core Watcher) - Choose `notify-debouncer-full` from the start

**Sources:**
- [notify-debouncer-full docs](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/)
- [notify-debouncer-mini docs](https://docs.rs/notify-debouncer-mini/latest/notify_debouncer_mini/)

---

### Pitfall 3: Watching Individual Files Instead of Directories

**What goes wrong:**
You watch `~/.claude/tasks/{uuid}/task.json` directly. Claude Code writes to a temp file and renames it (atomic write pattern). Your watcher on the original file is now orphaned - it points to a deleted inode and receives no further events.

**Why it happens:**
Many programs use atomic writes for safety: write to temp file, then rename over target. This ensures no partial content on crash. But it replaces the inode, orphaning any watchers on the original file.

**How to avoid:**
- Watch the parent directory (`~/.claude/tasks/{uuid}/`) instead of individual files
- Filter events by filename in your handler
- Use recursive watching on `~/.claude/tasks/` and filter by path pattern

**Warning signs:**
- Events stop arriving after first update
- Works with some editors but not others
- "Stale watcher" behavior - initial load works, updates don't

**Phase to address:**
Phase 1 (Core Watcher) - Architectural decision needed from start

**Sources:**
- [fsnotify Go library docs](https://pkg.go.dev/github.com/fsnotify/fsnotify)
- [notify docs: editor behavior variability](https://docs.rs/notify/latest/notify/)

---

### Pitfall 4: Cross-Platform Event Differences

**What goes wrong:**
Code works on macOS (FSEvents) but fails on Linux (inotify) or Windows (ReadDirectoryChangesW). Each backend has different:
- Event granularity (FSEvents batches events, inotify is per-operation)
- Recursion support (inotify requires watch per directory)
- Event types and semantics
- Resource limits

**Why it happens:**
OS-level file watching APIs are fundamentally different. The notify crate abstracts them but cannot hide all differences. Developers test on one platform and assume cross-platform compatibility.

**How to avoid:**
1. Test on all target platforms (at minimum: macOS and Linux for this project)
2. Use `notify-debouncer-full` which normalizes some platform differences
3. Avoid relying on specific event types - design for "file changed, reload it"
4. Handle gracefully when expected events don't arrive

**Warning signs:**
- Works on dev machine (macOS) but fails in CI (Linux)
- Different event counts per platform
- Rename tracking issues (macOS sends paired events, Linux may not)

**Phase to address:**
Phase 1 (Core Watcher) - Test infrastructure needed early

**Sources:**
- [parcel-bundler watcher issue: FSEvents vs inotify differences](https://github.com/parcel-bundler/watcher/issues/171)
- [fswatch monitors documentation](https://emcrisostomo.github.io/fswatch/doc/1.14.0/fswatch.html/Monitors.html)

---

### Pitfall 5: Tokio/Async Runtime Conflicts with Crossbeam

**What goes wrong:**
Using notify in a tokio async context causes tasks to hang or never complete. Specifically, the last spawned task in a batch never fires.

**Why it happens:**
The notify crate uses crossbeam-channel by default. Crossbeam can interfere with tokio's internal scheduling. Issue #380 documents this: sending 4 items, all 4 received, but only 3 process in spawned tasks.

**How to avoid:**
Disable default features and use an alternative channel:
```toml
[dependencies]
notify = { version = "7", default-features = false, features = ["macos_kqueue"] }
# Or use notify-debouncer-full which allows feature selection
```

Or bridge to tokio channels in your event handler before any async work.

**Warning signs:**
- Tasks hang indefinitely
- Last item in batch never processes
- Works with sync code, fails in async

**Phase to address:**
Phase 1 (Core Watcher) - Dependency configuration, must decide early

**Sources:**
- [notify-rs/notify Issue #380: Crossbeam breaks tokio::spawn](https://github.com/notify-rs/notify/issues/380)
- [PR #425: make crossbeam-channels optional](https://github.com/notify-rs/notify/pull/425)

---

### Pitfall 6: Linux inotify Watch Limits

**What goes wrong:**
On Linux, watching fails with "No space left on device" or "too many open files" error. The system has a default limit of ~8192 watches, and each file/directory counts toward it.

**Why it happens:**
`fs.inotify.max_user_watches` is system-wide per user. If user runs multiple file-watching applications (VS Code, Docker, your plugin), they compete for the same pool. Recursive watches on large directories exhaust limits quickly.

**How to avoid:**
1. Document the requirement to increase limits:
   ```bash
   echo fs.inotify.max_user_watches=524288 | sudo tee -a /etc/sysctl.conf
   sudo sysctl -p
   ```
2. Provide clear error message when limit is hit
3. Consider using PollWatcher as fallback on resource exhaustion
4. Watch only necessary directories, avoid recursive on large trees

**Warning signs:**
- Works in dev, fails in production/CI
- "No space left on device" errors on filesystem with plenty of space
- Errors correlate with other file-watching tools running

**Phase to address:**
Phase 2 (Robustness) - Error handling and documentation

**Sources:**
- [watchexec: Linux inotify limits](https://watchexec.github.io/docs/inotify-limits.html)
- [JetBrains: Inotify Watches Limit](https://intellij-support.jetbrains.com/hc/en-us/articles/15268113529362-Inotify-Watches-Limit-Linux)

---

### Pitfall 7: Watched Directory Deletion/Rename

**What goes wrong:**
If `~/.claude/tasks/{uuid}/` is deleted while being watched, behavior is undefined. The watcher may:
- Silently stop working
- Emit errors continuously
- Leak resources
- Crash

**Why it happens:**
When a watched path disappears, the OS-level watcher enters an error state. The notify crate documents this as "unexpected behavior" (see issues #165, #166).

**How to avoid:**
1. Watch the parent directory (`~/.claude/tasks/`) and manage child watches dynamically
2. Handle removal events and clean up associated state
3. Periodically verify watched paths still exist
4. Implement reconnection logic for disappeared paths

**Warning signs:**
- Errors in logs after task completion (directory cleaned up)
- Memory growth from accumulating orphan watchers
- "Path not found" errors after directory deletion

**Phase to address:**
Phase 2 (Robustness) - Lifecycle management for dynamic directories

**Sources:**
- [notify docs: known problems with renamed/removed paths](https://docs.rs/notify/latest/notify/)

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip debouncing | Faster initial dev | Event flood issues, race conditions | Never for production |
| Watch files directly | Simpler code | Broken atomic writes, orphan watchers | Never - always watch directories |
| Ignore platform differences | Works on dev machine | Fails on other platforms | Never |
| Sync file reads in event handler | Simpler code | Blocks watcher thread, missed events | Only for small files with short reads |
| Single retry on parse failure | Handles most cases | Flaky under load | MVP only, replace with proper backoff |
| No cleanup on unwatch | Simpler code | Memory leaks, resource exhaustion | Never |

## Integration Gotchas

Common mistakes when connecting file watcher to totui state.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Initial load | Only watch for changes, miss existing files | Scan directory on startup, then start watching |
| State updates | Update state directly in watcher callback | Send events to channel, update state in main thread |
| Error handling | Panic on JSON parse failure | Log error, skip file, continue watching |
| File deletion | Leave orphan tasks in totui state | Handle Remove events, clean up corresponding state |
| New task directories | Watch only at startup | Handle directory creation events, add new watches |
| Concurrent access | Read file while totui also reads | Use proper synchronization or accept eventual consistency |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Read full file on every event | Works fine | Only read if file actually changed (check mtime/size) | 100+ active tasks |
| Store all tasks in memory | Fast access | Paginate or lazy-load | 1000+ tasks |
| No event batching | UI updates correctly | Batch updates within render frame | 10+ rapid file changes |
| Recursive watch on home dir | Catches everything | Watch specific paths only | Any scale - too many events |
| Blocking file I/O in async | Works in dev | Use async file I/O | Under concurrent load |

## Security Mistakes

Domain-specific security issues for this plugin.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Following symlinks blindly | Path traversal, reading unintended files | Resolve symlinks, verify paths stay within expected directory |
| No path validation | Malicious task file could reference system paths | Validate all paths against expected patterns |
| Reading arbitrary JSON | Malformed JSON could cause DoS | Set size limits, use streaming parser for large files |
| World-readable watch state | Leaks task information | Ensure proper file permissions on any state files |

## UX Pitfalls

Common user experience mistakes for file-watching plugins.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No feedback on watcher errors | User thinks plugin is broken | Show status indicator, allow manual refresh |
| Updates without visual feedback | User doesn't notice changes | Subtle highlight on updated items |
| Lag between file change and UI update | Feels unresponsive | Target <500ms latency end-to-end |
| Lost updates on debounce | User's rapid edits lost | Debounce per-file, not globally |
| No offline/error recovery | Plugin stays broken until restart | Auto-reconnect, manual refresh option |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **File watcher:** Often missing symlink handling - verify symlink behavior
- [ ] **Debouncing:** Often missing per-file debounce - verify rapid saves to same file coalesce
- [ ] **Initial load:** Often missing existing file scan - verify tasks present before watcher starts appear
- [ ] **Cleanup:** Often missing unwatch on directory deletion - verify no leaked watchers
- [ ] **Error recovery:** Often missing retry logic - verify watcher restarts after errors
- [ ] **Cross-platform:** Often missing Linux/Windows testing - verify on all target platforms
- [ ] **Resource limits:** Often missing inotify limit handling - verify graceful degradation
- [ ] **Concurrent writes:** Often missing partial write handling - verify JSON parse with debounce

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Race condition (partial read) | LOW | Retry with backoff, log for debugging |
| Event flood | MEDIUM | Add debouncing, may need state reconciliation |
| Orphan watcher (atomic write) | MEDIUM | Restructure to watch directories, update watch targets |
| Cross-platform failure | HIGH | Add platform-specific tests, may need backend-specific code |
| Tokio conflict | MEDIUM | Change notify features, bridge to tokio channels |
| inotify limit | LOW | Document limit increase, add fallback to polling |
| Watched dir deleted | MEDIUM | Add lifecycle management, parent directory watching |
| State inconsistency | HIGH | Implement full state reconciliation from filesystem |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Partial file reads | Phase 1: Core Watcher | Test with rapid writes, verify no parse errors |
| Event flood | Phase 1: Core Watcher | Log shows single event per save |
| Watch files vs directories | Phase 1: Core Watcher | Atomic writes work, no orphan watchers |
| Cross-platform differences | Phase 1: Core Watcher | CI passes on Linux and macOS |
| Tokio/crossbeam conflict | Phase 1: Core Watcher | Async tests pass, no hung tasks |
| inotify limits | Phase 2: Robustness | Clear error message, graceful degradation |
| Directory deletion | Phase 2: Robustness | No errors after task directory removed |
| State consistency | Phase 2: Robustness | Manual refresh matches filesystem state |

## Sources

- [Deno Issue #13035: File watcher race condition](https://github.com/denoland/deno/issues/13035)
- [notify-rs/notify GitHub repository](https://github.com/notify-rs/notify)
- [notify docs.rs documentation](https://docs.rs/notify/latest/notify/)
- [notify-debouncer-full docs](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/)
- [notify Issue #380: Crossbeam breaks tokio::spawn](https://github.com/notify-rs/notify/issues/380)
- [fsnotify Go library documentation](https://pkg.go.dev/github.com/fsnotify/fsnotify)
- [fswatch monitors documentation](https://emcrisostomo.github.io/fswatch/doc/1.14.0/fswatch.html/Monitors.html)
- [watchexec inotify limits documentation](https://watchexec.github.io/docs/inotify-limits.html)
- [Apple Race Conditions documentation](https://developer.apple.com/library/archive/documentation/Security/Conceptual/SecureCodingGuide/Articles/RaceConditions.html)
- [parcel-bundler watcher issue #171: FSEvents vs inotify](https://github.com/parcel-bundler/watcher/issues/171)

---
*Pitfalls research for: File-watching sync plugin (claude-tasks)*
*Researched: 2026-01-27*
