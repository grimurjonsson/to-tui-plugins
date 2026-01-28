# claude-tasks

Real-time sync of Claude Code tasks to totui.

## Overview

This plugin watches Claude Code's native task lists and syncs them to totui in real-time. When Claude Code creates, updates, or completes tasks, they automatically appear in your totui interface.

## Installation

```bash
totui plugin install claude-tasks
```

## Usage

1. Press `P` in totui to open the plugins modal
2. Select claude-tasks from the Installed tab
3. Choose which Claude tasklist to sync
4. Tasks will appear and update automatically

## How it works

1. Discovers Claude Code tasklist folders on your system
2. Watches the selected tasklist for file changes
3. Parses task JSON files and converts to totui todos
4. Syncs creates, updates, and deletions in real-time
5. Shows staleness indicator when tasks haven't updated recently

## Features

- **Real-time sync**: Tasks appear instantly as Claude Code creates them
- **Bidirectional state**: Task completion status stays in sync
- **Multiple tasklists**: Choose which Claude session to follow
- **Staleness tracking**: Visual indicator when a tasklist goes quiet
- **Aliasing**: Configure friendly names for tasklist UUIDs

## Configuration

The plugin auto-discovers Claude Code tasklists. You can configure aliases for tasklist UUIDs in `~/.config/totui/claude-tasks.toml`:

```toml
[aliases]
"abc123-def456" = "My Project"

[settings]
staleness_threshold = 300  # seconds before showing stale indicator
```

## License

MIT
