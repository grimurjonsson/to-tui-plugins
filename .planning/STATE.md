# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-27)

**Core value:** Real-time visibility into Claude Code's task progress without switching contexts
**Current focus:** Phase 4 - Tasklist Selection UI - COMPLETE

## Current Position

Phase: 4 of 4 (Tasklist Selection)
Plan: 2 of 2 in current phase
Status: PHASE COMPLETE
Last activity: 2026-01-27 - Completed 04-02-PLAN.md

Progress: [##########] 100%

**All phases complete!** Plugin fully functional with tasklist selection UI.

## Performance Metrics

**Velocity:**
- Total plans completed: 11
- Average duration: 3.0 min
- Total execution time: 37 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Core Sync | 4/4 | 14 min | 3.5 min |
| 2. Robustness | 4/4 | 10 min | 2.5 min |
| 3. UX Polish | 3/3 | 10 min | 3.3 min |
| 4. Tasklist Selection | 2/2 | 3 min | 1.5 min |

**Recent Trend:**
- Last 5 plans: 02-04 (3 min), 03-01 (5 min), 03-02 (4 min), 03-03 (1 min), 04-02 (3 min)
- Trend: Consistent

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Used Mutex<Option<Receiver>> pattern for lazy watcher initialization
- Subscribe to OnLoad events for checking watcher updates
- Auto-select first tasklist (selection UI deferred to Phase 2)
- Watch directories not files (handles atomic writes correctly)
- 200ms debounce timeout for event batching
- Metadata includes source, tasklist_id, task_id, read_only, blocked_by
- Use local known_tasks HashSet to track synced tasks without HostApi queries
- Predictable todo IDs (claude-{tasklist}-{task_id}) enable update without lookup
- on_event returns commands directly via FfiHookResponse
- Global config at ~/.config/totui/claude-tasks.toml
- Local config at .totui/aliases.toml overrides global
- Staleness threshold defaults to 15 minutes
- Config stored in SyncState for access during sync
- **UPDATED:** Flat task list - all tasks at same indent level under header
- **UPDATED:** Blocked tasks show "(blocked by: A, B)" annotation suffix (no nesting)
- Circular dependencies shown at root with warning emoji
- Duration format: Xm for <60 min, XhYm for 60+ min
- Staleness indicator uses alarm clock emoji (U+23F0)
- Header updates only when tracking is active
- Guidance IDs use claude-guidance-* prefix for regular, claude-error-* for errors
- GuidanceState enum tracks None, NoTasklists, EmptyTasklist, Error states
- guidance_shown boolean for tracking displayed guidance todos
- Use pending_commands field in SyncState for deferred command delivery from on_config_loaded
- Return pending guidance on first OnLoad event to avoid blocking plugin initialization
- Clear guidance before processing real sync events to ensure clean transition
- Filter guidance clearing by SyncEvent::FileChanged only (InitialScan/FileRemoved don't trigger)
- **NEW:** Options format uses display|uuid separator for totui parsing
- **NEW:** Tasklist field is optional - auto-selects first if not specified
- **NEW:** Fall back to first tasklist if configured UUID not found

### Pending Todos

None.

### Blockers/Concerns

None.

### Roadmap Evolution

- Phase 3 added: UX Polish - Improve discoverability and user onboarding experience
- Plan 03-03 added: Gap closure for guidance clearing logic
- Phase 4 added: Tasklist Selection - Interactive tasklist picker via config UI (requires totui interface extension)
- **Phase 4 complete**: Config schema with Select field implemented

## Session Continuity

Last session: 2026-01-28T09:52:53Z
Stopped at: Completed quick-001-01-PLAN.md (Flatten Dependency Hierarchy)
Resume file: None

Previous plan context (quick-001-01-SUMMARY.md):
- Removed parent-child nesting from hierarchy
- All tasks now at flat list under header (indent_level 1)
- Blocked tasks show "(blocked by: X)" annotation suffix
- 109 unit tests passing

## Completed Phases

- Phase 1: Core Sync (4/4 plans) - COMPLETE
- Phase 2: Robustness and Features (4/4 plans) - COMPLETE
- Phase 3: UX Polish (3/3 plans) - COMPLETE
- Phase 4: Tasklist Selection (2/2 plans) - COMPLETE

The plugin is fully functional with all planned features:
- Real-time file watching and sync
- Error handling and graceful shutdown
- Config module with aliases and staleness threshold
- Flat task list with "(blocked by: X)" annotations
- Staleness detection with visual indicator
- Guidance for setup, waiting, and error states
- Automatic guidance lifecycle management
- Correct guidance clearing (only on real task arrivals)
- Tasklist selection UI via config schema Select field

## Quick Tasks

- quick-001: Flatten Dependency Hierarchy - COMPLETE (2026-01-28)
