# to-tui-plugins

Official plugin registry for [to-tui](https://github.com/grimurjonsson/to-tui).

## Available Plugins

| Plugin | Description | Version |
|--------|-------------|---------|
| [claude-tasks](./claude-tasks) | Real-time sync of Claude Code tasks to totui | 1.0.2 |
| [jira-claude](./jira-claude) | Generate todos from Jira tickets using Claude AI | 0.1.0 |

## Installing Plugins

```bash
# Install from registry
totui plugin install jira-claude

# List installed plugins
totui plugin list

# Update a plugin
totui plugin update jira-claude
```

## For Plugin Authors

See [to-tui documentation](https://github.com/grimurjonsson/to-tui/blob/main/docs/plugins.md) for plugin development guide.

### Plugin Structure

Each plugin is a separate directory containing:

- `Cargo.toml` - Rust crate configuration with `cdylib` output
- `src/lib.rs` - Plugin implementation (implements `Plugin` trait)
- `plugin.toml` - Plugin manifest with metadata and permissions
- `README.md` - Plugin documentation
- `CHANGELOG.md` - Version history

### Building a Plugin

```bash
cd plugin-name
cargo build --release
```

The compiled plugin will be in `target/release/libplugin_name.{so,dylib,dll}`.

### Testing Locally

1. Build the plugin
2. Copy the library and `plugin.toml` to `~/.config/to-tui/plugins/plugin-name/`
3. Restart to-tui

## License

MIT
