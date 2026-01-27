# Technology Stack

**Analysis Date:** 2026-01-27

## Languages

**Primary:**
- Rust 1.92.0 - Core plugin implementation, compiled to dynamic libraries (cdylib)

## Runtime

**Environment:**
- Rust stable toolchain (1.92.0)
- Cargo 1.92.0 package manager
- Target platforms: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64 MSVC)

**Package Manager:**
- Cargo - Rust's package manager
- Lockfile: `jira-claude/Cargo.lock` (present)

## Frameworks

**Core:**
- abi_stable 0.11 - FFI (Foreign Function Interface) abstraction for safe dynamic library loading and cross-library communication in Rust

**Build/Dev:**
- cargo build - Standard Rust build system
- cross - Cross-compilation tool for Linux targets (aarch64-unknown-linux-gnu)

## Key Dependencies

**Critical:**
- abi_stable 0.11 - Provides safe FFI layer for plugin architecture through trait objects and type-stable ABI
- totui-plugin-interface - Custom interface trait from https://github.com/grimurjonsson/to-tui (main branch) - Defines the Plugin trait and FFI structures for todo generation
- serde 1.0 - Serialization/deserialization framework (required for JSON parsing)
- serde_json 1.0 - JSON parsing for acli ticket data and Claude JSON responses

**Utilities:**
- uuid 1.0 - Generates unique identifiers for todo items (v4 feature for random UUIDs)

## Configuration

**Build Configuration:**
- `jira-claude/Cargo.toml` - Specifies crate-type = ["cdylib"] for dynamic library output
- Edition: Rust 2021

**Plugin Configuration:**
- `jira-claude/plugin.toml` - Plugin manifest with metadata, permissions, and external command dependencies
  - Declares subprocess permission required to run `acli` and `claude` commands
  - Declares dependencies on external commands: `acli` (Jira access) and `claude` (AI generation)

**Platform-Specific:**
- Targets: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin, x86_64-pc-windows-msvc

## External Command Dependencies

The plugin requires external CLI tools to function:
- `acli` (Atlassian CLI) - Executes commands: `acli jira workitem view {ticket_id} --fields key,summary,description,comment --json`
- `claude` (Claude CLI) - Executes: `claude -p "{prompt}"` for todo generation

Command validation happens at runtime via platform-specific checks:
- Windows: Uses `where` command to check if command exists
- Unix/Linux/macOS: Uses `which` command to check if command exists

## Artifact Output

**Library Output:**
- Linux: `libjira_claude.so` (placed in `jira-claude/target/{target}/release/`)
- macOS: `libjira_claude.dylib` (placed in `jira-claude/target/{target}/release/`)
- Windows: `jira_claude.dll` (placed in `jira-claude/target/{target}/release/`)

**Release Packaging:**
- Unix platforms: tar.gz archives with library, plugin.toml, and README.md
- Windows: ZIP archives with library, plugin.toml, and README.md

## Platform Requirements

**Development:**
- Rust stable toolchain (1.92.0 or later)
- Cargo package manager
- For aarch64-unknown-linux-gnu target: `cross` tool for cross-compilation

**Production:**
- Deployment target: to-tui plugin host (requires plugin interface version >= 0.1.0)
- Runtime: Host system must have `acli` and `claude` CLI tools installed and in PATH
- Jira access via acli authentication (uses existing acli configuration)
- Claude CLI authentication (uses existing claude CLI setup)

---

*Stack analysis: 2026-01-27*
