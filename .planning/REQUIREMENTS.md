# Requirements: claude-tasks

**Defined:** 2026-01-27
**Core Value:** Real-time visibility into Claude Code's task progress without switching contexts

## v1 Requirements

### Discovery

- [ ] **DISC-01**: Plugin can scan `~/.claude/tasks/` to list all available tasklist folders
- [ ] **DISC-02**: Plugin displays metadata for each tasklist (task count, last modified, sample task names)
- [ ] **DISC-03**: User can configure named aliases for tasklists in plugin config
- [ ] **DISC-04**: Plugin detects new tasklists that appear while running (auto-discovery)

### Watching

- [ ] **WATCH-01**: Plugin can start watching a selected tasklist folder for file changes
- [ ] **WATCH-02**: File events are debounced (100-300ms) to handle rapid writes from Claude
- [ ] **WATCH-03**: Plugin watches directories, not individual files (handles atomic writes correctly)
- [ ] **WATCH-04**: Plugin handles macOS FSEvents limits gracefully
- [ ] **WATCH-05**: Plugin handles Linux inotify limits gracefully with fallback or error message

### Sync

- [ ] **SYNC-01**: Plugin creates totui todos when new Claude task files appear
- [ ] **SYNC-02**: Plugin updates totui todos when Claude task files change (status, content, description)
- [ ] **SYNC-03**: Plugin deletes totui todos when Claude task files are removed
- [ ] **SYNC-04**: Plugin tracks correlation between Claude tasks and totui todos via metadata (claude_task_id, tasklist_id)
- [ ] **SYNC-05**: Plugin maps Claude task dependencies (blocks/blockedBy) to totui structure (parent-child if feasible, annotations as fallback)
- [ ] **SYNC-06**: Plugin batches rapid changes into grouped sync operations

### Display

- [ ] **DISP-01**: Plugin creates a header todo showing "CLAUDE TASKLIST: {name/alias} - last updated: {timestamp}"
- [ ] **DISP-02**: Synced todos have clear visual indicator that they are read-only (via naming or metadata)
- [ ] **DISP-03**: Plugin marks tasklist as stale when no updates received for configurable duration
- [ ] **DISP-04**: Plugin shows clear error messages when watch or sync operations fail

### Plugin Infrastructure

- [ ] **PLUG-01**: Plugin implements totui Plugin trait correctly
- [ ] **PLUG-02**: Plugin uses background thread for file watching (totui has no async runtime)
- [ ] **PLUG-03**: Plugin communicates with main thread via std::sync::mpsc channels
- [ ] **PLUG-04**: Plugin cleans up watcher thread on unload/shutdown
- [ ] **PLUG-05**: Plugin provides configuration schema for tasklist selection and options

### User Experience

- [x] **UX-01**: Plugin provides guidance when no Claude tasklists are found
- [x] **UX-02**: Plugin provides guidance when watching an empty tasklist
- [x] **UX-03**: Plugin provides actionable error recovery guidance

### Tasklist Selection

- [x] **SEL-01**: Plugin config_schema returns a Select field with available tasklists as options
- [x] **SEL-02**: Select options show alias (if any), UUID, task count, and last modified for each tasklist
- [x] **SEL-03**: Selected tasklist is persisted in plugin config and used on subsequent loads

## v2 Requirements

### Multi-Tasklist Support

- **MULTI-01**: Watch multiple tasklists simultaneously
- **MULTI-02**: Each tasklist appears as separate totui project
- **MULTI-03**: Grouped mode: all tasklists in single project with prefix

### Advanced Features

- **ADV-01**: Filter which tasks to sync (by status, age, pattern)
- **ADV-02**: Rich metadata display in todo description
- **ADV-03**: Historical tracking across Claude sessions

## Out of Scope

| Feature | Reason |
|---------|--------|
| Write-back to Claude tasks | Claude is source of truth; totui is read-only mirror |
| Create new Claude tasklists | Plugin is for viewing, not creating |
| Integration with Claude conversations | Task sync only; conversation features out of scope |
| Polling-based watching | Must use event-based file watching for real-time updates |
| Bidirectional sync | Adds conflict resolution complexity; one-way sync for v1 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DISC-01 | Phase 1 | Complete |
| DISC-02 | Phase 1 | Complete |
| DISC-03 | Phase 2 | Complete |
| DISC-04 | Phase 2 | Partial |
| WATCH-01 | Phase 1 | Complete |
| WATCH-02 | Phase 1 | Complete |
| WATCH-03 | Phase 1 | Complete |
| WATCH-04 | Phase 2 | Complete |
| WATCH-05 | Phase 2 | Complete |
| SYNC-01 | Phase 1 | Complete |
| SYNC-02 | Phase 1 | Complete |
| SYNC-03 | Phase 1 | Complete |
| SYNC-04 | Phase 1 | Complete |
| SYNC-05 | Phase 2 | Complete |
| SYNC-06 | Phase 2 | Complete |
| DISP-01 | Phase 1 | Complete |
| DISP-02 | Phase 1 | Complete |
| DISP-03 | Phase 2 | Complete |
| DISP-04 | Phase 2 | Complete |
| PLUG-01 | Phase 1 | Complete |
| PLUG-02 | Phase 1 | Complete |
| PLUG-03 | Phase 1 | Complete |
| PLUG-04 | Phase 2 | Complete |
| PLUG-05 | Phase 4 | Complete |
| UX-01 | Phase 3 | Complete |
| UX-02 | Phase 3 | Complete |
| UX-03 | Phase 3 | Complete |
| SEL-01 | Phase 4 | Complete |
| SEL-02 | Phase 4 | Complete |
| SEL-03 | Phase 4 | Complete |

**Coverage:**
- v1 requirements: 28 total
- Phase 1: 14 requirements
- Phase 2: 8 requirements
- Phase 3: 3 requirements
- Phase 4: 3 requirements
- Unmapped: 0

---
*Requirements defined: 2026-01-27*
*Last updated: 2026-01-27 after Phase 4 completion (milestone complete)*
