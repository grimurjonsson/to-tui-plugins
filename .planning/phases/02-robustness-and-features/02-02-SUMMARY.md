---
phase: 02-robustness-and-features
plan: 02
subsystem: config
tags: [toml, config, aliases, rust, serde]

# Dependency graph
requires:
  - phase: 01-core-sync
    provides: sync engine and state management
provides:
  - PluginConfig struct with aliases HashMap and staleness_threshold
  - load_config() with global/local merge semantics
  - format_tasklist_display() for alias resolution
  - Header todo displays alias instead of UUID when configured
affects: [staleness-detection, selection-ui, future-config-options]

# Tech tracking
tech-stack:
  added: [toml 0.8]
  patterns: [TOML config merging, config-in-state pattern]

key-files:
  created: [claude-tasks/src/config.rs]
  modified: [claude-tasks/Cargo.toml, claude-tasks/src/lib.rs, claude-tasks/src/state.rs, claude-tasks/src/commands.rs, claude-tasks/src/sync.rs]

key-decisions:
  - "Global config at ~/.config/totui/claude-tasks.toml"
  - "Local config at .totui/aliases.toml overrides global"
  - "Staleness threshold defaults to 15 minutes"
  - "Config stored in SyncState for access during sync"

patterns-established:
  - "Config loading: global + local merge with local override"
  - "Optional display_name parameter for header commands"

# Metrics
duration: 3min
completed: 2026-01-27
---

# Phase 02 Plan 02: Config Module Summary

**TOML configuration with tasklist aliases and staleness threshold, loaded from global/local paths with merge semantics**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-27T15:50:34Z
- **Completed:** 2026-01-27T15:53:47Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments
- PluginConfig struct with aliases HashMap and optional staleness_threshold_minutes
- load_config() merges ~/.config/totui/claude-tasks.toml with .totui/aliases.toml
- format_tasklist_display() returns "Alias (a1b2c3...)" when alias configured
- Header todo shows friendly alias name instead of raw UUID
- 7 comprehensive config module tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Add toml dependency and create config module** - `6238f2f` (feat)
2. **Task 2: Integrate config into plugin and update header display** - `2bb629f` (feat)
3. **Task 3: Add config tests** - `4b42160` (test)

## Files Created/Modified
- `claude-tasks/src/config.rs` - New config module with PluginConfig, load_config, format_tasklist_display
- `claude-tasks/Cargo.toml` - Added toml 0.8 dependency
- `claude-tasks/src/lib.rs` - Import config functions, load in on_config_loaded, pass alias to sync
- `claude-tasks/src/state.rs` - Added config field to SyncState
- `claude-tasks/src/commands.rs` - create_header_command accepts optional display_name
- `claude-tasks/src/sync.rs` - process_initial_scan_local accepts alias parameter

## Decisions Made
- Global config path: ~/.config/totui/claude-tasks.toml (platform-appropriate via dirs crate)
- Local config path: .totui/aliases.toml (project-specific overrides)
- Local aliases extend (override) global aliases
- Local staleness_threshold_minutes replaces global if specified
- Default staleness threshold: 15 minutes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required

None - no external service configuration required. Users can optionally create config files at:
- `~/.config/totui/claude-tasks.toml` (global)
- `.totui/aliases.toml` (project-local)

Example config:
```toml
staleness_threshold_minutes = 20

[aliases]
"abc-123-def-456" = "My Project"
```

## Next Phase Readiness
- Config infrastructure ready for staleness detection feature (02-03)
- Aliases fully functional, header todo shows friendly names
- 66 total tests passing

---
*Phase: 02-robustness-and-features*
*Completed: 2026-01-27*
