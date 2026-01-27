//! Staleness tracking for tasklist updates.
//!
//! Tracks time since last update and provides human-readable staleness display.
//! Default threshold: 15 minutes without updates triggers stale state.

use std::time::{Duration, Instant};

/// Tracks staleness of a tasklist.
#[derive(Debug)]
pub struct StalenessTracker {
    /// Time of last recorded update
    last_update: Option<Instant>,
    /// Threshold for staleness (default: 15 minutes)
    threshold: Duration,
}

impl StalenessTracker {
    /// Create a new tracker with the given threshold in minutes.
    pub fn new(threshold_minutes: u64) -> Self {
        Self {
            last_update: None,
            threshold: Duration::from_secs(threshold_minutes * 60),
        }
    }

    /// Record that an update was received.
    pub fn record_update(&mut self) {
        self.last_update = Some(Instant::now());
    }

    /// Check if the tasklist is stale.
    ///
    /// Returns None if not stale (or no updates yet), Some(duration) if stale.
    pub fn check_staleness(&self) -> Option<Duration> {
        self.last_update.and_then(|instant| {
            let elapsed = instant.elapsed();
            if elapsed > self.threshold {
                Some(elapsed)
            } else {
                None
            }
        })
    }

    /// Check if we're currently tracking (have received at least one update).
    pub fn is_tracking(&self) -> bool {
        self.last_update.is_some()
    }

    /// Format staleness duration for display.
    ///
    /// Returns None if not stale, Some("Xm") or Some("XhYm") if stale.
    pub fn format_staleness(&self) -> Option<String> {
        self.check_staleness().map(format_duration)
    }

    /// Get time since last update.
    pub fn time_since_update(&self) -> Option<Duration> {
        self.last_update.map(|instant| instant.elapsed())
    }
}

impl Default for StalenessTracker {
    fn default() -> Self {
        Self::new(15) // 15 minutes default
    }
}

/// Format a duration for human display.
///
/// - Less than 60 minutes: "Xm" (e.g., "23m")
/// - 60+ minutes: "XhYm" (e.g., "1h5m")
pub fn format_duration(duration: Duration) -> String {
    let total_minutes = duration.as_secs() / 60;

    if total_minutes < 60 {
        format!("{}m", total_minutes)
    } else {
        let hours = total_minutes / 60;
        let minutes = total_minutes % 60;
        if minutes == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h{}m", hours, minutes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_new_tracker() {
        let tracker = StalenessTracker::new(15);
        assert!(!tracker.is_tracking());
        assert!(tracker.check_staleness().is_none());
    }

    #[test]
    fn test_record_update() {
        let mut tracker = StalenessTracker::new(15);
        tracker.record_update();
        assert!(tracker.is_tracking());
        // Just updated, shouldn't be stale
        assert!(tracker.check_staleness().is_none());
    }

    #[test]
    fn test_staleness_detection() {
        // Use very short threshold for testing
        let mut tracker = StalenessTracker::new(0); // 0 minutes = immediate staleness
        tracker.record_update();

        // Small sleep to ensure time passes
        thread::sleep(Duration::from_millis(10));

        // Should be stale now
        assert!(tracker.check_staleness().is_some());
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0m");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(23 * 60)), "23m");
        assert_eq!(format_duration(Duration::from_secs(59 * 60)), "59m");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(60 * 60)), "1h");
        assert_eq!(format_duration(Duration::from_secs(65 * 60)), "1h5m");
        assert_eq!(format_duration(Duration::from_secs(90 * 60)), "1h30m");
        assert_eq!(format_duration(Duration::from_secs(120 * 60)), "2h");
        assert_eq!(format_duration(Duration::from_secs(125 * 60)), "2h5m");
    }

    #[test]
    fn test_default_threshold() {
        let tracker = StalenessTracker::default();
        assert_eq!(tracker.threshold, Duration::from_secs(15 * 60));
    }

    #[test]
    fn test_format_staleness_not_stale() {
        let mut tracker = StalenessTracker::new(15);
        tracker.record_update();
        assert!(tracker.format_staleness().is_none());
    }

    #[test]
    fn test_format_staleness_when_stale() {
        let mut tracker = StalenessTracker::new(0);
        tracker.record_update();
        thread::sleep(Duration::from_millis(10));
        assert!(tracker.format_staleness().is_some());
    }

    #[test]
    fn test_time_since_update() {
        let mut tracker = StalenessTracker::new(15);
        assert!(tracker.time_since_update().is_none());

        tracker.record_update();
        thread::sleep(Duration::from_millis(10));
        let elapsed = tracker.time_since_update().unwrap();
        assert!(elapsed >= Duration::from_millis(10));
    }
}
