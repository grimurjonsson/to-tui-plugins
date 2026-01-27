//! Plugin configuration loading and alias resolution.
//!
//! Configuration is loaded from two locations:
//! - Global: ~/.config/totui/claude-tasks.toml
//! - Local: .totui/aliases.toml (overrides global)

use crate::discovery::{discover_tasklists, TasklistInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Plugin configuration.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct PluginConfig {
    /// UUID -> friendly name mappings
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Staleness threshold in minutes (default: 15)
    #[serde(default)]
    pub staleness_threshold_minutes: Option<u64>,
}

impl PluginConfig {
    /// Get staleness threshold, defaulting to 15 minutes
    pub fn staleness_threshold(&self) -> u64 {
        self.staleness_threshold_minutes.unwrap_or(15)
    }

    /// Get alias for a tasklist UUID, if configured
    pub fn get_alias(&self, uuid: &str) -> Option<&str> {
        self.aliases.get(uuid).map(|s| s.as_str())
    }
}

/// Load configuration from global and local paths.
///
/// Global config: ~/.config/totui/claude-tasks.toml
/// Local config: .totui/aliases.toml (overrides global)
pub fn load_config() -> PluginConfig {
    let mut config = PluginConfig::default();

    // Load global config
    if let Some(global_path) = global_config_path() {
        if global_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&global_path) {
                if let Ok(global) = toml::from_str::<PluginConfig>(&content) {
                    config = global;
                }
            }
        }
    }

    // Merge local config (overrides global)
    let local_path = local_config_path();
    if local_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&local_path) {
            if let Ok(local) = toml::from_str::<PluginConfig>(&content) {
                // Merge aliases - local overrides global
                config.aliases.extend(local.aliases);
                // Override staleness if specified
                if local.staleness_threshold_minutes.is_some() {
                    config.staleness_threshold_minutes = local.staleness_threshold_minutes;
                }
            }
        }
    }

    config
}

/// Get global config path: ~/.config/totui/claude-tasks.toml
fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("totui").join("claude-tasks.toml"))
}

/// Get local config path: .totui/aliases.toml
fn local_config_path() -> PathBuf {
    PathBuf::from(".totui").join("aliases.toml")
}

/// Format tasklist display with alias if available.
///
/// Returns "Alias (a1b2c3...)" if alias exists, otherwise just UUID.
pub fn format_tasklist_display(uuid: &str, config: &PluginConfig) -> String {
    match config.get_alias(uuid) {
        Some(alias) => {
            let short_uuid = &uuid[..8.min(uuid.len())];
            format!("{} ({}...)", alias, short_uuid)
        }
        None => uuid.to_string(),
    }
}

/// Generate Select options for tasklist picker.
///
/// Format: "Alias (uuid...) - N tasks, updated X ago" or "uuid - N tasks, updated X ago"
/// Returns (display_string, uuid) pairs.
pub fn generate_tasklist_options(config: &PluginConfig) -> Vec<(String, String)> {
    let tasklists = discover_tasklists();

    tasklists
        .into_iter()
        .map(|t| {
            let display = format_tasklist_option(&t, config);
            (display, t.id)
        })
        .collect()
}

/// Format a single tasklist for display in Select options.
fn format_tasklist_option(tasklist: &TasklistInfo, config: &PluginConfig) -> String {
    let name_part = match config.get_alias(&tasklist.id) {
        Some(alias) => {
            let short_uuid = &tasklist.id[..8.min(tasklist.id.len())];
            format!("{} ({}...)", alias, short_uuid)
        }
        None => tasklist.id.clone(),
    };

    let age = format_age(tasklist.last_modified);
    format!("{} - {} tasks, {}", name_part, tasklist.task_count, age)
}

/// Format time since last modified.
fn format_age(time: SystemTime) -> String {
    let duration = time.elapsed().unwrap_or_default();
    let secs = duration.as_secs();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert!(config.aliases.is_empty());
        assert!(config.staleness_threshold_minutes.is_none());
        assert_eq!(config.staleness_threshold(), 15); // default
    }

    #[test]
    fn test_staleness_threshold_custom() {
        let config = PluginConfig {
            staleness_threshold_minutes: Some(30),
            ..Default::default()
        };
        assert_eq!(config.staleness_threshold(), 30);
    }

    #[test]
    fn test_get_alias() {
        let mut config = PluginConfig::default();
        config
            .aliases
            .insert("abc-123".to_string(), "MyProject".to_string());

        assert_eq!(config.get_alias("abc-123"), Some("MyProject"));
        assert_eq!(config.get_alias("xyz-789"), None);
    }

    #[test]
    fn test_format_tasklist_display_with_alias() {
        let mut config = PluginConfig::default();
        config
            .aliases
            .insert("abc-123-def-456".to_string(), "MyProject".to_string());

        let display = format_tasklist_display("abc-123-def-456", &config);
        assert_eq!(display, "MyProject (abc-123-...)");
    }

    #[test]
    fn test_format_tasklist_display_no_alias() {
        let config = PluginConfig::default();
        let display = format_tasklist_display("abc-123", &config);
        assert_eq!(display, "abc-123");
    }

    #[test]
    fn test_parse_toml_config() {
        let toml_str = r#"
            staleness_threshold_minutes = 20

            [aliases]
            "abc-123" = "Project A"
            "def-456" = "Project B"
        "#;

        let config: PluginConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.staleness_threshold(), 20);
        assert_eq!(config.get_alias("abc-123"), Some("Project A"));
        assert_eq!(config.get_alias("def-456"), Some("Project B"));
    }

    #[test]
    fn test_parse_empty_toml() {
        let config: PluginConfig = toml::from_str("").unwrap();
        assert!(config.aliases.is_empty());
        assert_eq!(config.staleness_threshold(), 15);
    }

    #[test]
    fn test_format_age_just_now() {
        let recent = SystemTime::now();
        let age = format_age(recent);
        assert_eq!(age, "just now");
    }

    #[test]
    fn test_format_age_minutes() {
        use std::time::Duration;
        let past = SystemTime::now() - Duration::from_secs(300); // 5 minutes
        let age = format_age(past);
        assert_eq!(age, "5m ago");
    }

    #[test]
    fn test_format_age_hours() {
        use std::time::Duration;
        let past = SystemTime::now() - Duration::from_secs(7200); // 2 hours
        let age = format_age(past);
        assert_eq!(age, "2h ago");
    }

    #[test]
    fn test_format_age_days() {
        use std::time::Duration;
        let past = SystemTime::now() - Duration::from_secs(172800); // 2 days
        let age = format_age(past);
        assert_eq!(age, "2d ago");
    }

    #[test]
    fn test_format_tasklist_option_with_alias() {
        let mut config = PluginConfig::default();
        config
            .aliases
            .insert("abc-123-def-456".to_string(), "MyProject".to_string());

        let tasklist = TasklistInfo {
            id: "abc-123-def-456".to_string(),
            path: PathBuf::from("/test"),
            task_count: 5,
            last_modified: SystemTime::now(),
            sample_tasks: vec![],
        };

        let display = format_tasklist_option(&tasklist, &config);
        assert!(display.starts_with("MyProject (abc-123-...)"));
        assert!(display.contains("5 tasks"));
        assert!(display.contains("just now"));
    }

    #[test]
    fn test_format_tasklist_option_no_alias() {
        let config = PluginConfig::default();

        let tasklist = TasklistInfo {
            id: "abc-123-def-456".to_string(),
            path: PathBuf::from("/test"),
            task_count: 3,
            last_modified: SystemTime::now(),
            sample_tasks: vec![],
        };

        let display = format_tasklist_option(&tasklist, &config);
        assert!(display.starts_with("abc-123-def-456"));
        assert!(display.contains("3 tasks"));
        assert!(display.contains("just now"));
    }
}
