---
phase: 04-tasklist-selection
plan: 02
subsystem: config
tags: [rust, ffi, config-schema, select, discovery]

# Dependency graph
requires:
  - phase: 04-01
    provides: FfiConfigType::Select in totui-plugin-interface
provides:
  - config_schema with Select field for tasklist selection
  - generate_tasklist_options function for option generation
  - on_config_loaded reads selected tasklist from config
affects: [user-config]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "display|uuid format for Select options (display for user, uuid for storage)"
    - "Fallback pattern for config values (use specified or default to first)"

key-files:
  created: []
  modified:
    - claude-tasks/src/lib.rs
    - claude-tasks/src/config.rs

key-decisions:
  - "Options format uses display|uuid separator for totui parsing"
  - "Tasklist field is optional - auto-selects first if not specified"
  - "Fall back to first tasklist if configured UUID not found"

patterns-established:
  - "Select field options include rich display with task count and age"
  - "Config map reading with FfiConfigValue::String extraction"

# Metrics
duration: 3min
completed: 2026-01-27
---

# Phase 4 Plan 2: Config Schema with Select Field Summary

**Config schema now returns Select field with discovered tasklists; on_config_loaded reads user selection with fallback to auto-select first**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-27T18:09:26Z
- **Completed:** 2026-01-27T18:12:46Z
- **Tasks:** 4
- **Files modified:** 2

## Accomplishments

- config_schema returns FfiConfigSchema with tasklist Select field
- Options populated from discover_tasklists() with formatted display (alias, task count, age)
- on_config_loaded reads "tasklist" UUID from config map
- Falls back to auto-select first if not specified or UUID not found
- 110 unit tests passing (6 new tests for format_age and format_tasklist_option)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add generate_tasklist_options function** - `8850929` (feat)
2. **Task 2: Implement config_schema with Select field** - `3559a4b` (feat)
3. **Task 3: Update on_config_loaded to read selected tasklist** - `23d37ba` (feat)
4. **Task 4: Add unit tests for tasklist option generation** - `e5595ab` (test)

## Files Created/Modified

- `claude-tasks/src/config.rs` - Added generate_tasklist_options(), format_tasklist_option(), format_age() functions and 6 tests
- `claude-tasks/src/lib.rs` - Updated config_schema() to return Select field, on_config_loaded() to read selection

## Decisions Made

- **Options format "display|uuid":** Allows totui to show user-friendly display string while storing the UUID value
- **Field is optional:** Will auto-select first tasklist if user doesn't configure one
- **Graceful fallback:** If configured UUID not found (e.g., tasklist deleted), falls back to first available

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 4 (Tasklist Selection) is complete
- Plugin now supports interactive tasklist selection via config UI
- Ready for final testing with totui integration

---
*Phase: 04-tasklist-selection*
*Completed: 2026-01-27*
