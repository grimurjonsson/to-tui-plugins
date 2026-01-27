# Feature Research: Claude Tasks Sync Plugin

**Domain:** Real-time file-watching sync plugin for todo application
**Researched:** 2026-01-27
**Confidence:** MEDIUM (patterns derived from ecosystem research, Claude Code task format verified)

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Tasklist Discovery** | Users need to see available tasklists before watching | LOW | Scan `~/.claude/tasks/` for UUID directories |
| **Tasklist Selection** | Users must choose which tasklist to sync | LOW | Display list with metadata (task count, last modified) |
| **Real-time File Watching** | Core value prop - instant visibility | MEDIUM | Use `notify` crate with debouncing |
| **Task Sync (Create)** | New Claude tasks must appear in totui | LOW | Parse JSON, create `FfiTodoItem` |
| **Task Sync (Update Status)** | Status changes (pending/in_progress/completed) must reflect | LOW | Map to `FfiTodoState` |
| **Task Sync (Update Content)** | Subject/description changes must sync | LOW | Update existing todos via metadata correlation |
| **Sync Status Indicator** | Users need to know sync is working | LOW | Header todo: "CLAUDE TASKLIST: {name} - synced: {timestamp}" |
| **Read-Only Enforcement** | Edits in totui would create confusion/data loss | MEDIUM | Mark via metadata, prevent modification |
| **Graceful Error Handling** | Missing files, parse errors shouldn't crash | LOW | Log warnings, continue operation |
| **Clean Shutdown** | Stop watching when plugin unloads | LOW | Drop watcher, release file handles |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Dependency Mapping** | Show blocks/blockedBy as visual hierarchy | MEDIUM | Parent-child mapping for blocking tasks |
| **Stale Detection** | Visual indicator when no updates for configurable time | LOW | Track last update timestamp, mark stale after threshold |
| **Multiple Tasklist Support** | Watch multiple Claude sessions simultaneously | HIGH | Separate projects per tasklist OR grouped with prefixes |
| **Tasklist Naming/Aliases** | UUIDs are unfriendly; aliases improve UX | LOW | Config file mapping UUID to human-readable name |
| **Auto-Discovery of New Tasklists** | Watch `~/.claude/tasks/` for new sessions | MEDIUM | Parent directory watcher, add/remove child watchers |
| **Rich Metadata Display** | Show activeForm (spinner text), owner, timestamps | LOW | Include in todo description |
| **Intelligent Debouncing** | Batch rapid file changes into single sync | MEDIUM | Use `notify-debouncer-full` for clean event stream |
| **Selective Task Filtering** | Watch only specific task statuses | LOW | Config option: sync all vs pending-only vs in_progress-only |

### Anti-Features (Deliberately NOT Building)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Write-back to Claude tasks** | Bidirectional sync feels complete | Claude is source of truth; write-back creates conflict, data corruption, unclear ownership | Read-only sync with clear "source of truth" messaging |
| **Task Creation from totui** | Users might want to add tasks | Claude manages its own tasklists; external creation breaks Claude's workflow model | User launches Claude Code to create tasks |
| **Historical Task Tracking** | See completed tasks over time | Scope creep, storage complexity, unclear what "history" means for Claude sessions | Focus on current session; Claude manages its own archives |
| **Polling-based Sync** | Simpler than file watching | CPU waste, battery drain, delayed updates | Event-based watching with `notify` crate |
| **Auto-resolve Conflicts** | Handle simultaneous changes | With read-only sync, no conflicts exist; adding resolution implies write capability | Don't write; no conflicts to resolve |
| **Deep Integration with Claude Conversations** | Link tasks to conversation context | Huge scope expansion, unclear API, privacy concerns | Stick to tasks JSON; conversations are separate |
| **Real-time Streaming UI Updates** | Push-based TUI updates | totui plugin model may not support push; forces architectural complexity | Reasonable refresh rate on plugin query |
| **Aggressive Caching** | Speed up repeat queries | JSON files are tiny; caching adds staleness risk, memory overhead | Read from disk each time; it's fast |

## Feature Dependencies

```
[Tasklist Discovery]
    |
    v
[Tasklist Selection]
    |
    v
[File Watching (notify)]
    |
    +---> [Task Sync: Create] --------+
    |                                 |
    +---> [Task Sync: Update Status] -+---> [Sync Status Indicator]
    |                                 |
    +---> [Task Sync: Update Content]-+
    |
    v
[Read-Only Enforcement] (metadata marker)

[Dependency Mapping] --requires--> [Task Sync: Create] (need tasks before mapping deps)

[Stale Detection] --requires--> [Sync Status Indicator] (need last-sync timestamp)

[Multiple Tasklist Support] --requires--> [Tasklist Selection] (selection per project)

[Auto-Discovery] --conflicts--> Simple single-watcher model (adds complexity)
```

### Dependency Notes

- **File Watching requires Discovery + Selection:** Must know which directory to watch before starting watcher
- **Sync operations are independent:** Create, Update Status, Update Content can execute in any order
- **Read-Only Enforcement applies to all synced todos:** Must mark metadata on every create/update
- **Dependency Mapping enhances Create:** Add parent_id based on blockedBy after creating tasks
- **Stale Detection enhances Sync Status:** Uses same "last update" tracking, adds timeout logic
- **Multiple Tasklists is additive:** Works with all other features, just multiplied

## MVP Definition

### Launch With (v1)

Minimum viable product - what's needed to validate the concept.

- [x] **Tasklist Discovery** - Scan ~/.claude/tasks/ and list available tasklists
- [x] **Tasklist Selection** - User picks one tasklist to watch
- [x] **Real-time File Watching** - notify crate watches selected folder
- [x] **Task Sync (Create/Update)** - JSON parse, map to FfiTodoItem
- [x] **Sync Status Indicator** - Header todo shows "CLAUDE TASKLIST: {uuid}"
- [x] **Read-Only Enforcement** - Metadata marker prevents editing

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **Dependency Mapping** - Add when users request hierarchy visualization
- [ ] **Stale Detection** - Add when users request "is Claude still working?" indicator
- [ ] **Tasklist Naming** - Add when UUID unfriendliness becomes friction
- [ ] **Intelligent Debouncing** - Add if performance issues arise from rapid file changes

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Multiple Tasklist Support** - Add when users run multiple Claude sessions
- [ ] **Auto-Discovery** - Add when manual selection feels cumbersome
- [ ] **Selective Task Filtering** - Add when tasklists become too large
- [ ] **Rich Metadata Display** - Add when users want more context

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Tasklist Discovery | HIGH | LOW | P1 |
| Tasklist Selection | HIGH | LOW | P1 |
| Real-time File Watching | HIGH | MEDIUM | P1 |
| Task Sync (Create/Update) | HIGH | LOW | P1 |
| Sync Status Indicator | MEDIUM | LOW | P1 |
| Read-Only Enforcement | HIGH | MEDIUM | P1 |
| Dependency Mapping | MEDIUM | MEDIUM | P2 |
| Stale Detection | MEDIUM | LOW | P2 |
| Tasklist Naming | LOW | LOW | P2 |
| Intelligent Debouncing | MEDIUM | LOW | P2 |
| Multiple Tasklist Support | MEDIUM | HIGH | P3 |
| Auto-Discovery | LOW | MEDIUM | P3 |
| Selective Filtering | LOW | LOW | P3 |
| Rich Metadata | LOW | LOW | P3 |

**Priority key:**
- P1: Must have for launch (MVP)
- P2: Should have, add when possible (post-launch)
- P3: Nice to have, future consideration

## Competitor/Comparable Feature Analysis

| Feature | Todosync (todo.txt) | Taskwarrior sync | Obsidian Tasks | Our Approach |
|---------|---------------------|------------------|----------------|--------------|
| Source Discovery | Config-defined sources | Single server | File globs | Directory scan |
| Real-time Updates | Polling-based | Sync on demand | Polling | Event-based (notify) |
| Read-only Mode | N/A (bidirectional) | N/A (bidirectional) | N/A | First-class, enforced |
| Dependency Visualization | None | Dependencies with UDA | Blocking notation | Parent-child hierarchy |
| Stale Indicators | None | Sync status | None | Timestamp-based timeout |
| Multi-source | Yes (aggregation) | Single server | Multiple files | Multiple tasklists |

**Key differentiation:**
- **Read-only by design:** Unlike bidirectional sync tools, we explicitly prevent write-back
- **Event-based watching:** Most tools poll; we use OS-level file events for instant updates
- **Claude-specific format:** Direct JSON parsing of Claude's schema vs generic adapters

## Claude Code Task Format Reference

Based on verified research, Claude Code stores tasks in:

**Location:** `~/.claude/tasks/{uuid}/`
- One folder per tasklist (UUID-named)
- Individual task files: `1.json`, `2.json`, etc.

**Task JSON Schema:**
```json
{
  "id": "1",
  "subject": "Task title",
  "description": "Detailed description",
  "status": "pending|in_progress|completed",
  "owner": "teammate-name",
  "activeForm": "Status spinner text",
  "blockedBy": ["2", "3"],
  "blocks": ["4"],
  "createdAt": 1706000000000,
  "updatedAt": 1706000001000
}
```

**Mapping to totui:**
| Claude Field | totui Field | Notes |
|--------------|-------------|-------|
| subject | content | Task title |
| description | description | Full details |
| status | state | pending->Empty, in_progress->Partial, completed->Complete |
| blockedBy | parent_id | First blocking task becomes parent (simplification) |
| id | metadata["claude_task_id"] | Correlation for updates |

## Sources

**HIGH Confidence (Official/Authoritative):**
- [notify crate documentation](https://docs.rs/notify/latest/notify/) - File watching API
- [notify-debouncer-full crate](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/) - Debouncing features
- [Claude Code official docs](https://code.claude.com/docs/en/common-workflows) - Workflow patterns

**MEDIUM Confidence (Multiple sources agree):**
- [Claude Code Swarm Orchestration guide](https://gist.github.com/kieranklaassen/4f2aba89594a4aea4ad64d753984b2ea) - Task JSON structure
- [Chokidar documentation](https://github.com/paulmillr/chokidar) - Debounce/batch patterns (Node.js but patterns apply)
- [OutSystems Read-Only Data Patterns](https://success.outsystems.com/documentation/11/developing_an_application/use_data/offline/offline_data_sync_patterns/read_only_data/) - Sync architecture

**LOW Confidence (Single source, pattern extrapolation):**
- [Todosync](https://github.com/RichardGomer/todosync) - Multi-source sync patterns
- [Taskwarrior](https://github.com/GothenburgBitFactory/taskwarrior) - CLI task management patterns
- Various file watcher debouncing articles - Event batching strategies

---
*Feature research for: claude-tasks totui plugin*
*Researched: 2026-01-27*
