# Testing Patterns

**Analysis Date:** 2026-01-27

## Test Framework

**Runner:**
- Cargo test (built-in Rust test runner)
- No explicit test framework dependency detected in Cargo.toml
- Run command: `just test` or `cargo test` (from `justfile` line 14-15)

**Assertion Library:**
- Built-in Rust assertions (assert!, assert_eq!, etc.)
- No external assertion crate detected

**Run Commands:**
```bash
just test                    # Run all tests
cargo test                   # Run all tests (direct)
cargo test --lib           # Run library tests
cargo test -- --nocapture  # Run with output
```

## Test File Organization

**Location:**
- No separate test files detected
- Pattern appears to be: tests would be inline in `src/lib.rs` or in `tests/` directory (not present)

**Naming:**
- No test files found; project has zero test coverage

**Structure:**
```
jira-claude/
├── src/
│   └── lib.rs          # No #[test] modules present
└── tests/              # Directory not present
```

## Test Coverage

**Current Status:** Not implemented

**Files without tests:**
- `src/lib.rs` - 468 lines, zero test coverage
  - `JiraClaudePlugin` struct and impl block (lines 43-346)
  - All public methods unexercised: `generate()`, `config_schema()`, `execute_with_host()`, etc.
  - Helper functions untested: `fetch_jira_ticket()`, `run_command()`, `parse_claude_output()`

**Reason:** This is a plugin library with FFI boundaries and external CLI dependencies (acli, claude), making unit tests complex without mocking infrastructure

## Test Areas That Should Be Added

**Unit Tests (Currently Missing):**

1. **Command Execution** - `run_command()` and `check_command_exists()`
   - Would need mocks for Command struct
   - Test successful execution with stdout capture
   - Test failure paths with exit codes and stderr
   - Test UTF-8 validation on output

2. **JSON Parsing** - `parse_claude_output()`
   - Extract valid JSON arrays from wrapped text
   - Handle missing brackets
   - Handle malformed JSON
   - Handle different whitespace scenarios

3. **Prompt Building** - `build_prompt()`
   - Correct formatting with ticket details
   - Comment section assembly with author/date extraction
   - Empty comment handling
   - ADF to text extraction

4. **ADF Extraction** - `extract_text_from_adf()`
   - String values pass through
   - Object `content` field extraction
   - Object `text` field extraction
   - Array flattening with newline joins
   - Null/invalid value handling

5. **Type Conversions** - `JiraTicket::from(AcliTicket)`
   - Field mapping correctness
   - Optional field handling (description, comments)
   - Comment filtering and author extraction

6. **Error Messages** - `truncate_string()`
   - Preserves short strings
   - Truncates long strings with ellipsis
   - Boundary conditions (max_len, strings at boundary)

**Integration Tests (Should Be Added):**
- Full flow: ticket ID → acli fetch → Claude prompt → todo generation
- Requires mocking acli and claude CLI commands
- Would need test fixtures for acli JSON responses

## Mocking Strategy

**Recommended Approach:**
- Use `mockito` or similar for Command mocking
- Create test fixtures in `tests/fixtures/` for sample acli outputs
- Mock `Command::new()` via trait abstraction (currently not present)

**Current Limitation:**
- `Command::new()` is called directly in code; not mockable without refactoring
- Would need to introduce `CommandRunner` trait for testability
- FFI layer (`abi_stable`) adds testing complexity

## Test Structure (If Implemented)

**Pattern to follow:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_command_success() {
        // Would mock Command::new() and Command::output()
        // Assert successful stdout parsing
    }

    #[test]
    fn test_parse_claude_output_with_json() {
        let plugin = JiraClaudePlugin::new();
        let output = r#"Some text [{"content": "Task 1", "indent_level": 0}] more text"#;
        let result = plugin.parse_claude_output(output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("short", 10), "short");
        assert_eq!(truncate_string("this is long", 5), "this ...");
    }
}
```

## Requirements

**Current:** No test coverage enforced

**Recommended:**
- Minimum 60% coverage for critical paths
- 100% coverage for error handling paths
- All public methods should have at least one test case

## View Coverage

Currently no coverage reporting configured.

**To add coverage reporting:**
```bash
cargo tarpaulin --out Html  # Would require tarpaulin dependency
```

## Special Notes

**Fragile Areas That Need Tests:**
1. JSON extraction and parsing (lines 184-209) - Easy to break with whitespace changes
2. ADF text extraction (lines 415-436) - Recursive, non-obvious behavior
3. Prompt building (lines 135-180) - Output format critical for Claude CLI
4. Command execution (lines 65-83) - Error handling with platform-specific paths

**Why Tests Are Hard:**
- External CLI dependencies (acli, claude) must be available
- FFI/ABI stable layer (`abi_stable` crate) requires special handling
- Plugin interface types are complex Rust FFI constructs

**Testing Philosophy for This Codebase:**
- Focus on pure functions first: `extract_text_from_adf()`, `truncate_string()`, `parse_claude_output()`
- Refactor Command usage into trait for easier mocking
- Use integration tests with containerized test environment for acli/claude

---

*Testing analysis: 2026-01-27*
