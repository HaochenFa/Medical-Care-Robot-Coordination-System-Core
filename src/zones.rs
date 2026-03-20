//! Zone access control: ensures exclusive occupancy per zone.

use std::collections::{HashMap, HashSet};
use std::sync::{Condvar, Mutex};

use crate::types::{RobotId, ZoneId};

/// Tracks zone ownership and blocks until zones become available.
pub struct ZoneAccess {
    occupied: Mutex<HashMap<ZoneId, RobotId>>,
    condvars: Vec<Condvar>,
}

impl ZoneAccess {
    /// Create a new zone-access controller with per-zone condvars for n zones.
    /// Zones are 1-indexed; index 0 is allocated but unused.
    pub fn new_with_zones(n: usize) -> Self {
        let condvars = (0..=n).map(|_| Condvar::new()).collect();
        Self {
            occupied: Mutex::new(HashMap::new()),
            condvars,
        }
    }

    /// Create a new, empty zone-access controller (default capacity: 256 zones).
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::new_with_zones(256)
    }

    /// Acquire the zone for a robot, blocking until the zone is free.
    pub fn acquire(&self, zone: ZoneId, robot: RobotId) {
        let idx = zone as usize % self.condvars.len();
        let mut guard = self.occupied.lock().expect("zone mutex poisoned");
        loop {
            if !guard.contains_key(&zone) {
                guard.insert(zone, robot);
                return;
            }
            // Wait releases the lock; on wake, re-check the condition.
            guard = self.condvars[idx].wait(guard).expect("condvar wait failed");
        }
    }

    /// Release a zone; returns false if the caller is not the owner.
    pub fn release(&self, zone: ZoneId, robot: RobotId) -> bool {
        let idx = zone as usize % self.condvars.len();
        let mut guard = self.occupied.lock().expect("zone mutex poisoned");
        match guard.get(&zone) {
            Some(owner) if *owner == robot => {
                guard.remove(&zone);
                // Wake one contender so the next robot can acquire the zone.
                self.condvars[idx].notify_one();
                true
            }
            Some(_) => {
                // Non-owner release indicates a logic error in the caller.
                #[cfg(not(debug_assertions))]
                {
                    eprintln!("[ZONE] release by non-owner: zone={zone} robot={robot}");
                }
                debug_assert!(
                    false,
                    "zone release by non-owner: zone={zone} robot={robot}"
                );
                false
            }
            None => {
                // Releasing an unoccupied zone is also a caller error.
                #[cfg(not(debug_assertions))]
                {
                    eprintln!("[ZONE] release on unoccupied zone: zone={zone}");
                }
                debug_assert!(false, "zone release on unoccupied zone: zone={zone}");
                false
            }
        }
    }

    /// Snapshot of zones that are currently occupied.
    pub fn occupied_zones(&self) -> HashSet<ZoneId> {
        let guard = self.occupied.lock().expect("zone mutex poisoned");
        guard.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn zone_is_exclusive_under_contention() {
        let access = Arc::new(ZoneAccess::new());
        let contenders = 6;
        let barrier = Arc::new(Barrier::new(contenders));
        let occupancy = Arc::new(AtomicUsize::new(0));
        let max_occupancy = Arc::new(AtomicUsize::new(0));
        let violation = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();
        for robot_id in 0..contenders {
            let access = Arc::clone(&access);
            let barrier = Arc::clone(&barrier);
            let occupancy = Arc::clone(&occupancy);
            let max_occupancy = Arc::clone(&max_occupancy);
            let violation = Arc::clone(&violation);
            handles.push(thread::spawn(move || {
                barrier.wait();
                access.acquire(1, robot_id as u64);
                let current = occupancy.fetch_add(1, Ordering::SeqCst) + 1;
                let mut prev = max_occupancy.load(Ordering::SeqCst);
                while current > prev {
                    match max_occupancy.compare_exchange(
                        prev,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(next) => prev = next,
                    }
                }
                if current > 1 {
                    violation.store(true, Ordering::SeqCst);
                }
                // Hold the zone briefly to force contention.
                thread::sleep(Duration::from_millis(20));
                occupancy.fetch_sub(1, Ordering::SeqCst);
                assert!(access.release(1, robot_id as u64));
            }));
        }

        for handle in handles {
            handle.join().expect("zone thread panicked");
        }

        assert!(!violation.load(Ordering::SeqCst));
        assert_eq!(max_occupancy.load(Ordering::SeqCst), 1);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "zone release by non-owner")]
    fn release_by_non_owner_panics_in_debug() {
        let access = ZoneAccess::new();
        access.acquire(1, 1);
        let _ = access.release(1, 2);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_by_non_owner_fails_and_keeps_zone() {
        let access = ZoneAccess::new();
        access.acquire(1, 1);
        assert!(!access.release(1, 2));
        let occupied = access.occupied_zones();
        assert!(occupied.contains(&1));
        assert!(access.release(1, 1));
    }
}
