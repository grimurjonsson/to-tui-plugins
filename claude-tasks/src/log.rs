//! Simple file-based logging for plugin debugging.
//!
//! Plugins don't share the host's tracing subscriber, so we use
//! a simple file-based approach for debugging.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Once;

static INIT: Once = Once::new();

fn log_file_path() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("claude-tasks-plugin.log")
}

/// Initialize the log file (truncate on first use).
fn init_log() {
    INIT.call_once(|| {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(log_file_path())
        {
            let _ = writeln!(file, "=== Claude Tasks Plugin Log ===");
            let _ = writeln!(file, "Log file: {}", log_file_path().display());
            let _ = writeln!(file, "");
        }
    });
}

/// Log a message to the plugin log file.
pub fn log(level: &str, message: &str) {
    init_log();

    if let Ok(mut file) = OpenOptions::new()
        .append(true)
        .create(true)
        .open(log_file_path())
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}: {}", timestamp, level, message);
    }
}

/// Log an info message.
#[macro_export]
macro_rules! plugin_info {
    ($($arg:tt)*) => {
        $crate::log::log("INFO", &format!($($arg)*))
    };
}

/// Log a debug message.
#[macro_export]
macro_rules! plugin_debug {
    ($($arg:tt)*) => {
        $crate::log::log("DEBUG", &format!($($arg)*))
    };
}
