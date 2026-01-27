//! Error types for the Claude Tasks plugin.
//!
//! Provides platform-aware error handling with user-friendly messages.

use notify::ErrorKind;
use std::fmt;

/// Plugin-specific errors with user-facing messages.
#[derive(Debug)]
pub enum PluginError {
    /// Platform-specific watch limit reached (inotify on Linux, file descriptors on macOS)
    WatchLimitReached(String),
    /// Generic watcher failure
    WatcherFailed(String),
    /// Tasks directory not found
    DirectoryNotFound,
    /// Invalid configuration file
    ConfigParseError(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::WatchLimitReached(msg) => write!(f, "{}", msg),
            PluginError::WatcherFailed(msg) => write!(f, "Watch failed: {}", msg),
            PluginError::DirectoryNotFound => write!(f, "Tasks directory not found"),
            PluginError::ConfigParseError(path) => write!(f, "Invalid config: {}", path),
        }
    }
}

impl std::error::Error for PluginError {}

/// Convert a notify error to a user-friendly PluginError.
///
/// Maps platform-specific error codes to actionable messages:
/// - MaxFilesWatch -> inotify watch limit message
/// - ENOSPC (28) on Linux -> inotify watch limit message
/// - EMFILE (24) -> file descriptor limit message
/// - PathNotFound -> DirectoryNotFound
/// - Other -> WatcherFailed with error details
pub fn handle_notify_error(error: &notify::Error) -> PluginError {
    match error.kind {
        ErrorKind::MaxFilesWatch => {
            PluginError::WatchLimitReached("inotify watch limit reached".to_string())
        }
        ErrorKind::Io(ref io_err) => {
            // Check for platform-specific OS errors
            if let Some(os_code) = io_err.raw_os_error() {
                match os_code {
                    28 => {
                        // ENOSPC on Linux - often means inotify watch limit
                        PluginError::WatchLimitReached("inotify watch limit reached".to_string())
                    }
                    24 => {
                        // EMFILE - too many open files (file descriptor limit)
                        PluginError::WatchLimitReached(
                            "File descriptor limit reached".to_string(),
                        )
                    }
                    _ => PluginError::WatcherFailed(error.to_string()),
                }
            } else {
                PluginError::WatcherFailed(error.to_string())
            }
        }
        ErrorKind::PathNotFound => PluginError::DirectoryNotFound,
        _ => PluginError::WatcherFailed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::{Error, ErrorKind};
    use std::io;

    #[test]
    fn test_max_files_watch_error() {
        let error = Error::new(ErrorKind::MaxFilesWatch);
        let plugin_error = handle_notify_error(&error);
        let msg = plugin_error.to_string();
        assert!(msg.contains("inotify") || msg.contains("watch limit"));
    }

    #[test]
    fn test_path_not_found_error() {
        let error = Error::new(ErrorKind::PathNotFound);
        let plugin_error = handle_notify_error(&error);
        assert!(matches!(plugin_error, PluginError::DirectoryNotFound));
    }

    #[test]
    fn test_io_error_emfile() {
        let io_err = io::Error::from_raw_os_error(24);
        let error = Error::new(ErrorKind::Io(io_err.into()));
        let plugin_error = handle_notify_error(&error);
        assert!(plugin_error.to_string().contains("descriptor"));
    }

    #[test]
    fn test_io_error_enospc() {
        let io_err = io::Error::from_raw_os_error(28);
        let error = Error::new(ErrorKind::Io(io_err.into()));
        let plugin_error = handle_notify_error(&error);
        assert!(plugin_error.to_string().contains("inotify"));
    }

    #[test]
    fn test_display_formatting() {
        let err = PluginError::WatcherFailed("test".into());
        assert_eq!(err.to_string(), "Watch failed: test");

        let err = PluginError::DirectoryNotFound;
        assert_eq!(err.to_string(), "Tasks directory not found");

        let err = PluginError::ConfigParseError("/path/config.toml".into());
        assert_eq!(err.to_string(), "Invalid config: /path/config.toml");

        let err = PluginError::WatchLimitReached("inotify watch limit reached".into());
        assert_eq!(err.to_string(), "inotify watch limit reached");
    }
}
