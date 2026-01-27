---
phase: 03-ux-polish
plan: 01
subsystem: ui
tags: [guidance, ux, state, ffi-commands]

# Dependency graph
requires:
  - phase: 02-robustness
    provides: Plugin foundation with error handling and state management
provides:
  - Guidance module with todo creation functions
  - GuidanceState enum for tracking UX state
  - Guidance tracking fields in SyncState
affects: [03-02, 03-03, 03-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Guidance todo IDs for lifecycle management"
    - "Centralized message constants"
    - "GuidanceState enum for UX flow tracking"

key-files:
  created:
    - claude-tasks/src/guidance.rs
  modified:
    - claude-tasks/src/state.rs
    - claude-tasks/src/lib.rs

key-decisions:
  - "9 unique guidance IDs for all guidance scenarios"
  - "Centralized message constants for easy i18n if needed"
  - "GuidanceState enum with 4 variants: None, NoTasklists, EmptyTasklist, Error"
  - "guidance_shown boolean tracks whether guidance todos are displayed"

patterns-established:
  - "Guidance IDs: claude-guidance-* prefix for all guidance todos"
  - "Guidance metadata: {source, type, error?} JSON structure"
  - "FfiTodoState.Question for setup guidance, Exclamation for errors"

# Metrics
duration: 5min
completed: 2026-01-27
---

# Phase 3 Plan 1: Guidance Module Foundation Summary

**Guidance module with 4 creation functions, 9 ID constants, GuidanceState enum, and state tracking fields for in-context user help**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-27T16:38:28Z
- **Completed:** 2026-01-27T16:43:00Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Created guidance.rs module with todo creation functions for 3 UX states
- Added GuidanceState enum with None, NoTasklists, EmptyTasklist, Error variants
- Integrated guidance tracking into SyncState with set/clear/is_shown methods
- Added 22 new tests (16 guidance + 6 state) bringing total to 100

## Task Commits

Each task was committed atomically:

1. **Task 1: Create guidance module with todo creation functions** - `606a861` (feat)
2. **Task 2: Add GuidanceState enum and tracking to SyncState** - `01485d2` (feat)
3. **Task 3: Export guidance module from lib.rs** - `a4c2b22` (feat)

## Files Created/Modified
- `claude-tasks/src/guidance.rs` - Guidance todo creation functions with 9 ID constants
- `claude-tasks/src/state.rs` - GuidanceState enum and tracking fields
- `claude-tasks/src/lib.rs` - Module export for guidance

## Decisions Made
- Used consistent ID prefix `claude-guidance-*` for regular guidance, `claude-error-*` for error guidance
- Centralized all user-facing text as message constants for maintainability
- GuidanceState enum uses `#[default]` attribute for None variant
- clear_guidance() deletes all 9 IDs (safe even if they don't exist)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - implementation straightforward.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Guidance module ready for integration with main plugin flow (Plan 03-02)
- All functions return valid FfiCommand sequences
- GuidanceState and tracking ready for use in on_config_loaded

---
*Phase: 03-ux-polish*
*Completed: 2026-01-27*
