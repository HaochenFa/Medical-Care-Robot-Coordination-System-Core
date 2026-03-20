//! Simulation, benchmark, and stress-test runners for Project Blaze.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::health_monitor::HealthMonitor;
use crate::log_dev;
use crate::logging::{BOLD, CYAN, GRAY, GREEN, RED, RESET, YELLOW};
use crate::task_queue::TaskQueue;
use crate::types::Task;
use crate::zones::ZoneAccess;

// Demo/offline timing knobs (small for quick CLI feedback).
const DEMO_OFFLINE_TIMEOUT_MS: u64 = 200;
const DEMO_OFFLINE_MAX_WAIT_MS: u64 = 1000;
const DEMO_OFFLINE_TARGET_ROBOT: u64 = 1;
// Benchmark offline timing (looser to reduce false positives).
const BENCH_OFFLINE_TIMEOUT_MS: u64 = 500;
const BENCH_OFFLINE_MAX_WAIT_MS: u64 = 1000;
// Polling interval used while waiting for offline detection.
const OFFLINE_POLL_MS: u64 = 50;

/// Best-effort CPU user/system time snapshot (seconds) on Unix platforms.
#[cfg(unix)]
fn cpu_times_seconds() -> Option<(f64, f64)> {
    use libc::{RUSAGE_SELF, getrusage, rusage};
    let mut usage = rusage {
        ru_utime: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        ru_stime: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        ru_maxrss: 0,
        ru_ixrss: 0,
        ru_idrss: 0,
        ru_isrss: 0,
        ru_minflt: 0,
        ru_majflt: 0,
        ru_nswap: 0,
        ru_inblock: 0,
        ru_oublock: 0,
        ru_msgsnd: 0,
        ru_msgrcv: 0,
        ru_nsignals: 0,
        ru_nvcsw: 0,
        ru_nivcsw: 0,
    };
    let rc = unsafe { getrusage(RUSAGE_SELF, &mut usage) };
    if rc != 0 {
        return None;
    }
    let user = usage.ru_utime.tv_sec as f64 + (usage.ru_utime.tv_usec as f64 / 1_000_000.0);
    let sys = usage.ru_stime.tv_sec as f64 + (usage.ru_stime.tv_usec as f64 / 1_000_000.0);
    Some((user, sys))
}

/// Stub on non-Unix platforms.
#[cfg(not(unix))]
fn cpu_times_seconds() -> Option<(f64, f64)> {
    None
}

/// Spawn a background thread that periodically runs offline detection
/// using the provided `HealthMonitor` until `stop_flag` is set.
fn spawn_health_monitor(
    monitor: Arc<HealthMonitor>,
    stop_flag: Arc<AtomicBool>,
    timeout: Duration,
    poll: Duration,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while !stop_flag.load(Ordering::SeqCst) {
            let _ = monitor.detect_offline_any(timeout);
            thread::sleep(poll);
        }
    })
}

/// Wait until at least one robot is offline or a max wait is reached.
fn wait_for_offline(monitor: &HealthMonitor, timeout_ms: u64, max_wait_ms: u64) {
    let max_wait = Duration::from_millis(max_wait_ms);
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();
    loop {
        if monitor.detect_offline_any(timeout) || start.elapsed() >= max_wait {
            return;
        }
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let remaining_ms = timeout_ms.saturating_sub(elapsed_ms).max(1);
        thread::sleep(Duration::from_millis(OFFLINE_POLL_MS.min(remaining_ms)));
    }
}

/// Wait for a specific robot to be marked offline while keeping others alive.
fn wait_for_specific_offline(
    monitor: &HealthMonitor,
    target_robot: u64,
    keepalive_robots: &[u64],
    timeout_ms: u64,
    max_wait_ms: u64,
) -> bool {
    let max_wait = Duration::from_millis(max_wait_ms);
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();
    loop {
        // Keep non-target robots fresh so demo output is deterministic.
        for &robot in keepalive_robots {
            monitor.heartbeat(robot);
        }
        let offline = monitor.detect_offline(timeout);
        if offline.contains(&target_robot) {
            return true;
        }
        if start.elapsed() >= max_wait {
            return false;
        }
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let remaining_ms = timeout_ms.saturating_sub(elapsed_ms).max(1);
        thread::sleep(Duration::from_millis(OFFLINE_POLL_MS.min(remaining_ms)));
    }
}

/// Pre-size per-zone occupancy counters (index 1..=zones_total).
fn init_zone_counters(zones_total: usize) -> Vec<AtomicUsize> {
    let mut counters = Vec::with_capacity(zones_total + 1);
    for _ in 0..=zones_total {
        counters.push(AtomicUsize::new(0));
    }
    counters
}

/// Tracks aggregate and per-zone occupancy metrics and constraint violations.
struct ZoneMetrics {
    occupancy: AtomicUsize,
    max_occupancy: AtomicUsize,
    zone_violation: AtomicBool,
    per_zone_occupancy: Vec<AtomicUsize>,
}

impl ZoneMetrics {
    fn new(zones_total: usize) -> Self {
        Self {
            occupancy: AtomicUsize::new(0),
            max_occupancy: AtomicUsize::new(0),
            zone_violation: AtomicBool::new(false),
            per_zone_occupancy: init_zone_counters(zones_total),
        }
    }

    fn enter(&self, zone: u64, zones_total: usize) {
        let current = self.occupancy.fetch_add(1, Ordering::SeqCst) + 1;
        let zone_index = zone as usize;
        // Zone ids are 1-based; index 0 is unused.
        debug_assert!(zone_index <= zones_total, "zone index out of range");
        let zone_count = self.per_zone_occupancy[zone_index].fetch_add(1, Ordering::SeqCst) + 1;
        if zone_count > 1 {
            self.zone_violation.store(true, Ordering::SeqCst);
        }
        let mut prev = self.max_occupancy.load(Ordering::SeqCst);
        while current > prev {
            match self.max_occupancy.compare_exchange(
                prev,
                current,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(next) => prev = next,
            }
        }
        if current > zones_total {
            self.zone_violation.store(true, Ordering::SeqCst);
        }
    }

    fn pre_release(&self, zone: u64, zones_total: usize) {
        let zone_index = zone as usize;
        debug_assert!(zone_index <= zones_total, "zone index out of range");
        let zone_prev = self.per_zone_occupancy[zone_index].fetch_sub(1, Ordering::SeqCst);
        debug_assert!(zone_prev > 0, "zone counter underflow");
        let occ_prev = self.occupancy.fetch_sub(1, Ordering::SeqCst);
        debug_assert!(occ_prev > 0, "occupancy counter underflow");
    }

    fn revert_pre_release(&self, zone: u64, zones_total: usize) {
        let zone_index = zone as usize;
        debug_assert!(zone_index <= zones_total, "zone index out of range");
        self.per_zone_occupancy[zone_index].fetch_add(1, Ordering::SeqCst);
        self.occupancy.fetch_add(1, Ordering::SeqCst);
    }

    fn max_occupancy(&self) -> usize {
        self.max_occupancy.load(Ordering::SeqCst)
    }

    fn has_violation(&self) -> bool {
        self.zone_violation.load(Ordering::SeqCst)
    }
}

/// Aggregated metrics from a single benchmark run.
struct BenchResult {
    robots: usize,
    tasks_per_robot: usize,
    zones_total: u64,
    total_tasks: usize,
    elapsed_ms: f64,
    throughput: f64,
    avg_zone_wait_us: f64,
    cpu_user_s: Option<f64>,
    cpu_sys_s: Option<f64>,
    leftover: usize,
    max_occupancy: usize,
    zone_violation: bool,
    duplicate_tasks: bool,
    offline_count: usize,
}

fn benchmark_once(
    robots: usize,
    tasks_per_robot: usize,
    zones_total: u64,
    work_ms: u64,
    validate: bool,
    simulate_offline: bool,
) -> BenchResult {
    debug_assert!(robots > 0, "robots must be > 0");
    debug_assert!(tasks_per_robot > 0, "tasks_per_robot must be > 0");
    debug_assert!(zones_total > 0, "zones_total must be > 0");
    let zones_len = zones_total as usize;
    let queue = Arc::new(TaskQueue::new());
    let zones = Arc::new(ZoneAccess::new_with_zones(zones_len));
    let monitor = Arc::new(HealthMonitor::new());
    let stop_flag = Arc::new(AtomicBool::new(false));

    let total_tasks = robots * tasks_per_robot;
    for id in 0..total_tasks {
        queue
            .push(Task::new(id as u64, format!("bench-{id}")))
            .expect("task queue closed");
    }
    let total_tasks = queue.len();

    // Total wait time across all zone acquisitions for averaging.
    let zone_wait_us = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let zone_metrics = Arc::new(ZoneMetrics::new(zones_len));
    let duplicate_tasks = Arc::new(AtomicBool::new(false));
    let (task_tx, task_rx) = if validate {
        let (tx, rx) = mpsc::channel::<u64>();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    for robot_id in 0..robots {
        monitor.register_robot(robot_id as u64);
    }

    let monitor_thread = spawn_health_monitor(
        Arc::clone(&monitor),
        Arc::clone(&stop_flag),
        Duration::from_millis(BENCH_OFFLINE_TIMEOUT_MS),
        Duration::from_millis(100),
    );

    let mut handles = Vec::new();
    let cpu_start = cpu_times_seconds();
    let start = Instant::now();
    for robot_id in 0..robots {
        let queue = Arc::clone(&queue);
        let zones = Arc::clone(&zones);
        let zone_wait_us = Arc::clone(&zone_wait_us);
        let monitor = Arc::clone(&monitor);
        let zone_metrics = Arc::clone(&zone_metrics);
        let task_sender = task_tx.as_ref().map(|tx| tx.clone());
        handles.push(thread::spawn(move || {
            let stop_after = if simulate_offline && robots > 1 && robot_id == 0 {
                tasks_per_robot / 2
            } else {
                usize::MAX
            };
            let mut completed = 0usize;
            while completed < tasks_per_robot {
                let task = queue.pop_blocking_or_closed().expect("task queue closed");
                if let Some(ref s) = task_sender {
                    let _ = s.send(task.id);
                }
                let zone = (task.id % zones_total) + 1;
                let wait_start = Instant::now();
                zones.acquire(zone, robot_id as u64);
                let waited = wait_start.elapsed().as_micros() as u64;
                zone_wait_us.fetch_add(waited, Ordering::SeqCst);
                zone_metrics.enter(zone, zones_len);
                if work_ms > 0 {
                    thread::sleep(Duration::from_millis(work_ms));
                }
                zone_metrics.pre_release(zone, zones_len);
                let released = zones.release(zone, robot_id as u64);
                if !released {
                    log_dev!("[ZONE] bench release failed zone={zone} robot={robot_id}");
                    zone_metrics.revert_pre_release(zone, zones_len);
                }
                completed += 1;
                // Optionally stop heartbeats early to simulate offline detection.
                if completed <= stop_after {
                    monitor.heartbeat(robot_id as u64);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().expect("benchmark thread panicked");
    }
    // Drop the original sender so the channel closes when all thread senders drop.
    drop(task_tx);
    if let Some(rx) = task_rx {
        let mut seen = HashSet::new();
        while let Ok(id) = rx.try_recv() {
            if !seen.insert(id) {
                duplicate_tasks.store(true, Ordering::SeqCst);
            }
        }
    }
    if simulate_offline {
        wait_for_offline(
            &monitor,
            BENCH_OFFLINE_TIMEOUT_MS,
            BENCH_OFFLINE_MAX_WAIT_MS,
        );
    }
    stop_flag.store(true, Ordering::SeqCst);
    monitor_thread
        .join()
        .expect("health monitor thread panicked");

    // Drain any unexpected leftover tasks for validation reporting.
    let mut leftover = 0usize;
    while queue.try_pop().is_some() {
        leftover += 1;
    }

    let elapsed_ms = start.elapsed().as_millis() as f64;
    let throughput = if elapsed_ms > 0.0 {
        (total_tasks as f64) / (elapsed_ms / 1000.0)
    } else {
        0.0
    };
    let avg_zone_wait = if total_tasks > 0 {
        zone_wait_us.load(Ordering::SeqCst) as f64 / total_tasks as f64
    } else {
        0.0
    };

    let (cpu_user_s, cpu_sys_s) = match (cpu_start, cpu_times_seconds()) {
        (Some((user_start, sys_start)), Some((user_end, sys_end))) => {
            (Some(user_end - user_start), Some(sys_end - sys_start))
        }
        _ => (None, None),
    };

    BenchResult {
        robots,
        tasks_per_robot,
        zones_total,
        total_tasks,
        elapsed_ms,
        throughput,
        avg_zone_wait_us: avg_zone_wait,
        cpu_user_s,
        cpu_sys_s,
        leftover,
        max_occupancy: zone_metrics.max_occupancy(),
        zone_violation: zone_metrics.has_violation(),
        duplicate_tasks: duplicate_tasks.load(Ordering::SeqCst),
        offline_count: monitor.offline_robots().len(),
    }
}

/// Run the default demo showing queueing, zoning, and offline detection.
pub fn run_demo() {
    crate::logging::init_demo_start();
    println!("{BOLD}{CYAN}╔════════════════════════════════════════════════════════════╗{RESET}");
    println!("{BOLD}{CYAN}║                    Project Blaze — Demo                    ║{RESET}");
    println!("{BOLD}{CYAN}║                  robots=3  zones=2  tasks=9                ║{RESET}");
    println!("{BOLD}{CYAN}╚════════════════════════════════════════════════════════════╝{RESET}");
    println!();
    log_dev!("[DEMO] start");

    let queue = Arc::new(TaskQueue::new());
    let robots = 3;
    let tasks_per_robot = 3;
    let zones_total = 2;
    let zones = Arc::new(ZoneAccess::new_with_zones(zones_total));
    let monitor = Arc::new(HealthMonitor::new());

    let offline_target = DEMO_OFFLINE_TARGET_ROBOT;
    assert!(
        (offline_target as usize) < robots,
        "offline target {offline_target} out of range for robots={robots}"
    );

    // Track per-robot completions for the final summary.
    let per_robot_tasks = Arc::new((0..robots).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let zone_metrics = Arc::new(ZoneMetrics::new(zones_total));

    for id in 0..(robots * tasks_per_robot) {
        queue
            .push(Task::new(id as u64, format!("deliver-{id}")))
            .expect("task queue closed");
    }
    log_dev!(
        "[QUEUE] loaded tasks total={} per_robot={}",
        robots * tasks_per_robot,
        tasks_per_robot
    );
    if cfg!(debug_assertions) {
        println!();
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    for robot_id in 0..robots {
        monitor.register_robot(robot_id as u64);
    }

    let monitor_thread = {
        let monitor = Arc::clone(&monitor);
        let stop_flag = Arc::clone(&stop_flag);
        thread::Builder::new()
            .name("health-monitor".to_string())
            .spawn(move || {
                let timeout = Duration::from_millis(DEMO_OFFLINE_TIMEOUT_MS);
                let mut already_offline = HashSet::new();
                while !stop_flag.load(Ordering::SeqCst) {
                    let offline = monitor.detect_offline(timeout);
                    for robot in offline {
                        if already_offline.insert(robot) {
                            log_dev!("[HEALTH] robot {robot} marked offline");
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            })
            .expect("failed to spawn health monitor")
    };

    let mut handles = Vec::new();
    for robot_id in 0..robots {
        let queue = Arc::clone(&queue);
        let zones = Arc::clone(&zones);
        let monitor = Arc::clone(&monitor);
        let per_robot_tasks = Arc::clone(&per_robot_tasks);
        let zone_metrics = Arc::clone(&zone_metrics);
        let name = format!("robot-{robot_id}");
        let handle = thread::Builder::new()
            .name(name.clone())
            .spawn(move || {
                let mut completed = 0;
                // One fixed robot stops heartbeats mid-demo to trigger deterministic offline detection.
                let stop_heartbeat_after = if robot_id as u64 == offline_target {
                    2
                } else {
                    usize::MAX
                };
                while completed < tasks_per_robot {
                    let task = queue.pop_blocking_or_closed().expect("task queue closed");
                    per_robot_tasks[robot_id].fetch_add(1, Ordering::SeqCst);
                    log_dev!("[QUEUE] {name} fetched task {}", task.id);
                    let zone = (task.id % zones_total as u64) + 1;
                    zones.acquire(zone, robot_id as u64);
                    zone_metrics.enter(zone, zones_total);
                    log_dev!("[ZONE] {name} entered zone {zone} for task {}", task.id);
                    thread::sleep(Duration::from_millis(80));
                    zone_metrics.pre_release(zone, zones_total);
                    let released = zones.release(zone, robot_id as u64);
                    if !released {
                        log_dev!("[ZONE] {name} failed to release zone {zone}");
                        zone_metrics.revert_pre_release(zone, zones_total);
                    }
                    log_dev!("[ZONE] {name} left zone {zone} for task {}", task.id);
                    completed += 1;
                    if completed <= stop_heartbeat_after {
                        monitor.heartbeat(robot_id as u64);
                        log_dev!("[HEALTH] {name} heartbeat");
                    } else {
                        log_dev!("[HEALTH] {name} stops heartbeats");
                    }
                    if cfg!(debug_assertions) {
                        println!();
                    }
                }
            })
            .expect("failed to spawn robot thread");
        handles.push(handle);
    }

    let start = Instant::now();
    for handle in handles {
        handle.join().expect("robot thread panicked");
    }
    let keepalive_robots: Vec<u64> = (0..robots as u64)
        .filter(|&robot| robot != offline_target)
        .collect();
    let offline_target_detected = wait_for_specific_offline(
        &monitor,
        offline_target,
        &keepalive_robots,
        DEMO_OFFLINE_TIMEOUT_MS,
        DEMO_OFFLINE_MAX_WAIT_MS,
    );
    stop_flag.store(true, Ordering::SeqCst);
    monitor_thread
        .join()
        .expect("health monitor thread panicked");

    let occupied = zones.occupied_zones();
    if cfg!(debug_assertions) {
        println!("{GRAY}  ────────────────────────────────────────────────────────────{RESET}");
    }
    log_dev!("[ZONE] occupied_zones at end = {}", occupied.len());
    let offline = monitor.offline_robots();
    log_dev!("[HEALTH] offline robots at end = {}", offline.len());
    if !offline.is_empty() {
        log_dev!("[HEALTH] offline set = {:?}", offline);
    }
    log_dev!(
        "[DEMO] finished in {}ms (dev logs suppressed in release mode)",
        start.elapsed().as_millis()
    );

    let tasks_done: Vec<usize> = per_robot_tasks
        .iter()
        .map(|count| count.load(Ordering::SeqCst))
        .collect();
    let viol_str = if zone_metrics.has_violation() {
        format!("{RED}✗ true{RESET}")
    } else {
        format!("{GREEN}✓ false{RESET}")
    };
    let det_str = if offline_target_detected {
        format!("{GREEN}✓ true{RESET}")
    } else {
        format!("{RED}✗ false{RESET}")
    };
    let offline_str = if offline.is_empty() {
        format!("{GRAY}none{RESET}")
    } else {
        format!("{RED}{offline:?}{RESET}")
    };
    // Inner box width (visible chars between the two ║).
    const W: usize = 60;
    // Build a padded row: ║  label<16 value_colored pad ║
    let row = |label: &str, value_colored: &str, value_plain: &str| -> String {
        // label is printed with a fixed 16-char field; recompute using that
        let label_field = format!("{label:<16}");
        let visible = 2 + label_field.len() + value_plain.chars().count();
        let pad = W.saturating_sub(visible);
        format!(
            "{BOLD}║{RESET}  {label_field}{value_colored}{}{BOLD}║{RESET}",
            " ".repeat(pad)
        )
    };
    println!();
    println!("{BOLD}╔════════════════════════════════════════════════════════════╗{RESET}");
    println!("{BOLD}║                        DEMO SUMMARY                        ║{RESET}");
    println!("{BOLD}╠════════════════════════════════════════════════════════════╣{RESET}");
    println!(
        "{}",
        row(
            "robots",
            &format!("{CYAN}{robots}{RESET}"),
            &robots.to_string()
        )
    );
    println!(
        "{}",
        row(
            "tasks_total",
            &format!("{CYAN}{}{RESET}", robots * tasks_per_robot),
            &(robots * tasks_per_robot).to_string()
        )
    );
    println!(
        "{}",
        row(
            "per_robot_done",
            &format!("{CYAN}{tasks_done:?}{RESET}"),
            &format!("{tasks_done:?}")
        )
    );
    println!(
        "{}",
        row(
            "max_zone_occ",
            &format!("{YELLOW}{}{RESET}", zone_metrics.max_occupancy()),
            &zone_metrics.max_occupancy().to_string()
        )
    );
    println!(
        "{}",
        row(
            "zone_violation",
            &viol_str,
            if zone_metrics.has_violation() {
                "✗ true"
            } else {
                "✓ false"
            }
        )
    );
    println!(
        "{}",
        row(
            "offline_target",
            &format!("{YELLOW}{offline_target}{RESET}"),
            &offline_target.to_string()
        )
    );
    println!(
        "{}",
        row(
            "detected",
            &det_str,
            if offline_target_detected {
                "✓ true"
            } else {
                "✗ false"
            }
        )
    );
    println!(
        "{}",
        row(
            "offline_robots",
            &offline_str,
            &if offline.is_empty() {
                "none".to_string()
            } else {
                format!("{offline:?}")
            }
        )
    );
    println!("{BOLD}╚════════════════════════════════════════════════════════════╝{RESET}");
}

/// Run a single benchmark with optional parameter overrides.
pub fn run_benchmark(
    robots: Option<usize>,
    tasks_per_robot: Option<usize>,
    zones_total: Option<u64>,
    work_ms: Option<u64>,
    validate: bool,
    simulate_offline: bool,
) {
    let robots = robots.unwrap_or(4);
    let tasks_per_robot = tasks_per_robot.unwrap_or(25);
    let zones_total = zones_total.unwrap_or(2);
    let work_ms = work_ms.unwrap_or(5);
    if robots == 0 {
        eprintln!("benchmark error: robots must be > 0");
        return;
    }
    if tasks_per_robot == 0 {
        eprintln!("benchmark error: tasks_per_robot must be > 0");
        return;
    }
    if zones_total == 0 {
        eprintln!("benchmark error: zones must be > 0");
        return;
    }
    let result = benchmark_once(
        robots,
        tasks_per_robot,
        zones_total,
        work_ms,
        validate,
        simulate_offline,
    );

    print_bench_box(&result, robots, tasks_per_robot, zones_total, work_ms);
    if result.leftover > 0 {
        eprintln!("# warning,leftover_tasks,{}", result.leftover);
    }
    if validate {
        if result.zone_violation {
            eprintln!("# violation,zone_exclusivity");
        }
        if result.duplicate_tasks {
            eprintln!("# violation,duplicate_tasks");
        }
    }
}

fn print_bench_box(
    result: &BenchResult,
    robots: usize,
    tasks_per_robot: usize,
    zones_total: u64,
    work_ms: u64,
) {
    use crate::logging::*;
    const W: usize = 60;
    let row = |label: &str, value_colored: &str, value_plain: &str| -> String {
        let label_field = format!("{label:<18}");
        let visible = 2 + label_field.chars().count() + value_plain.chars().count();
        let pad = W.saturating_sub(visible);
        format!(
            "{BOLD}║{RESET}  {label_field}{value_colored}{}{BOLD}║{RESET}",
            " ".repeat(pad)
        )
    };
    let cpu_user = result
        .cpu_user_s
        .map(|v| format!("{v:.4}"))
        .unwrap_or_else(|| "NA".to_string());
    let cpu_sys = result
        .cpu_sys_s
        .map(|v| format!("{v:.4}"))
        .unwrap_or_else(|| "NA".to_string());
    let viol_str = if result.zone_violation {
        format!("{RED}✗ true{RESET}")
    } else {
        format!("{GREEN}✓ false{RESET}")
    };
    let dup_str = if result.duplicate_tasks {
        format!("{RED}✗ true{RESET}")
    } else {
        format!("{GREEN}✓ false{RESET}")
    };
    let banner_params = format!(
        "robots={}  tasks/robot={}  zones={}  work={}ms",
        robots, tasks_per_robot, zones_total, work_ms
    );
    let banner_visible = banner_params.chars().count();
    let banner_pad_l = (W.saturating_sub(banner_visible)) / 2;
    let banner_pad_r = W
        .saturating_sub(banner_visible)
        .saturating_sub(banner_pad_l);
    println!();
    println!("{BOLD}{CYAN}╔════════════════════════════════════════════════════════════╗{RESET}");
    println!("{BOLD}{CYAN}║              Project Blaze — Benchmark                     ║{RESET}");
    println!(
        "{BOLD}{CYAN}║{RESET}{}{CYAN}{}{RESET}{}{BOLD}{CYAN}║{RESET}",
        " ".repeat(banner_pad_l),
        banner_params,
        " ".repeat(banner_pad_r)
    );
    println!("{BOLD}{CYAN}╚════════════════════════════════════════════════════════════╝{RESET}");
    println!();
    println!("{BOLD}╔════════════════════════════════════════════════════════════╗{RESET}");
    println!("{BOLD}║                     BENCH RESULTS                          ║{RESET}");
    println!("{BOLD}╠════════════════════════════════════════════════════════════╣{RESET}");
    println!(
        "{}",
        row(
            "robots",
            &format!("{CYAN}{robots}{RESET}"),
            &robots.to_string()
        )
    );
    println!(
        "{}",
        row(
            "tasks_per_robot",
            &format!("{CYAN}{tasks_per_robot}{RESET}"),
            &tasks_per_robot.to_string()
        )
    );
    println!(
        "{}",
        row(
            "zones",
            &format!("{CYAN}{zones_total}{RESET}"),
            &zones_total.to_string()
        )
    );
    println!(
        "{}",
        row(
            "total_tasks",
            &format!("{CYAN}{}{RESET}", result.total_tasks),
            &result.total_tasks.to_string()
        )
    );
    println!(
        "{}",
        row(
            "elapsed_ms",
            &format!("{YELLOW}{:.2}{RESET}", result.elapsed_ms),
            &format!("{:.2}", result.elapsed_ms)
        )
    );
    println!(
        "{}",
        row(
            "throughput",
            &format!("{YELLOW}{:.2} tasks/s{RESET}", result.throughput),
            &format!("{:.2} tasks/s", result.throughput)
        )
    );
    println!(
        "{}",
        row(
            "avg_zone_wait_µs",
            &format!("{YELLOW}{:.2}{RESET}", result.avg_zone_wait_us),
            &format!("{:.2}", result.avg_zone_wait_us)
        )
    );
    println!(
        "{}",
        row(
            "cpu_user_s",
            &format!("{YELLOW}{cpu_user}{RESET}"),
            &cpu_user
        )
    );
    println!(
        "{}",
        row("cpu_sys_s", &format!("{YELLOW}{cpu_sys}{RESET}"), &cpu_sys)
    );
    println!(
        "{}",
        row(
            "max_occupancy",
            &format!("{YELLOW}{}{RESET}", result.max_occupancy),
            &result.max_occupancy.to_string()
        )
    );
    println!(
        "{}",
        row(
            "zone_violation",
            &viol_str,
            if result.zone_violation {
                "✗ true"
            } else {
                "✓ false"
            }
        )
    );
    println!(
        "{}",
        row(
            "duplicate_tasks",
            &dup_str,
            if result.duplicate_tasks {
                "✗ true"
            } else {
                "✓ false"
            }
        )
    );
    println!(
        "{}",
        row(
            "offline_robots",
            &format!("{YELLOW}{}{RESET}", result.offline_count),
            &result.offline_count.to_string()
        )
    );
    println!("{BOLD}╚════════════════════════════════════════════════════════════╝{RESET}");
}

/// Sweep multiple benchmark configurations and print a formatted summary table.
pub fn run_stress(
    robot_sets: Option<Vec<usize>>,
    task_sets: Option<Vec<usize>>,
    zone_sets: Option<Vec<u64>>,
    work_ms: Option<u64>,
    validate: bool,
    simulate_offline: bool,
) {
    let default_robot_sets = [1usize, 2, 4, 8, 12];
    let default_task_sets = [10usize, 25, 50];
    let default_zone_sets = [1u64, 2, 4];
    let work_ms = work_ms.unwrap_or(5);

    let robot_sets = robot_sets.unwrap_or_else(|| default_robot_sets.to_vec());
    let task_sets = task_sets.unwrap_or_else(|| default_task_sets.to_vec());
    let mut zone_sets = zone_sets.unwrap_or_else(|| default_zone_sets.to_vec());
    if robot_sets.iter().any(|&robots| robots == 0) {
        eprintln!("stress error: robot_sets must be > 0");
        return;
    }
    if task_sets.iter().any(|&tasks| tasks == 0) {
        eprintln!("stress error: task_sets must be > 0");
        return;
    }
    if zone_sets.iter().any(|&zones| zones == 0) {
        let before = zone_sets.len();
        zone_sets.retain(|&zones| zones > 0);
        let dropped = before.saturating_sub(zone_sets.len());
        if dropped > 0 {
            eprintln!("stress warning: ignored {dropped} zone set(s) <= 0");
        }
        if zone_sets.is_empty() {
            eprintln!("stress error: zones must be > 0");
            return;
        }
    }

    let robot_sets_str = robot_sets
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let task_sets_str = task_sets
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let zone_sets_str = zone_sets
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut results: Vec<BenchResult> = Vec::new();
    for robots in robot_sets {
        for tasks_per_robot in task_sets.iter().copied() {
            for zones_total in zone_sets.iter().copied() {
                let result = benchmark_once(
                    robots,
                    tasks_per_robot,
                    zones_total,
                    work_ms,
                    validate,
                    simulate_offline,
                );
                if result.leftover > 0 {
                    eprintln!("# warning,leftover_tasks,{}", result.leftover);
                }
                if validate {
                    if result.zone_violation {
                        eprintln!("# violation,zone_exclusivity");
                    }
                    if result.duplicate_tasks {
                        eprintln!("# violation,duplicate_tasks");
                    }
                }
                results.push(result);
            }
        }
    }
    print_stress_table(&results, &robot_sets_str, &task_sets_str, &zone_sets_str);
}

fn print_stress_table(
    results: &[BenchResult],
    robot_sets_str: &str,
    task_sets_str: &str,
    zone_sets_str: &str,
) {
    use crate::logging::*;
    // Column headers (short aliases)
    let headers = [
        "robots",
        "tasks/r",
        "zones",
        "total",
        "elapsed_ms",
        "tput(t/s)",
        "wait_µs",
        "max_occ",
        "violation",
        "dupes",
        "offline",
    ];
    // Build formatted cell values for each row
    let rows: Vec<[String; 11]> = results
        .iter()
        .map(|r| {
            [
                r.robots.to_string(),
                r.tasks_per_robot.to_string(),
                r.zones_total.to_string(),
                r.total_tasks.to_string(),
                format!("{:.2}", r.elapsed_ms),
                format!("{:.2}", r.throughput),
                format!("{:.2}", r.avg_zone_wait_us),
                r.max_occupancy.to_string(),
                if r.zone_violation {
                    "✗ true".to_string()
                } else {
                    "✓ false".to_string()
                },
                if r.duplicate_tasks {
                    "✗ true".to_string()
                } else {
                    "✓ false".to_string()
                },
                r.offline_count.to_string(),
            ]
        })
        .collect();
    // Compute column widths: max of header and all row values
    let col_widths: Vec<usize> = (0..11)
        .map(|i| {
            let header_w = headers[i].chars().count();
            let data_w = rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
            header_w.max(data_w)
        })
        .collect();
    // Separator line width
    let sep_width: usize = col_widths.iter().sum::<usize>() + 3 * (col_widths.len() - 1) + 4;
    let sep: String = "─".repeat(sep_width);
    // Banner
    let banner_params = format!(
        "robots={}  tasks={}  zones={}",
        robot_sets_str, task_sets_str, zone_sets_str
    );
    let banner_w = 60usize;
    let bpv = banner_params.chars().count();
    let bp_l = (banner_w.saturating_sub(bpv)) / 2;
    let bp_r = banner_w.saturating_sub(bpv).saturating_sub(bp_l);
    println!();
    println!("{BOLD}{CYAN}╔════════════════════════════════════════════════════════════╗{RESET}");
    println!("{BOLD}{CYAN}║               Project Blaze — Stress Test                  ║{RESET}");
    println!(
        "{BOLD}{CYAN}║{RESET}{}{CYAN}{}{RESET}{}{BOLD}{CYAN}║{RESET}",
        " ".repeat(bp_l),
        banner_params,
        " ".repeat(bp_r)
    );
    println!("{BOLD}{CYAN}╚════════════════════════════════════════════════════════════╝{RESET}");
    println!();
    // Header row
    let header_line: String = col_widths
        .iter()
        .enumerate()
        .map(|(i, &w)| format!("{BOLD}{:>w$}{RESET}", headers[i], w = w))
        .collect::<Vec<_>>()
        .join("   ");
    println!("  {header_line}");
    println!("  {sep}");
    // Data rows
    for row in &rows {
        let cells: Vec<String> = col_widths
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let plain = &row[i];
                let colored = match i {
                    8 => {
                        if plain.starts_with('✗') {
                            format!("{RED}{plain}{RESET}")
                        } else {
                            format!("{GREEN}{plain}{RESET}")
                        }
                    }
                    9 => {
                        if plain.starts_with('✗') {
                            format!("{RED}{plain}{RESET}")
                        } else {
                            format!("{GREEN}{plain}{RESET}")
                        }
                    }
                    _ => format!("{YELLOW}{plain}{RESET}"),
                };
                // right-align by padding before colored value
                let pad = w.saturating_sub(plain.chars().count());
                format!("{}{colored}", " ".repeat(pad))
            })
            .collect();
        println!("  {}", cells.join("   "));
    }
    // Footer
    let total_violations: usize = results.iter().filter(|r| r.zone_violation).count();
    let total_duplicates: usize = results.iter().filter(|r| r.duplicate_tasks).count();
    println!();
    println!("  {}", "─".repeat(36));
    println!(
        "  {} runs  ·  {} violations  ·  {} duplicates",
        results.len(),
        total_violations,
        total_duplicates
    );
}
