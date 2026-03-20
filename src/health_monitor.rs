//! Heartbeat tracking and offline detection for robots.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::types::RobotId;

/// Tracks robot heartbeats and reports offline robots after a timeout.
pub struct HealthMonitor {
    inner: Mutex<HealthState>,
}

struct HealthState {
    heartbeats: HashMap<RobotId, Instant>,
    offline: HashSet<RobotId>,
}

impl HealthMonitor {
    /// Create an empty health monitor.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HealthState {
                heartbeats: HashMap::new(),
                offline: HashSet::new(),
            }),
        }
    }

    /// Ensure a robot is tracked; no-op if already registered.
    pub fn register_robot(&self, robot: RobotId) {
        self.inner
            .lock()
            .expect("health monitor mutex poisoned")
            .heartbeats
            .entry(robot)
            .or_insert_with(Instant::now);
    }

    /// Record a heartbeat; clears any prior offline mark for the robot.
    pub fn heartbeat(&self, robot: RobotId) {
        let mut guard = self
            .inner
            .lock()
            .expect("health monitor mutex poisoned");
        guard.heartbeats.insert(robot, Instant::now());
        guard.offline.remove(&robot);
    }

    /// Detect robots whose last heartbeat exceeds the timeout.
    pub fn detect_offline(&self, timeout: Duration) -> HashSet<RobotId> {
        self.refresh_offline_with_hook(timeout, || {})
    }

    /// Detect offline robots and report whether any are offline.
    pub fn detect_offline_any(&self, timeout: Duration) -> bool {
        !self.detect_offline(timeout).is_empty()
    }

    fn refresh_offline_with_hook<F>(&self, timeout: Duration, before_commit: F) -> HashSet<RobotId>
    where
        F: FnOnce(),
    {
        let now = Instant::now();
        let mut guard = self
            .inner
            .lock()
            .expect("health monitor mutex poisoned");
        let overdue: Vec<_> = guard
            .heartbeats
            .iter()
            .filter_map(|(&robot, &last)| {
                if now.duration_since(last) > timeout {
                    Some(robot)
                } else {
                    None
                }
            })
            .collect();
        before_commit();
        for robot in overdue {
            guard.offline.insert(robot);
        }
        guard.offline.clone()
    }

    /// Snapshot of the robots currently marked offline.
    pub fn offline_robots(&self) -> HashSet<RobotId> {
        self.inner
            .lock()
            .expect("health monitor mutex poisoned")
            .offline
            .clone()
    }

    /// Test-only hook to set deterministic timestamps without sleeping.
    #[cfg(test)]
    fn set_last_seen_for_test(&self, robot: RobotId, instant: Instant) {
        self.inner
            .lock()
            .expect("health monitor mutex poisoned")
            .heartbeats
            .insert(robot, instant);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn detects_offline_after_timeout() {
        let monitor = HealthMonitor::new();
        let robot = 7;
        let past = Instant::now() - Duration::from_millis(50);
        monitor.set_last_seen_for_test(robot, past);
        // Timeout shorter than elapsed time should mark offline.
        let offline = monitor.detect_offline(Duration::from_millis(10));
        assert!(offline.contains(&robot));
    }

    #[test]
    fn marks_never_heartbeat_after_timeout() {
        let monitor = HealthMonitor::new();
        let robot = 11;
        let past = Instant::now() - Duration::from_millis(30);
        monitor.set_last_seen_for_test(robot, past);
        let offline = monitor.detect_offline(Duration::from_millis(5));
        assert!(offline.contains(&robot));
    }

    #[test]
    fn heartbeat_clears_offline() {
        let monitor = HealthMonitor::new();
        let robot = 21;
        let past = Instant::now() - Duration::from_millis(30);
        monitor.set_last_seen_for_test(robot, past);
        monitor.detect_offline(Duration::from_millis(5));
        assert!(monitor.offline_robots().contains(&robot));
        // Heartbeat should clear the offline status.
        monitor.heartbeat(robot);
        assert!(!monitor.offline_robots().contains(&robot));
    }

    #[test]
    fn deterministic_offline_without_sleep() {
        let monitor = HealthMonitor::new();
        let robot = 42;
        let past = Instant::now() - Duration::from_secs(5);
        monitor.set_last_seen_for_test(robot, past);
        let offline = monitor.detect_offline(Duration::from_secs(1));
        assert!(offline.contains(&robot));
    }

    #[test]
    fn concurrent_heartbeat_cannot_restore_stale_offline_mark() {
        let monitor = Arc::new(HealthMonitor::new());
        let robot = 9;
        let past = Instant::now() - Duration::from_millis(30);
        monitor.set_last_seen_for_test(robot, past);

        let snapshot_ready = Arc::new(Barrier::new(2));
        let detect_monitor = Arc::clone(&monitor);
        let detect_barrier = Arc::clone(&snapshot_ready);
        let detection = thread::spawn(move || {
            detect_monitor.refresh_offline_with_hook(Duration::from_millis(5), || {
                detect_barrier.wait();
            })
        });

        snapshot_ready.wait();

        let heartbeat_monitor = Arc::clone(&monitor);
        let heartbeat = thread::spawn(move || heartbeat_monitor.heartbeat(robot));

        let detected = detection.join().expect("offline detection thread panicked");
        heartbeat.join().expect("heartbeat thread panicked");

        assert!(detected.contains(&robot));
        assert!(
            !monitor.offline_robots().contains(&robot),
            "heartbeat should win over a stale offline snapshot"
        );
    }
}
