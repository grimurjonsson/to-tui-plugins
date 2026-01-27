# Architecture

**Analysis Date:** 2026-01-27

## Pattern Overview

**Overall:** Plugin architecture based on dynamic library FFI with interface-driven design.

**Key Characteristics:**
- Implements `Plugin` trait from `totui-plugin-interface` via FFI boundary
- Uses `abi_stable` for ABI-stable FFI exports and stable type representations
- Single-responsibility plugin: generates todo items from Jira tickets using AI
- Subprocess-based data fetching (external CLI tools)
- Stateless plugin instance (all state immutable)

## Layers

**FFI Boundary Layer:**
- Purpose: Expose plugin to host via ABI-stable interface
- Location: `src/lib.rs` lines 1-33 (module export and plugin factory)
- Contains: `export_root_module`, `create_plugin`, FFI type wrappers
- Depends on: `abi_stable`, `totui_plugin_interface`
- Used by: to-tui host application

**Plugin Implementation Layer:**
- Purpose: Core plugin logic and trait implementation
- Location: `src/lib.rs` lines 39-346 (`JiraClaudePlugin` struct and impl blocks)
- Contains: Plugin trait methods, generation orchestration
- Depends on: Data fetching layer, prompt building, JSON parsing
- Used by: FFI boundary layer

**Data Fetching Layer:**
- Purpose: Query external systems and validate tool availability
- Location: `src/lib.rs` lines 50-123 (command execution, Jira ticket fetching, URL discovery)
- Contains: `check_command_exists`, `run_command`, `fetch_jira_ticket`, `fetch_jira_base_url`
- Depends on: Shell subprocess execution via `std::process::Command`
- Used by: Plugin implementation layer

**Prompt Engineering Layer:**
- Purpose: Construct well-formatted Claude prompts for consistent output
- Location: `src/lib.rs` lines 125-181 (prompt building and Claude invocation)
- Contains: `build_prompt`, `generate_todos_with_claude`
- Depends on: Jira data model, Data fetching layer
- Used by: Plugin implementation layer

**Parsing & Transformation Layer:**
- Purpose: Convert external data formats to internal representations and FFI types
- Location: `src/lib.rs` lines 183-437 (JSON parsing, text extraction, model transformations)
- Contains: `parse_claude_output`, `extract_text_from_adf`, From implementations, deserialization
- Depends on: `serde`, `serde_json` for deserialization
- Used by: Plugin implementation layer

**Model Layer:**
- Purpose: Type-safe representation of Jira and generated data
- Location: `src/lib.rs` lines 360-468 (data structures)
- Contains: `AcliTicket`, `JiraFields`, `JiraTicket`, `JiraComment`, `GeneratedTodo`, `AcliComment`, `CommentAuthor`
- Depends on: `serde` for serialization
- Used by: All other layers for data representation

## Data Flow

**Todo Generation Request:**

1. Host calls `Plugin::generate(ticket_id)` via FFI
2. `JiraClaudePlugin::generate_from_ticket` validates tool availability (acli, claude)
3. Fetch phase: Call `acli jira workitem view` with `--json` flag → receive `AcliTicket`
4. Transform phase: Parse JSON via `serde` → convert `AcliTicket` → `JiraTicket`
5. Enrichment phase: Extract base URL via `acli jira auth status` → build ticket URLs
6. Prompt phase: Build Claude prompt from ticket data → invoke `claude -p "..."`
7. Parse phase: Extract JSON array from Claude output → deserialize into `GeneratedTodo` objects
8. Hierarchy phase: Create parent `FfiTodoItem` from ticket summary/URL, create child items with parent_id references
9. Return: `RVec<FfiTodoItem>` to host

**State Management:**
- No persistent state. Plugin is stateless instance.
- All data flows through method parameters and local variables.
- Results returned as owned FFI types via `RVec`, `RString`, `ROption`.

## Key Abstractions

**JiraTicket:**
- Purpose: Normalized representation of Jira data regardless of acli output format
- Examples: `src/lib.rs` lines 373-413
- Pattern: From<AcliTicket> implements transformation from acli JSON schema to domain model

**GeneratedTodo:**
- Purpose: Intermediate representation of AI-generated todo with hierarchy level
- Examples: `src/lib.rs` lines 440-443
- Pattern: Deserialized directly from Claude JSON via serde

**FfiTodoItem:**
- Purpose: FFI-safe todo item structure for host consumption
- Examples: `src/lib.rs` lines 254-267, 272-285
- Pattern: Constructed with explicit parent_id references for nested structure

**ADF Text Extraction:**
- Purpose: Parse Jira's Atlassian Document Format for text content
- Examples: `src/lib.rs` lines 415-437
- Pattern: Recursive descent through nested JSON structures

## Entry Points

**Plugin Factory:**
- Location: `src/lib.rs` line 31-33
- Triggers: Host load of libdylib via `export_root_module`
- Responsibilities: Return Plugin instance wrapped in `Plugin_TO` opaque trait object

**Plugin::generate():**
- Location: `src/lib.rs` lines 312-316
- Triggers: User requests todo generation from plugin modal in to-tui
- Responsibilities: Orchestrate full request flow (validation → fetch → transform → generate → return)

**Plugin trait method stubs:**
- Location: `src/lib.rs` lines 319-345
- Triggers: Host may call config/event handlers (currently unused)
- Responsibilities: Return empty/default responses

## Error Handling

**Strategy:** Result-based error propagation with descriptive error messages.

**Patterns:**
- All fallible operations return `Result<T, String>` with context-rich error messages
- Error messages include command name, exit codes, stderr output, and truncated problematic input (max 2000 chars)
- Validation errors at boundaries (command existence check before execution)
- FFI boundary converts Rust `Result` to `RResult` with errors as `RString`
- Partial success accepted: if base URL fetch fails, continues without URL in description (lines 235-238)

**Example:** Lines 65-83 show command execution with detailed error context including command, exit code, and stderr.

## Cross-Cutting Concerns

**Logging:** No explicit logging. Errors returned as string messages to caller (host/user).

**Validation:**
- Command existence validation before execution (lines 51-62, 215-216)
- JSON format validation with helpful error messages (lines 187-199)
- UTF-8 validation on subprocess output (lines 81-82)

**Subprocess Management:**
- All external invocations via `std::process::Command` (stateless)
- Output collected via `.output()` (blocks until completion)
- Standard error captured for diagnostics

**ADF Handling:** Recursive pattern-matching for Jira's Atlassian Document Format to extract plain text across multiple content types (String, nested Object, Array).

---

*Architecture analysis: 2026-01-27*
