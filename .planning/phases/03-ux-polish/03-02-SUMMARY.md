---
phase: 03-ux-polish
plan: 02
subsystem: ui
tags: [guidance, lifecycle, ffi-commands, user-onboarding]

# Dependency graph
requires:
  - phase: 03-01
    provides: guidance module with create/clear functions and GuidanceState enum
provides:
  - Integrated guidance lifecycle in plugin
  - Setup guidance when no tasklists found
  - Waiting guidance when tasklist is empty
  - Error guidance with recovery instructions
  - Automatic guidance clearing when real tasks arrive
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - pending_commands pattern for deferred command delivery
    - take_pending_commands() for atomic return-and-clear

key-files:
  created: []
  modified:
    - claude-tasks/src/state.rs
    - claude-tasks/src/lib.rs
    - claude-tasks/src/guidance.rs

key-decisions:
  - "Use pending_commands field in SyncState for deferred command delivery from on_config_loaded"
  - "Return pending guidance on first OnLoad event to avoid blocking plugin initialization"
  - "Clear guidance before processing real sync events to ensure clean transition"

patterns-established:
  - "Deferred command delivery: Store commands in state during init, return on next event"
  - "Guidance lifecycle: Show on state detection, clear when real data arrives"

# Metrics
duration: 4min
completed: 2026-01-27
---

# Phase 3 Plan 2: Guidance Integration Summary

**Integrated guidance lifecycle into plugin - users see setup help, waiting state, and error recovery instructions automatically**

## Performance

- **Duration:** 4 min
- **Started:** 2026-01-27T16:44:00Z
- **Completed:** 2026-01-27T16:48:19Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Added pending_commands mechanism for deferred command delivery from on_config_loaded
- Integrated guidance creation for three states: no tasklists, empty tasklist, watcher error
- Automatic guidance clearing when real sync events arrive
- Fixed clippy warnings in guidance.rs

## Task Commits

Each task was committed atomically:

1. **Task 1: Add pending_commands field to SyncState** - `381e643` (feat)
2. **Task 2: Integrate guidance into on_config_loaded** - `20c6f43` (feat)
3. **Task 3: Integrate guidance delivery and clearing in on_event** - `e268cbd` (feat)

## Files Created/Modified
- `claude-tasks/src/state.rs` - Added pending_commands field and helper methods
- `claude-tasks/src/lib.rs` - Integrated guidance into plugin lifecycle (on_config_loaded and on_event)
- `claude-tasks/src/guidance.rs` - Added clippy allow attributes to fix warnings

## Decisions Made
- **Deferred command delivery:** Since on_config_loaded returns void, commands are stored in pending_commands and returned on the first OnLoad event
- **Guidance clearing placement:** Clear guidance before processing sync events so clear commands are prepended to the command list

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed clippy warnings in guidance.rs**
- **Found during:** Task 3 (verification step)
- **Issue:** clippy::vec_init_then_push warning on guidance creation functions
- **Fix:** Added #[allow(clippy::vec_init_then_push)] attributes to three functions
- **Files modified:** claude-tasks/src/guidance.rs
- **Verification:** cargo clippy -- -D warnings passes
- **Committed in:** e268cbd (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Clippy fix necessary for clean build. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Guidance integration complete
- Phase 3 (UX Polish) complete
- All 103 tests passing
- Plugin ready for release

---
*Phase: 03-ux-polish*
*Completed: 2026-01-27*
