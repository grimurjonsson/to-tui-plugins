# jira-claude

Generate todos from Jira tickets using Claude AI.

## Requirements

- [acli](https://github.com/atlassian/acli) - Atlassian CLI for Jira access
- [claude](https://claude.ai) - Claude CLI for AI generation

## Installation

```bash
totui plugin install jira-claude
```

## Usage

1. Press `P` in to-tui to open the plugins modal
2. Select jira-claude from the Installed tab
3. Enter a Jira ticket ID (e.g., PROJ-123)
4. Review and accept the generated todos

## How it works

1. Fetches ticket details via `acli jira workitem view`
2. Extracts summary, description, and comments
3. Sends to Claude CLI with a task breakdown prompt
4. Parses JSON response into nested todo items
5. Creates parent item with ticket link and summary

## Example Output

For a ticket `PROJ-123: Implement user authentication`, you might get:

```
[ ] PROJ-123 : Implement user authentication
  [ ] Set up authentication middleware
  [ ] Create login endpoint
  [ ] Create logout endpoint
  [ ] Add JWT token validation
  [ ] Write authentication tests
```

## Configuration

This plugin requires no configuration. It uses your existing `acli` and `claude` CLI authentication.

## License

MIT
