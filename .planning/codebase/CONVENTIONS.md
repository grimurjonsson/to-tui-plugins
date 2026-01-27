# Coding Conventions

**Analysis Date:** 2026-01-27

## Naming Patterns

**Files:**
- Single file crate: `src/lib.rs` - The entire plugin is a single module
- Configuration files: `plugin.toml` for plugin metadata
- Lowercase with hyphens for package names in Cargo.toml: `jira-claude`

**Functions:**
- Snake_case for function names: `fetch_jira_ticket()`, `run_command()`, `check_command_exists()`
- Private helper functions use leading underscore implicitly via `fn` (not `pub`)
- Methods on impl blocks follow snake_case: `generate_from_ticket()`, `build_prompt()`

**Variables:**
- Snake_case for all variables and bindings: `ticket_id`, `generated_todos`, `base_url`
- Temporary variables for intermediate processing: `output`, `json_str`, `texts`
- Option/Result binding pattern: `map_err()`, `map()`, `and_then()` for chaining

**Types:**
- PascalCase for struct names: `JiraClaudePlugin`, `JiraTicket`, `AcliTicket`, `JiraComment`
- PascalCase for enum variants (standard Rust): `FfiTodoState::Empty`
- Type aliases for FFI types with prefix/suffix: `PluginModule_Ref`, `Plugin_TO`

## Code Style

**Formatting:**
- Standard Rust formatting (4-space indentation implied by edition 2021)
- Lines are generally under 100 characters, with some exceptions for error messages
- Blank lines separate logical sections within functions (see `lib.rs` line groups)

**Linting:**
- No explicit `.clippy.toml` or `rustfmt.toml` detected
- Default Rust 2021 edition conventions apply
- `#![allow(non_local_definitions)]` at file top (line 5) suppresses macro-related warnings

## Import Organization

**Order:**
1. `use abi_stable::*` - FFI/ABI stable crate imports (lines 7-11)
2. `use serde::*` - Serialization framework (line 13)
3. `use std::*` - Standard library (line 14)
4. `use totui_plugin_interface::*` - Plugin interface imports (lines 16-19)
5. `use uuid::*` - External crate utilities (line 20)

**Path Aliases:**
- No path aliases detected; absolute imports used throughout
- Nested imports via `use module::{Item1, Item2}` pattern for related types
- FFI types heavily qualified: `RString`, `RVec`, `RHashMap`, `ROption`, `RResult`

## Error Handling

**Patterns:**
- Result<T, String> used throughout for fallible operations
- Explicit `map_err()` conversions with context-rich messages
- Error messages include operation context: `"Failed to execute '{}': {}"` (line 69)
- Graceful degradation with `.ok()` for non-critical operations (line 238)
- Pattern matching on Results with descriptive error messages

**Examples from code:**
- Line 51-62: `check_command_exists()` returns `Result<(), String>` with command validation
- Line 65-83: `run_command()` chains `map_err()` for detailed error context
- Line 100-106: JSON parsing with detailed error output including raw content truncation

## Logging

**Framework:** No explicit logging framework detected; uses standard output implicitly

**Patterns:**
- Error messages are human-readable and descriptive
- No log levels (debug, info, warn, error) in code
- Context included in error messages: command name, exit code, stderr output
- Truncation of large outputs for readability: `truncate_string()` function (line 352-358)

## Comments

**When to Comment:**
- Module-level documentation: `//!` at file top (lines 1-3)
- Section dividers for major code blocks: `// ============================================================================` (lines 22, 35, 348, 349)
- Implementation notes above complex logic: `fn check_command_exists()` explained at line 50
- Comments on non-obvious design decisions: Line 329 explains why `execute_with_host()` is unused

**JSDoc/TSDoc:**
- Rust doc comments with `///` for public items (not extensive in this codebase)
- Example: Line 39-41 documents the plugin purpose
- Inline comments for Jira field extraction logic (lines 85, 111, 125, 134, 183, 212)

## Function Design

**Size:**
- Generally 10-30 lines per function
- Longer functions (40+ lines) are broken into helper methods
- `generate_from_ticket()` (lines 213-290) is largest, justified by orchestration logic

**Parameters:**
- `&self` for instance methods on impl blocks
- String references `&str` for immutable string data
- Owned `String` when building output or error messages
- FFI types use stable ABI types: `RString`, `RVec` from `abi_stable`

**Return Values:**
- `Result<T, String>` for fallible operations
- Wrapped FFI types: `RResult<RVec<...>, RString>` for plugin interface compliance
- Option<T> for nullable values: `Some()` / `None` patterns (line 387-404)

## Module Design

**Exports:**
- Single public struct `JiraClaudePlugin` (line 43)
- Impl block exports trait methods via `Plugin` trait
- Helper types are public but not re-exported in separate module
- Private helper functions via implicit privacy (no `pub` keyword)

**Barrel Files:**
- Not applicable; single-file library crate

## Struct Organization

**Instance vs Static:**
- `JiraClaudePlugin` is largely stateless; methods are effectively static
- `impl JiraClaudePlugin` contains all logic (lines 45-290)
- No fields on JiraClaudePlugin struct; derives only Debug
- Static methods: `check_command_exists()`, `run_command()` (no &self)

---

*Convention analysis: 2026-01-27*
