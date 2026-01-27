//! File watcher setup with notify-debouncer-full.
//!
//! Provides debounced file system watching for Claude tasklist directories.
//! Events are sent via mpsc channel to the main plugin thread.

use crate::errors::handle_notify_error;
use crate::state::SyncEvent;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Handle to the watcher thread.
///
/// Keeps the watcher thread alive and provides graceful shutdown.
/// Implements Drop for automatic cleanup when the plugin unloads.
pub struct WatcherHandle {
    /// The thread handle for the watcher loop
    thread_handle: Option<JoinHandle<()>>,
    /// Flag to signal the thread to shut down
    shutdown_flag: Arc<AtomicBool>,
}

impl WatcherHandle {
    /// Create a new WatcherHandle with the given thread handle and shutdown flag.
    fn new(handle: JoinHandle<()>, shutdown_flag: Arc<AtomicBool>) -> Self {
        Self {
            thread_handle: Some(handle),
            shutdown_flag,
        }
    }

    /// Gracefully shut down the watcher thread.
    ///
    /// Signals the thread to stop and waits for it to finish.
    pub fn shutdown(&mut self) {
        // Signal thread to stop
        self.shutdown_flag.store(true, Ordering::SeqCst);

        // Take ownership and join
        if let Some(handle) = self.thread_handle.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl std::fmt::Debug for WatcherHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatcherHandle")
            .field("thread_alive", &self.thread_handle.is_some())
            .finish()
    }
}

/// Start a file watcher for the given tasklist directory.
///
/// The watcher uses notify-debouncer-full with a 200ms timeout to batch
/// rapid file system events. Events are translated to SyncEvent and sent
/// through the provided mpsc channel.
///
/// # Arguments
/// * `tasklist_path` - The directory to watch (e.g., ~/.claude/tasks/{uuid}/)
/// * `tx` - Channel sender for SyncEvent notifications
///
/// # Returns
/// * `Ok(WatcherHandle)` - Handle to the watcher thread
/// * `Err(String)` - Error message if watcher setup failed
pub fn start_watcher(
    tasklist_path: PathBuf,
    tx: mpsc::Sender<SyncEvent>,
) -> Result<WatcherHandle, String> {
    let tx_for_debouncer = tx.clone();
    let path_for_thread = tasklist_path.clone();

    // Create shutdown flag for graceful termination
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_for_thread = shutdown_flag.clone();

    // Spawn the watcher thread
    let handle = thread::spawn(move || {
        // Create debouncer with 200ms timeout
        let debouncer_result = new_debouncer(
            Duration::from_millis(200),
            None,
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    for event in events {
                        if let Some(sync_event) = translate_event(&event) {
                            // Ignore send errors - receiver might be dropped
                            let _ = tx_for_debouncer.send(sync_event);
                        }
                    }
                }
            },
        );

        let mut debouncer = match debouncer_result {
            Ok(d) => d,
            Err(e) => {
                let plugin_error = handle_notify_error(&e);
                eprintln!("claude-tasks: Failed to create debouncer: {}", plugin_error);
                return;
            }
        };

        // Watch the tasklist directory
        if let Err(e) = debouncer.watch(&path_for_thread, RecursiveMode::Recursive) {
            let plugin_error = handle_notify_error(&e);
            eprintln!("claude-tasks: {}", plugin_error);
            return;
        }

        // Keep thread alive - debouncer needs to stay in scope
        // Check shutdown flag every 100ms for graceful termination
        loop {
            if shutdown_for_thread.load(Ordering::SeqCst) {
                break;
            }
            thread::park_timeout(Duration::from_millis(100));
        }
    });

    Ok(WatcherHandle::new(handle, shutdown_flag))
}

/// Translate a debounced file system event to a SyncEvent.
///
/// Only processes .json files. Returns None for non-json files or
/// event types we don't care about.
fn translate_event(event: &DebouncedEvent) -> Option<SyncEvent> {
    use notify::EventKind;

    // Get the first path from the event (most events have exactly one)
    let path = event.paths.first()?.clone();

    // Only process .json files
    if path.extension().map(|e| e != "json").unwrap_or(true) {
        return None;
    }

    match &event.kind {
        // File created or modified
        EventKind::Create(_) | EventKind::Modify(_) => Some(SyncEvent::FileChanged(path)),
        // File removed
        EventKind::Remove(_) => Some(SyncEvent::FileRemoved(path)),
        // Ignore other events (access, metadata-only, etc.)
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use notify::EventKind;

    fn make_event(kind: EventKind, path: &str) -> DebouncedEvent {
        DebouncedEvent {
            event: notify::Event {
                kind,
                paths: vec![PathBuf::from(path)],
                attrs: Default::default(),
            },
            time: std::time::Instant::now(),
        }
    }

    #[test]
    fn test_translate_create_json() {
        let event = make_event(EventKind::Create(CreateKind::File), "/path/to/1.json");
        let result = translate_event(&event);
        assert!(matches!(result, Some(SyncEvent::FileChanged(_))));
    }

    #[test]
    fn test_translate_modify_json() {
        let event = make_event(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            "/path/to/1.json",
        );
        let result = translate_event(&event);
        assert!(matches!(result, Some(SyncEvent::FileChanged(_))));
    }

    #[test]
    fn test_translate_remove_json() {
        let event = make_event(EventKind::Remove(RemoveKind::File), "/path/to/1.json");
        let result = translate_event(&event);
        assert!(matches!(result, Some(SyncEvent::FileRemoved(_))));
    }

    #[test]
    fn test_translate_ignores_non_json() {
        let event = make_event(EventKind::Create(CreateKind::File), "/path/to/file.txt");
        let result = translate_event(&event);
        assert!(result.is_none());
    }

    #[test]
    fn test_translate_ignores_no_extension() {
        let event = make_event(EventKind::Create(CreateKind::File), "/path/to/somefile");
        let result = translate_event(&event);
        assert!(result.is_none());
    }

    #[test]
    fn test_watcher_handle_shutdown() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        let handle = thread::spawn(move || {
            while !flag_clone.load(Ordering::SeqCst) {
                thread::park_timeout(Duration::from_millis(10));
            }
        });

        let mut watcher = WatcherHandle {
            thread_handle: Some(handle),
            shutdown_flag: flag,
        };

        // Shutdown should complete without hanging
        watcher.shutdown();
        assert!(watcher.thread_handle.is_none());
    }
}
