# Codebase Concerns

**Analysis Date:** 2026-01-27

## Tech Debt

**External command dependency without fallback:**
- Issue: Plugin relies on external CLI tools (`acli` and `claude`) that must be installed and accessible in PATH. If either is missing, the entire plugin fails at runtime with no graceful degradation.
- Files: `jira-claude/src/lib.rs` (lines 51-62, 86-98, 125-132)
- Impact: Plugin is unusable if dependencies aren't installed. Users get opaque error messages about missing commands rather than actionable guidance.
- Fix approach: Implement pre-flight validation with helpful setup instructions, add documentation for required CLI tools, consider providing a package-based installation guide or Docker container.

**Version mismatch in marketplace.toml:**
- Issue: Plugin version is 0.1.6 in Cargo.toml and marketplace.toml, but download URLs reference v0.1.5 release assets. This is out of sync.
- Files: `marketplace.toml` (lines 20, 25-29), `jira-claude/Cargo.toml` (line 3)
- Impact: Users installing from marketplace.toml get v0.1.5 binaries even though the metadata says v0.1.6.
- Fix approach: Release workflow must update all download URLs when CI rebuilds for new versions. Current sed-based approach in `.github/workflows/release.yml` (line 112) only updates version number, not URLs.

**Hard-coded version strings:**
- Issue: Plugin version is duplicated across `Cargo.toml`, `plugin.toml`, and marketplace.toml. Must be manually kept in sync.
- Files: `jira-claude/Cargo.toml` (line 3), `jira-claude/plugin.toml` (line 3), `marketplace.toml` (line 20)
- Impact: Easy to create version mismatches during releases. Release workflow partly automates this but requires manual attention in marketplace.toml.
- Fix approach: Create a single source of truth (e.g., read from Cargo.toml in workflow) or use automated sync in release workflow.

## Known Bugs

**Incomplete marketplace.toml update in release workflow:**
- Issue: Release workflow successfully builds and creates GitHub releases, but marketplace.toml download URLs don't get updated to the new release version.
- Files: `.github/workflows/release.yml` (lines 104-122)
- Trigger: Every release - marketplace.toml shows version 0.1.6 but downloads still point to v0.1.5
- Workaround: Manual update of download URLs in marketplace.toml after release
- Impact: Users installing from marketplace get mismatched binary versions, potential compatibility issues

**ADF parsing incomplete for complex Jira content:**
- Issue: `extract_text_from_adf()` function recursively searches for `text` and `content` fields but may not handle all Atlassian Document Format structures. If Jira returns deeply nested or complex ADF, important content could be silently dropped.
- Files: `jira-claude/src/lib.rs` (lines 415-437)
- Impact: Ticket descriptions and comments may be incomplete or empty when passed to Claude, resulting in poor todo generation.
- Risk: Occurs silently without user awareness

## Security Considerations

**Command injection risk in subprocess execution:**
- Risk: Plugin passes user-supplied ticket IDs directly to `acli` CLI via `Command::new()`. While the ID is only passed as a positional argument (safer than shell execution), a ticket ID containing shell metacharacters could cause issues.
- Files: `jira-claude/src/lib.rs` (lines 86-98, where `ticket_id` is passed to acli)
- Current mitigation: ID is uppercase-normalized (line 218) but not sanitized. Passed as argument (not through shell).
- Recommendations: Document that ticket IDs must be valid Jira keys (validated server-side by acli). Consider explicit validation of ticket ID format before passing to CLI.

**External CLI dependency security:**
- Risk: Plugin's security depends on trustworthiness of `acli` and `claude` CLIs installed on user's system. A compromised CLI could leak Jira data or execute arbitrary commands.
- Files: Plugin architecture in `jira-claude/src/lib.rs`
- Current mitigation: Plugin declares `subprocess = true` permission in `plugin.toml` (line 10), host is aware of this capability
- Recommendations: Document security implications in README. Consider future API-based integration instead of CLI-based if available.

**Sensitive data in error messages:**
- Risk: Error messages may include Jira ticket content or Claude output in full (see `truncate_string` usage). If logs are exposed or errors are displayed to untrusted parties, sensitive ticket data could leak.
- Files: `jira-claude/src/lib.rs` (lines 100-106, 189-209 include truncated output in errors)
- Current mitigation: Errors truncate to 200-2000 characters, but full content is still partially exposed
- Recommendations: Implement error logging strategy that separates user-facing messages from detailed logs. Consider not including content in error messages.

## Performance Bottlenecks

**Synchronous subprocess execution blocks UI:**
- Problem: `generate_from_ticket()` executes `acli` and `claude` CLIs synchronously, blocking the to-tui event loop during fetching and generation. For slow network or large Jira instances, this could freeze the UI.
- Files: `jira-claude/src/lib.rs` (lines 213-290, specifically subprocess calls in lines 87, 129)
- Cause: Rust `std::process::Command` is blocking. Plugin interface likely expects synchronous execution but this can create poor UX.
- Improvement path: Investigate if host plugin interface supports async operations. If so, use tokio or async subprocess execution. Document expected latency in README.

**No caching of Jira data:**
- Problem: Every generate request re-fetches the full ticket from Jira via `acli`. If user generates multiple times for same ticket (e.g., to tweak Claude prompts), multiple redundant API calls occur.
- Files: `jira-claude/src/lib.rs` (lines 213-290)
- Impact: Unnecessary network traffic and latency
- Improvement path: Add optional caching layer (e.g., in-memory cache with TTL) for recently fetched tickets

## Fragile Areas

**Claude output parsing is brittle:**
- Files: `jira-claude/src/lib.rs` (lines 184-210)
- Why fragile: Parser searches for `[` and `]` to extract JSON from Claude's output. If Claude wraps JSON in markdown code blocks (common with LLMs), or includes extra text, parsing fails. Also assumes exact JSON format from Claude.
- Safe modification: Add more robust JSON extraction that handles markdown code blocks (```json...```). Improve error messages to include full Claude output (sanitized) for debugging.
- Test coverage: No tests for this critical parsing function. Needs unit tests with various Claude output formats.

**Jira API dependency on acli format changes:**
- Files: `jira-claude/src/lib.rs` (lines 86-123)
- Why fragile: Plugin hardcodes specific `acli` command flags and JSON response structure. If `acli` changes output format or fields, parsing breaks silently.
- Safe modification: Add schema validation for acli JSON response. Log warnings if expected fields are missing.
- Test coverage: No integration tests against real or mock Jira instances

**Version mismatch between plugin code and interface:**
- Files: `jira-claude/src/lib.rs` (line 305 hardcodes "0.1.0"), `jira-claude/Cargo.toml` (line 3 has "0.1.6")
- Why fragile: Plugin's `version()` method returns hardcoded "0.1.0" while package version is 0.1.6. If host checks version, this could cause issues.
- Safe modification: Read version from environment variable set during build, or use `env!()` macro to embed Cargo version at compile time.

## Scaling Limits

**No batch ticket processing:**
- Current capacity: Plugin processes one ticket at a time
- Limit: Users can't generate todos for multiple tickets in one operation
- Scaling path: Add support for comma-separated ticket IDs or a "project" mode to process multiple tickets

**No rate limiting:**
- Risk: Rapid sequential `generate()` calls could hit Jira and Claude API limits without warning
- Recommendation: Implement rate limiting or request queuing with user feedback

## Dependencies at Risk

**`totui-plugin-interface` on git branch:**
- Risk: Plugin depends on `{ git = "...", branch = "main" }` reference (line 12 in Cargo.toml). If host project changes main branch unexpectedly, plugin could break.
- Impact: Build failures, compatibility issues
- Migration plan: Once to-tui stabilizes API, use released versions from crates.io instead of git branch

**`abi_stable` version pinned:**
- Risk: abi_stable 0.11.3 is used for FFI stability. Version constraints are strict.
- Impact: Hard to update if security fixes needed in transitive dependencies
- Recommendation: Monitor security advisories for abi_stable and dependencies

## Missing Critical Features

**No retry logic:**
- Problem: If `acli` or `claude` commands timeout or fail transiently, plugin fails immediately with no retry
- Blocks: Unreliable operation in network-constrained environments
- Fix: Implement exponential backoff retry with configurable attempts

**No progress feedback:**
- Problem: Plugin doesn't report progress during long Jira fetches or Claude API calls. User sees no indication that something is happening.
- Blocks: Users unsure if plugin is working or hung
- Fix: Return intermediate status or hook into host's progress reporting if available

**No configuration:**
- Problem: All behavior is hardcoded. Can't customize Claude prompt, acli flags, or connection details.
- Blocks: Can't adapt to different Jira instances or customize todo structure
- Fix: Implement `config_schema()` to expose configurable options

## Test Coverage Gaps

**No unit tests:**
- What's not tested: JSON parsing (Claude output, acli response), ADF extraction, prompt building, error handling
- Files: `jira-claude/src/lib.rs` - entire codebase
- Risk: Regressions in parsing logic, silent failures in edge cases (empty responses, malformed Jira data)
- Priority: High

**No integration tests:**
- What's not tested: End-to-end flow with real or mocked Jira + Claude CLIs
- Risk: Plugin could build successfully but fail at runtime with actual Jira instances
- Priority: High

**No error case testing:**
- What's not tested: Missing commands, invalid ticket IDs, malformed JSON responses, CLI failures
- Risk: Error paths have never been validated
- Priority: Medium

---

*Concerns audit: 2026-01-27*
