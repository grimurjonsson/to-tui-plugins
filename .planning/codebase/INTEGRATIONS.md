# External Integrations

**Analysis Date:** 2026-01-27

## APIs & External Services

**Jira:**
- Atlassian Jira Cloud/Server via acli
  - CLI Tool: `acli` (Atlassian CLI)
  - Purpose: Fetch Jira ticket data (key, summary, description, comments)
  - Integration: Subprocess execution via `Command::new("acli")` in `jira-claude/src/lib.rs` (lines 87-98)
  - Authentication: Uses existing acli configuration (acli handles auth internally)

**Claude AI:**
- Anthropic Claude via claude CLI
  - CLI Tool: `claude` (Claude command-line interface)
  - Purpose: Generate actionable todo items from Jira ticket data via AI
  - Integration: Subprocess execution via `Command::new("claude")` in `jira-claude/src/lib.rs` (line 129)
  - Authentication: Uses existing claude CLI authentication setup
  - Prompt format: Sends Jira ticket details (summary, description, comments) and expects JSON array of todo objects

## Data Storage

**Databases:**
- Not applicable - Plugin is stateless, generates todos in-memory and passes to host

**File Storage:**
- Local filesystem only - No persistent storage
- Plugin uses system temp/memory for command execution

**Caching:**
- None - Each plugin invocation makes fresh acli and claude requests

## Authentication & Identity

**Auth Providers:**
- Custom CLI-based authentication through existing tools
  - Jira: Handled by acli CLI (pre-configured on user's system)
  - Claude: Handled by claude CLI (pre-configured on user's system)

**Implementation:**
- No explicit authentication in plugin code
- Plugin validation checks that both CLIs exist in PATH before execution (lines 215-216 in `jira-claude/src/lib.rs`)
- Both CLIs are invoked with subprocess permissions declared in `plugin.toml`

## Monitoring & Observability

**Error Tracking:**
- Not detected - Errors propagated as RResult strings through FFI interface

**Logs:**
- Command stdout/stderr captured and returned to host on error
- Error messages include:
  - Missing command messages (lines 60 in `jira-claude/src/lib.rs`)
  - JSON parsing errors with truncated output for debugging (lines 100-105)
  - Failed command exit codes with stderr content (lines 73-78)
  - Claude output parsing errors (lines 187-210)

## CI/CD & Deployment

**Hosting:**
- GitHub repository: https://github.com/grimurjonsson/to-tui-plugins
- Release distribution: GitHub Releases with binary artifacts

**CI Pipeline:**
- GitHub Actions (`jira-claude/../../.github/workflows/release.yml`)
- Triggered by: Git tags matching pattern `jira-claude-v*`
- Multi-platform builds:
  - x86_64-unknown-linux-gnu (native)
  - aarch64-unknown-linux-gnu (cross-compiled via cross)
  - x86_64-apple-darwin (macOS Intel)
  - aarch64-apple-darwin (macOS Apple Silicon)
  - x86_64-pc-windows-msvc (Windows MSVC)
- Artifacts: Compiled binaries packaged with plugin.toml and README.md
- Release creation: Automated GitHub release generation with compiled artifacts
- Marketplace update: `marketplace.toml` auto-updated with download URLs after successful release

**Version Management:**
- Semantic versioning used (e.g., v0.1.6 in current marketplace.toml)
- Plugin version declared in:
  - `jira-claude/Cargo.toml` (package version field)
  - `jira-claude/plugin.toml` (plugin version field)
  - `marketplace.toml` (plugins.version field)

## Environment Configuration

**Required env vars:**
- None explicitly - Plugin relies on:
  - `acli` being configured in user's environment with Jira credentials
  - `claude` being configured in user's environment with API credentials

**Secrets location:**
- Not applicable - All authentication delegated to external CLI tools
- External tools (acli, claude) manage their own credential storage

## Webhooks & Callbacks

**Incoming:**
- Not applicable - Plugin is stateless generator, no incoming webhooks

**Outgoing:**
- Not applicable - Plugin only reads from Jira and Claude, does not write back

**Host Integration:**
- Plugin implements Plugin trait from `totui-plugin-interface`:
  - `generate()` method: Takes ticket ID as input, returns vector of FfiTodoItem
  - `on_event()` method: Stub implementation (always returns success with no-op response)
  - `execute_with_host()` method: Not used, returns empty command vector
  - `config_schema()` method: Returns empty schema (no plugin configuration)
  - `on_config_loaded()` method: Stub implementation

## Data Flow

**Workflow:**

1. Host calls `Plugin::generate(ticket_id)` with Jira ticket ID (e.g., "PROJ-123")
2. Plugin validates `acli` and `claude` commands exist in PATH
3. Plugin executes: `acli jira workitem view PROJ-123 --fields key,summary,description,comment --json`
4. acli returns JSON, plugin parses via `AcliTicket` struct deserialization
5. Plugin extracts text from Atlassian Document Format (ADF) nested structures
6. Plugin builds Claude prompt with ticket summary, description, and comments (lines 135-181)
7. Plugin executes: `claude -p "{prompt}"`
8. Claude returns JSON array of todo objects with format: `[{"content": "Task", "indent_level": 0}, ...]`
9. Plugin parses JSON response, creates nested FfiTodoItem hierarchy
10. Root todo item contains ticket key/summary and link to Jira ticket
11. Child todos are generated items with indent level preserved as FfiTodoItem.indent_level
12. All todos assigned UUID identifiers and current timestamp
13. Host receives vector of FfiTodoItem for display/persistence

---

*Integration audit: 2026-01-27*
