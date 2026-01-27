# Phase 2: Robustness and Features - Context

**Gathered:** 2026-01-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Make the plugin production-ready with graceful error handling, clean shutdown, dependency visualization, and user-facing configuration. Includes platform-specific handling (FSEvents/inotify limits), watcher cleanup, tasklist aliases, task dependency display, and staleness detection.

</domain>

<decisions>
## Implementation Decisions

### Error handling UX
- Display errors in status bar (persistent line at bottom)
- Keep error messages brief — just state the problem, no fix hints
- Auto-reconnect when watcher recovers, notify user when reconnected
- Retry transient errors (permission denied, file locked) silently — only surface if persistent

### Dependency visualization
- Indent blocked tasks under their blocker when there's a single blocker
- Multiple blockers: show task at root level with "⛔ Blocked by: Task A, Task B" annotation
- No tree lines — indentation alone indicates hierarchy
- Circular dependencies: show all involved tasks at root level with "⚠ Circular dependency" warning
- Keep completed blockers' children nested (preserve hierarchy)
- Max nesting depth: 2-3 levels
- Tasks exceeding max depth: flatten to root with full chain "⛔ Blocked by: A → B → C"
- Read-only mirror — no special transitions, just reflect file state

### Staleness indication
- Threshold: 15 minutes without updates triggers stale state
- Location: Header todo modification ("CLAUDE TASKLIST: X ⏰ STALE (23m)")
- Show duration since last update, not just "STALE" flag
- Silently clear stale indicator when updates resume (no notification)

### Alias configuration
- Storage: Both global (~/.config/totui/claude-tasks.toml) and local (.totui/aliases.toml), local overrides global
- Format: TOML
- Setting aliases: Keybind in totui when viewing a tasklist (no need to edit config file manually)
- Selection display: "Alias (a1b2c3...)" — show alias plus truncated UUID

### Claude's Discretion
- Exact retry count and backoff strategy for transient errors
- Specific keybind for setting aliases
- Exact truncation length for UUIDs in selection display

</decisions>

<specifics>
## Specific Ideas

- Read-only mirror principle: don't animate or transition, just reflect what's in the files
- Hierarchy preserved even after blocker completes — matches mental model of dependency chain

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-robustness-and-features*
*Context gathered: 2026-01-27*
