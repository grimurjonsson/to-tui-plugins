# claude-tasks

## What This Is

A totui plugin that syncs Claude Code's native task lists into totui in real-time. Users select a Claude tasklist (from `~/.claude/tasks/`) and watch tasks update live as Claude works on them. This is a read-only bridge — Claude is the source of truth, totui provides visibility.

## Core Value

Real-time visibility into Claude Code's task progress without switching contexts.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Discover available Claude tasklists by scanning `~/.claude/tasks/`
- [ ] Display tasklist metadata (task count, last modified, recent task names) for selection
- [ ] Watch a selected tasklist folder for file changes via file system events
- [ ] Sync Claude tasks to totui todos (create, update state, update content)
- [ ] Map Claude task dependencies (blocks/blockedBy) to totui — parent-child hierarchy if feasible, annotation fallback
- [ ] Show header todo with tasklist info: "CLAUDE TASKLIST: {name/uuid} - last updated: {timestamp}"
- [ ] Mark stale tasklists visually when no updates for configurable duration
- [ ] Support watching multiple tasklists — one project per tasklist OR grouped in single project with prefix
- [ ] Allow naming tasklists via config for easier reference (aliases)
- [ ] Enforce read-only on synced todos (prevent editing in totui)

### Out of Scope

- Write-back to Claude tasks — Claude is source of truth, totui is read-only mirror
- Creating new Claude tasklists from totui
- Historical task tracking across Claude sessions
- Integration with other Claude features (conversations, memory)

## Context

**Claude Code Task Format:**
- Location: `~/.claude/tasks/{uuid}/` — one folder per tasklist
- Files: `1.json`, `2.json`, etc. — one file per task
- Schema: `{id, subject, description, activeForm, status, blocks[], blockedBy[]}`
- Status values: `pending`, `in_progress`, `completed`

**totui Plugin Interface:**
- Plugins implement `Plugin` trait from `totui-plugin-interface` crate
- Key methods: `generate()`, `execute_with_host()`, `on_event()`, `subscribed_events()`
- Host API provides: `query_todos()`, `get_todo()`, `SetTodoMetadata`, `CreateTodo`, `UpdateTodo`, `DeleteTodo`
- Event types: `OnLoad`, `OnAdd`, `OnModify`, `OnComplete`, `OnDelete`

**Existing Pattern:**
- jira-claude plugin uses `generate()` for on-demand todo creation
- claude-tasks needs continuous sync — will use `execute_with_host()` with file watcher thread

## Constraints

- **Rust**: Must be a Rust plugin using `totui-plugin-interface` (matches jira-claude)
- **File watching**: Use `notify` crate for cross-platform file system events
- **Read-only**: Synced todos must be marked via metadata and visually distinguished
- **No polling**: Prefer event-based file watching over interval polling

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| File watcher in plugin | Continuous sync requires reacting to external changes | — Pending |
| Metadata for tracking | Need to correlate totui todos back to Claude tasks for updates | — Pending |
| Header todo for context | User wants "CLAUDE TASKLIST: {name}" header showing sync status | — Pending |
| Parent-child for deps | blocks/blockedBy maps conceptually to parent-child; fallback to annotations if complex | — Pending |

---
*Last updated: 2026-01-27 after initialization*
