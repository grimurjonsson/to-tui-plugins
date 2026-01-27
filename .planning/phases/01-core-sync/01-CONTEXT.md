# Phase 1: Core Sync - Context

**Gathered:** 2026-01-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Plugin that discovers Claude Code tasklists, watches for file changes, and syncs tasks to totui in real-time. User selects a tasklist, plugin watches it, changes appear in totui within 1 second. Read-only UX deferred to Phase 2.

</domain>

<decisions>
## Implementation Decisions

### Discovery UI
- Tasklists sorted alphabetically by name/ID (predictable order)
- Each entry shows: name + task count + last modified timestamp
- If no tasklists found: show empty state with explanation of where Claude stores tasklists
- Always show selection UI, even with single tasklist (no auto-select)

### Sync Indicators
- Brief flash/highlight animation on changed items when sync occurs
- If tasklist file becomes unavailable: clear tasks and show error state
- Staleness indicator: show "last synced" timestamp only when >1 minute since last sync
- Normal operation: no timestamp shown (clean header)

### Task Display
- Direct status mapping: pending→[ ], in_progress→[*], completed→[x]
- Preserve hierarchy: parent tasks have children nested/indented below
- Metadata shown: subject + owner (if assigned) + blocked status
- Blocked tasks marked with ⛔ prefix icon

### Claude's Discretion
- Exact highlight animation duration/style
- Error state message wording
- Header format details
- How to handle very deep nesting

</decisions>

<specifics>
## Specific Ideas

- Staleness threshold of 1 minute feels like a good UX balance - don't alarm users prematurely but surface problems reasonably quickly
- Using ⛔ for blocked is more universally understood than dimming

</specifics>

<deferred>
## Deferred Ideas

- Read-only UX handling (Phase 2 scope - covered by PLUG-04/PLUG-05)
- Tasklist aliases (Phase 2 - DISC-04)

</deferred>

---

*Phase: 01-core-sync*
*Context gathered: 2026-01-27*
