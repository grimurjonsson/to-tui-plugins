# Roadmap: claude-tasks

## Overview

This roadmap delivers a totui plugin that provides real-time visibility into Claude Code's task progress. Phase 1 establishes the core sync loop (discovery, watching, sync, display) using the notify ecosystem with proper debouncing. Phase 2 adds robustness (platform-specific handling, cleanup, configuration) and advanced features (dependency mapping, staleness detection, aliases) that make the plugin production-ready.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Core Sync** - Establish file watching and basic task synchronization
- [x] **Phase 2: Robustness and Features** - Platform handling, cleanup, and advanced features
- [x] **Phase 3: UX Polish** - Improve discoverability and user onboarding experience
- [x] **Phase 4: Tasklist Selection** - Interactive tasklist picker via config UI

## Phase Details

### Phase 1: Core Sync
**Goal**: User can watch a Claude tasklist and see tasks sync to totui in real-time
**Depends on**: Nothing (first phase)
**Requirements**: DISC-01, DISC-02, WATCH-01, WATCH-02, WATCH-03, SYNC-01, SYNC-02, SYNC-03, SYNC-04, DISP-01, DISP-02, PLUG-01, PLUG-02, PLUG-03
**Success Criteria** (what must be TRUE):
  1. User can select a Claude tasklist from discovered options showing task count and last modified
  2. When Claude creates/updates/completes a task, the change appears in totui within 1 second
  3. User sees a header todo displaying "CLAUDE TASKLIST: {name} - last updated: {timestamp}"
  4. Synced todos are visually marked as read-only and cannot be edited in totui
  5. Plugin runs continuously without blocking totui's main thread
**Plans**: 4 plans

Plans:
- [x] 01-01-PLAN.md — Project scaffolding, ClaudeTask types, Plugin trait skeleton
- [x] 01-02-PLAN.md — Tasklist discovery and file watcher infrastructure
- [x] 01-03-PLAN.md — Sync engine with create/update/delete reconciliation
- [x] 01-04-PLAN.md — Gap closure: Wire on_event to return sync commands (fixes real-time sync)

### Phase 2: Robustness and Features
**Goal**: Plugin handles edge cases gracefully and supports advanced sync features
**Depends on**: Phase 1
**Requirements**: DISC-03, DISC-04, WATCH-04, WATCH-05, SYNC-05, SYNC-06, DISP-03, DISP-04, PLUG-04, PLUG-05
**Success Criteria** (what must be TRUE):
  1. Plugin provides clear error message when macOS FSEvents or Linux inotify limits are reached
  2. When plugin unloads, watcher thread stops cleanly with no orphaned processes
  3. User can configure aliases for tasklists (UUID -> friendly name) and see them in selection
  4. Claude task dependencies (blockedBy) appear as parent-child hierarchy in totui
  5. Tasklist shows stale indicator when no updates received for configured duration
**Plans**: 4 plans

Plans:
- [x] 02-01-PLAN.md — Platform error handling and graceful watcher shutdown
- [x] 02-02-PLAN.md — TOML configuration with tasklist aliases
- [x] 02-03-PLAN.md — Dependency hierarchy visualization (parent-child relationships)
- [x] 02-04-PLAN.md — Staleness detection and header indicator

### Phase 3: UX Polish
**Goal**: User can easily discover how to use the plugin and start syncing without documentation
**Depends on**: Phase 2
**Requirements**: UX-01 (guidance for no tasklists), UX-02 (guidance for empty tasklist), UX-03 (error recovery guidance)
**Success Criteria** (what must be TRUE):
  1. User sees clear instructions or prompts when plugin loads but no tasklist is being watched
  2. User can easily trigger tasklist discovery and selection
  3. Plugin provides helpful feedback when no Claude tasklists are found
  4. Common workflows are intuitive without reading external docs
  5. Error states provide actionable guidance
**Plans**: 3 plans

Plans:
- [x] 03-01-PLAN.md — Guidance module and state tracking (Wave 1)
- [x] 03-02-PLAN.md — Integrate guidance into plugin lifecycle (Wave 2)
- [x] 03-03-PLAN.md — Gap closure: Fix guidance clearing to filter by event type

### Phase 4: Tasklist Selection
**Goal**: User can interactively select which tasklist to watch when adding the plugin
**Depends on**: Phase 3, totui-plugin-interface Select type
**Requirements**: SEL-01, SEL-02, SEL-03
**Success Criteria** (what must be TRUE):
  1. When adding claude-tasks plugin, user sees a dropdown of available tasklists (not empty input)
  2. Each tasklist option shows alias (if configured), UUID, task count, and last modified
  3. Selected tasklist is persisted and used on subsequent plugin loads
  4. User can change selection by reconfiguring the plugin
**Plans**: TBD (requires totui interface extension first)

Plans:
- [x] 04-01-PLAN.md — Extend totui-plugin-interface with FfiConfigType::Select (DONE in to-tui v0.2.0)
- [x] 04-02-PLAN.md — Wire config_schema to return Select field with discovered tasklists

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Sync | 4/4 | ✓ Complete | 2026-01-27 |
| 2. Robustness and Features | 4/4 | ✓ Complete | 2026-01-27 |
| 3. UX Polish | 3/3 | ✓ Complete | 2026-01-27 |
| 4. Tasklist Selection | 2/2 | ✓ Complete | 2026-01-27 |
