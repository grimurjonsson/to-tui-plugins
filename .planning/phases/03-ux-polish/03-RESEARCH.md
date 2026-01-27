# Phase 3: UX Polish - Research

**Researched:** 2026-01-27
**Domain:** Plugin UX patterns - empty states, first-run onboarding, actionable feedback, progressive disclosure
**Confidence:** MEDIUM

## Summary

This phase improves the claude-tasks plugin's user experience by addressing first-time user confusion. Currently, the plugin auto-selects the first tasklist and logs to stderr - users don't see clear guidance in the TUI when no tasks exist or when configuration is needed.

UX research shows empty states are critical conversion points - users decide to "stop using" apps within 3-7 days. Effective empty states combine: descriptive headline, supportive subtext, primary action CTA, and visual support. For CLI/TUI contexts, progressive disclosure (showing what's needed when needed) reduces cognitive load.

The totui plugin interface provides limited UX hooks - plugins communicate via FfiCommand (CreateTodo, UpdateTodo, SetTodoMetadata) and stderr logging. There's no notification system, status bar API, or modal dialog support. The UX strategy must work within these constraints: create "guidance todos" that explain next steps, use todo content for messaging, and leverage metadata for state tracking.

**Primary recommendation:** Use informational todos as the UX mechanism - create descriptive placeholder todos when no tasklist is being watched or no tasks exist. These "guidance todos" serve as in-context documentation that users see immediately in the TUI. Delete guidance todos automatically when actual sync begins.

## Standard Stack

### Core (Already in Cargo.toml)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| totui-plugin-interface | git (main) | Plugin trait, FfiCommand | Required interface - provides CreateTodo for guidance todos |
| abi_stable | 0.11 | FFI-safe types | Required for plugin interface |
| serde | 1.0 | Serialization | Already in use for JSON/TOML |

### No New Dependencies Required

UX polish is primarily about information architecture and message design - no new libraries needed. The existing command set (CreateTodo, UpdateTodo, DeleteTodo, SetTodoMetadata) provides all necessary capabilities.

### Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Guidance todos | stderr logging | Never for user-visible messaging; stderr invisible in TUI |
| In-todo messaging | Status bar | Only if totui adds plugin status bar API (not currently available) |
| Auto-delete guidance | Manual dismissal | Only if totui adds dismissible notification system |

## Architecture Patterns

### Recommended Module Additions

```
src/
├── lib.rs                # Add guidance state checks in on_config_loaded and on_event
├── guidance.rs           # NEW: Guidance todo creation and lifecycle management
├── discovery.rs          # Add richer discovery metadata (aliases, suggestions)
└── messages.rs           # NEW: User-facing message templates (optional, could inline)
```

### Pattern 1: Guidance Todos as In-Context Help

**What:** Create placeholder todos that explain how to use the plugin. Delete them when real sync begins.

**When to use:** When plugin loads but no tasklist is being watched, or when tasklist is empty.

**Example:**
```rust
// Source: UX research - empty states should be actionable and informative

/// Guidance todo IDs for lifecycle management
const GUIDANCE_NO_TASKLIST: &str = "claude-guidance-no-tasklist";
const GUIDANCE_NO_TASKS: &str = "claude-guidance-no-tasks";
const GUIDANCE_GETTING_STARTED: &str = "claude-guidance-getting-started";

pub fn create_no_tasklist_guidance() -> Vec<FfiCommand> {
    vec![
        // Header
        FfiCommand::CreateTodo {
            content: "CLAUDE TASKS - Setup Required".into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome("claude-guidance-header".into()),
            state: FfiTodoState::Question,  // [?] indicates attention needed
            priority: ROption::RNone,
            indent_level: 0,
        },
        // Explanation
        FfiCommand::CreateTodo {
            content: "No Claude tasklists found in ~/.claude/tasks/".into(),
            parent_id: ROption::RSome("claude-guidance-header".into()),
            temp_id: ROption::RSome(GUIDANCE_NO_TASKLIST.into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
        // Action guidance
        FfiCommand::CreateTodo {
            content: "Start a Claude Code session to create a tasklist".into(),
            parent_id: ROption::RSome("claude-guidance-header".into()),
            temp_id: ROption::RSome(GUIDANCE_GETTING_STARTED.into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
        // Mark as guidance via metadata
        FfiCommand::SetTodoMetadata {
            todo_id: "claude-guidance-header".into(),
            data: r#"{"source":"claude-tasks","type":"guidance","dismissible":true}"#.into(),
            merge: false,
        },
    ]
}

pub fn create_empty_tasklist_guidance(tasklist_display: &str) -> Vec<FfiCommand> {
    vec![
        // Header with tasklist name
        FfiCommand::CreateTodo {
            content: format!("CLAUDE TASKLIST: {} - Waiting for tasks", tasklist_display).into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome("claude-header-waiting".into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        },
        // Explanation
        FfiCommand::CreateTodo {
            content: "Claude hasn't created any tasks yet".into(),
            parent_id: ROption::RSome("claude-header-waiting".into()),
            temp_id: ROption::RSome(GUIDANCE_NO_TASKS.into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
        // Hint
        FfiCommand::CreateTodo {
            content: "Tasks will appear here as Claude works".into(),
            parent_id: ROption::RSome("claude-header-waiting".into()),
            temp_id: ROption::RSome("claude-guidance-hint".into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
    ]
}

/// Delete all guidance todos when real sync begins
pub fn clear_guidance() -> Vec<FfiCommand> {
    vec![
        FfiCommand::DeleteTodo { id: "claude-guidance-header".into() },
        FfiCommand::DeleteTodo { id: GUIDANCE_NO_TASKLIST.into() },
        FfiCommand::DeleteTodo { id: GUIDANCE_GETTING_STARTED.into() },
        FfiCommand::DeleteTodo { id: GUIDANCE_NO_TASKS.into() },
        FfiCommand::DeleteTodo { id: "claude-guidance-hint".into() },
        FfiCommand::DeleteTodo { id: "claude-header-waiting".into() },
    ]
}
```

### Pattern 2: Progressive Disclosure via Guidance States

**What:** Track guidance state in SyncState, transition between states as plugin context changes.

**When to use:** To manage guidance todo lifecycle cleanly.

**Example:**
```rust
// Source: UX patterns - progressive disclosure

/// Current guidance state for UX flow
#[derive(Debug, Clone, PartialEq)]
pub enum GuidanceState {
    /// No tasklists exist at all
    NoTasklists,
    /// Tasklist exists but empty
    EmptyTasklist { display_name: String },
    /// Active sync - no guidance needed
    ActiveSync,
    /// Error state with recovery guidance
    Error { message: String },
}

impl SyncState {
    pub fn guidance_state(&self) -> GuidanceState {
        if self.selected_tasklist.is_none() {
            GuidanceState::NoTasklists
        } else if self.known_tasks.is_empty() {
            let display = self.get_display_name();
            GuidanceState::EmptyTasklist { display_name: display }
        } else {
            GuidanceState::ActiveSync
        }
    }

    fn get_display_name(&self) -> String {
        self.selected_tasklist
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|uuid| {
                self.config.get_alias(uuid)
                    .unwrap_or(uuid)
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown".to_string())
    }
}
```

### Pattern 3: Error States with Recovery Actions

**What:** When errors occur, create guidance todos that explain what went wrong and how to fix it.

**When to use:** On watcher failures, config errors, or directory not found.

**Example:**
```rust
// Source: UX research - error states should be actionable

pub fn create_error_guidance(error: &PluginError) -> Vec<FfiCommand> {
    let (title, explanation, action) = match error {
        PluginError::WatchLimitReached(msg) => (
            "CLAUDE TASKS - Watcher Limit Reached",
            msg.as_str(),
            "Increase system watch limit (see plugin docs)",
        ),
        PluginError::DirectoryNotFound => (
            "CLAUDE TASKS - Directory Not Found",
            "The ~/.claude/tasks/ directory doesn't exist",
            "Start a Claude Code session to create it",
        ),
        PluginError::ConfigParseError(path) => (
            "CLAUDE TASKS - Config Error",
            &format!("Invalid configuration file: {}", path),
            "Check TOML syntax in config file",
        ),
        PluginError::WatcherFailed(msg) => (
            "CLAUDE TASKS - Watcher Failed",
            msg.as_str(),
            "Restart totui to retry",
        ),
    };

    vec![
        FfiCommand::CreateTodo {
            content: title.into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome("claude-error-header".into()),
            state: FfiTodoState::Exclamation,  // [!] indicates error
            priority: ROption::RNone,
            indent_level: 0,
        },
        FfiCommand::CreateTodo {
            content: explanation.into(),
            parent_id: ROption::RSome("claude-error-header".into()),
            temp_id: ROption::RSome("claude-error-detail".into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
        FfiCommand::CreateTodo {
            content: format!("Action: {}", action).into(),
            parent_id: ROption::RSome("claude-error-header".into()),
            temp_id: ROption::RSome("claude-error-action".into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        },
    ]
}
```

### Pattern 4: Discovery UX Enhancement

**What:** When multiple tasklists exist, show selection guidance with helpful metadata.

**When to use:** When auto-selecting tasklist, show what was selected and alternatives.

**Example:**
```rust
// Source: UX research - help users understand context

pub fn create_selection_summary(
    selected: &TasklistInfo,
    total_count: usize,
    aliases: &HashMap<String, String>,
) -> Vec<FfiCommand> {
    let display_name = aliases.get(&selected.id)
        .map(|a| format!("{} ({}...)", a, &selected.id[..8.min(selected.id.len())]))
        .unwrap_or_else(|| selected.id.clone());

    let mut commands = vec![
        // Main header
        FfiCommand::CreateTodo {
            content: format!("CLAUDE TASKLIST: {}", display_name).into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RSome(format!("claude-header-{}", selected.id).into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        },
    ];

    // If multiple tasklists, mention alternatives
    if total_count > 1 {
        commands.push(FfiCommand::CreateTodo {
            content: format!("({} other tasklist{} available - configure aliases to switch)",
                total_count - 1,
                if total_count > 2 { "s" } else { "" }
            ).into(),
            parent_id: ROption::RSome(format!("claude-header-{}", selected.id).into()),
            temp_id: ROption::RSome("claude-selection-info".into()),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 1,
        });
    }

    commands
}
```

### Anti-Patterns to Avoid

- **Relying on stderr for user messaging:** Users don't see stderr in the TUI; use todos for visibility.
- **Persistent guidance that blocks:** Guidance todos should auto-delete when no longer needed.
- **Generic error messages:** Always include specific cause AND actionable recovery steps.
- **Overwhelming first-run info:** Progressive disclosure - show minimal guidance initially.
- **Hard-coded strings scattered:** Centralize message templates for consistency and i18n readiness.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Notification system | Custom UI layer | Guidance todos | Plugin interface doesn't support notifications |
| Status bar messages | Direct terminal write | Guidance todos | Would bypass totui's rendering |
| Modal dialogs | Custom prompts | Informational todos | Plugin can't control TUI rendering |
| User preferences for guidance | Custom config | Metadata on guidance todos | Leverage existing metadata system |

**Key insight:** The plugin interface constrains UX to what FfiCommand supports. Creative use of todos AS documentation/guidance is the only viable pattern. This actually works well - guidance appears exactly where users are looking (their todo list).

## Common Pitfalls

### Pitfall 1: Guidance Todos Orphaned on Error

**What goes wrong:** Error during sync leaves guidance todos visible alongside real todos, confusing users.

**Why it happens:** Guidance deletion not called before creating real todos.

**How to avoid:** Always call `clear_guidance()` as first step of any sync that creates real todos.

**Warning signs:** Users see "[?] Setup Required" alongside actual synced tasks.

### Pitfall 2: Guidance Flicker on Fast Sync

**What goes wrong:** User sees guidance briefly flash then disappear as sync completes quickly.

**Why it happens:** Guidance created, then immediately deleted when sync starts.

**How to avoid:** Check if tasks will be synced BEFORE creating guidance. Only show guidance if truly in empty state.

**Warning signs:** Visible flicker on every project load.

### Pitfall 3: Stale Guidance After Tasklist Appears

**What goes wrong:** User starts Claude Code, creates tasklist, but guidance still says "no tasklists found".

**Why it happens:** Discovery only runs on plugin load, not on file events.

**How to avoid:** Re-run discovery on OnLoad events if currently showing "no tasklists" guidance.

**Warning signs:** User reports having to restart totui to see new tasklists.

### Pitfall 4: Delete Commands for Non-Existent Todos

**What goes wrong:** `clear_guidance()` called when guidance was never created; delete commands may error or warn.

**Why it happens:** Not tracking whether guidance was actually created.

**How to avoid:** Track `guidance_shown: bool` in SyncState; only delete what was created.

**Warning signs:** Error messages about deleting non-existent todos in logs.

### Pitfall 5: Guidance Metadata Pollutes Queries

**What goes wrong:** Queries for "claude-tasks" source return guidance todos along with real todos.

**Why it happens:** Using same metadata structure for guidance and real todos.

**How to avoid:** Use distinct `"type": "guidance"` field in metadata; filter in queries.

**Warning signs:** Sync logic tries to update guidance todos as if they were real tasks.

## Code Examples

### Message Templates (Centralized)

```rust
// Source: UX best practice - centralized, consistent messaging

pub mod messages {
    pub const HEADER_SETUP_REQUIRED: &str = "CLAUDE TASKS - Setup Required";
    pub const HEADER_WAITING: &str = "CLAUDE TASKLIST: {} - Waiting for tasks";
    pub const HEADER_ERROR: &str = "CLAUDE TASKS - {}";

    pub const MSG_NO_TASKLISTS: &str = "No Claude tasklists found in ~/.claude/tasks/";
    pub const MSG_START_CLAUDE: &str = "Start a Claude Code session to create a tasklist";
    pub const MSG_NO_TASKS_YET: &str = "Claude hasn't created any tasks yet";
    pub const MSG_TASKS_WILL_APPEAR: &str = "Tasks will appear here as Claude works";
    pub const MSG_OTHER_TASKLISTS: &str = "({} other tasklist{} available - configure aliases to switch)";

    pub const ACTION_PREFIX: &str = "Action: {}";
    pub const ACTION_INCREASE_LIMIT: &str = "Increase system watch limit (see plugin docs)";
    pub const ACTION_START_CLAUDE: &str = "Start a Claude Code session to create it";
    pub const ACTION_CHECK_CONFIG: &str = "Check TOML syntax in config file";
    pub const ACTION_RESTART: &str = "Restart totui to retry";
}
```

### Guidance State Integration

```rust
// In lib.rs on_config_loaded

fn on_config_loaded(&self, _config: RHashMap<RString, FfiConfigValue>) {
    let plugin_config = load_config();
    let tasklists = discovery::discover_tasklists();

    if tasklists.is_empty() {
        // No tasklists - show guidance
        let guidance_commands = guidance::create_no_tasklist_guidance();
        // Store commands to return on first on_event
        let mut state = self.state.lock().unwrap();
        state.pending_commands = guidance_commands;
        state.guidance_shown = true;
        eprintln!("claude-tasks: No tasklists found - showing setup guidance");
        return;
    }

    // Continue with normal initialization...
    let selected = &tasklists[0];
    // ...existing code...

    // If tasklist is empty, show waiting guidance
    let tasks = discovery::scan_tasks_directory(&selected.path);
    if tasks.is_empty() {
        let display = format_tasklist_display(&selected.id, &plugin_config);
        let guidance_commands = guidance::create_empty_tasklist_guidance(&display);
        let mut state = self.state.lock().unwrap();
        state.pending_commands = guidance_commands;
        state.guidance_shown = true;
    }
}
```

### Guidance Cleanup on First Real Sync

```rust
// In process_sync_events_local

fn process_sync_events_local(&self) -> Vec<FfiCommand> {
    let mut commands = Vec::new();

    // Check if we need to clear guidance
    let should_clear_guidance = {
        let state = self.state.lock().unwrap();
        state.guidance_shown
    };

    // ... process events ...

    if !commands.is_empty() && should_clear_guidance {
        // Prepend guidance cleanup before real commands
        let mut all_commands = guidance::clear_guidance();
        all_commands.extend(commands);

        // Update state
        let mut state = self.state.lock().unwrap();
        state.guidance_shown = false;

        return all_commands;
    }

    commands
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| stderr logging only | Guidance todos in TUI | 2024+ | Users see feedback directly |
| Generic errors | Actionable error guidance | Best practice | Higher recovery success |
| Single message | Progressive disclosure | UX research | Reduced cognitive load |
| Hard-coded strings | Centralized messages | Maintenance practice | Easier updates, i18n ready |

**Deprecated/outdated:**
- Plain stderr for user messaging: Invisible in TUI context; use todos instead.
- "Contact support" generic errors: Modern UX requires actionable self-service guidance.

## Open Questions

1. **Todo deletion cascading behavior**
   - What we know: DeleteTodo takes an ID
   - What's unclear: Does deleting parent automatically delete children? Or do we need explicit deletion?
   - Recommendation: Test with totui; if children orphan, delete children first then parent

2. **Guidance persistence across restarts**
   - What we know: Todos persist in daily files; guidance todos would too
   - What's unclear: Should guidance persist or be re-evaluated on each load?
   - Recommendation: Delete guidance on shutdown if possible; otherwise filter on load

3. **Multiple concurrent guidance states**
   - What we know: Could have error AND no-tasklist simultaneously
   - What's unclear: How to prioritize multiple guidance states
   - Recommendation: Error takes precedence; show most actionable state

4. **Tasklist selection UI beyond auto-select**
   - What we know: Currently auto-selects first tasklist
   - What's unclear: Whether Phase 3 should include selection UI
   - Recommendation: Defer selection UI to future phase; focus on guidance for current auto-select behavior

## Sources

### Primary (HIGH confidence)
- [Empty State UX Design Guide 2025](https://ui-deploy.com/blog/complete-guide-to-empty-state-ux-design-turn-nothing-into-something-2025) - Empty state patterns, types, elements
- [Progressive Disclosure - NN/G](https://www.nngroup.com/articles/progressive-disclosure/) - Progressive disclosure principles
- totui-plugin-interface source code - FfiCommand variants, event types, Plugin trait
- Existing claude-tasks source - Current architecture, state management, discovery

### Secondary (MEDIUM confidence)
- [Appcues UX Onboarding Patterns](https://www.appcues.com/blog/user-onboarding-ui-ux-patterns) - Onboarding UI patterns
- [Carbon Design System - Empty States](https://carbondesignsystem.com/patterns/empty-states-pattern/) - Enterprise empty state patterns
- [Toptal Empty State UX](https://www.toptal.com/designers/ux/empty-state-ux-design) - Empty state importance and design

### Tertiary (LOW confidence)
- General TUI/CLI UX discussions - Community patterns (no authoritative source)

## Metadata

**Confidence breakdown:**
- Guidance todos pattern: MEDIUM - Novel approach within plugin constraints; needs validation
- Message design: HIGH - Based on well-established UX research
- State management: HIGH - Extends existing patterns in codebase
- Error recovery: MEDIUM - Dependent on totui behavior for DeleteTodo

**Research date:** 2026-01-27
**Valid until:** 2026-02-27 (30 days - UX patterns stable, implementation details may need validation)
