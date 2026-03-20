//! Project Blaze CLI entry point and argument parsing.

mod health_monitor;
mod logging;
mod sim;
mod task_queue;
mod types;
mod zones;

use std::io::Write;
use std::str::FromStr;

// Parse a comma-separated list of values, or "-" to mean "skip".
fn parse_list<T: FromStr>(arg: &str) -> Option<Vec<T>> {
    if arg == "-" {
        return None;
    }
    let mut values = Vec::new();
    for part in arg.split(',') {
        if part.trim().is_empty() {
            return None;
        }
        let value = part.trim().parse::<T>().ok()?;
        values.push(value);
    }
    Some(values)
}

// Parse a comma-separated list of u64 values, or "-" to mean "skip".
fn parse_u64_list(arg: &str) -> Option<Vec<u64>> {
    parse_list(arg)
}

// Parse a comma-separated list of usize values, or "-" to mean "skip".
fn parse_usize_list(arg: &str) -> Option<Vec<usize>> {
    parse_list(arg)
}

// Emit usage text to any writer (stdout or stderr).
fn write_usage<W: Write>(out: &mut W, program: &str) {
    let _ = writeln!(out, "Project Blaze CLI");
    let _ = writeln!(out, "Usage:");
    let _ = writeln!(out, "  {program} (run demo)");
    let _ = writeln!(
        out,
        "  {program} bench [robots] [tasks_per_robot] [zones] [work_ms] [validate] [offline-demo]"
    );
    let _ = writeln!(
        out,
        "  {program} stress [robot_sets] [task_sets] [zone_sets] [work_ms] [validate] [offline-demo]"
    );
    let _ = writeln!(out, "  {program} --help");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Sets are comma-separated lists (e.g., 1,2,4). Use \"-\" to keep defaults for robot/task/zone sets."
    );
    let _ = writeln!(out, "Omit work_ms to keep its default.");
    let _ = writeln!(out, "Defaults:");
    let _ = writeln!(
        out,
        "  bench  robots=4 tasks_per_robot=25 zones=2 work_ms=5"
    );
    let _ = writeln!(
        out,
        "  stress robots=1,2,4,8,12 tasks_per_robot=10,25,50 zones=1,2,4 work_ms=5"
    );
    let _ = writeln!(out, "Flags:");
    let _ = writeln!(out, "  validate       enable extra safety checks");
    let _ = writeln!(
        out,
        "  offline-demo   simulate a robot going offline (alias: offline)"
    );
}

// Print usage to stdout (for --help).
fn print_usage_stdout(program: &str) {
    let mut out = std::io::stdout();
    write_usage(&mut out, program);
}

// Print usage to stderr (for input errors).
fn print_usage_stderr(program: &str) {
    let mut out = std::io::stderr();
    write_usage(&mut out, program);
}

// Exit with a message and usage.
fn exit_with_usage(program: &str, message: &str) -> ! {
    eprintln!("{message}");
    print_usage_stderr(program);
    std::process::exit(2);
}

struct BenchArgs {
    robots: Option<usize>,
    tasks_per_robot: Option<usize>,
    zones: Option<u64>,
    work_ms: Option<u64>,
    validate: bool,
    simulate_offline: bool,
}

struct StressArgs {
    robot_sets: Option<Vec<usize>>,
    task_sets: Option<Vec<usize>>,
    zone_sets: Option<Vec<u64>>,
    work_ms: Option<u64>,
    validate: bool,
    simulate_offline: bool,
}

fn parse_common_flag(arg: &str, validate: &mut bool, simulate_offline: &mut bool) -> bool {
    match arg {
        "validate" => {
            *validate = true;
            true
        }
        "offline" | "offline-demo" | "--offline-demo" => {
            *simulate_offline = true;
            true
        }
        _ => false,
    }
}

fn parse_bench_args(program: &str, args: impl Iterator<Item = String>) -> BenchArgs {
    let mut robots: Option<usize> = None;
    let mut tasks_per_robot: Option<usize> = None;
    let mut zones: Option<u64> = None;
    let mut work_ms: Option<u64> = None;
    let mut validate = false;
    let mut simulate_offline = false;

    for arg in args {
        if parse_common_flag(&arg, &mut validate, &mut simulate_offline) {
            continue;
        }
        if robots.is_none() {
            robots = arg.parse::<usize>().ok();
            if robots.is_none() {
                exit_with_usage(&program, &format!("bench: invalid robots value: {arg}"));
            } else if robots == Some(0) {
                exit_with_usage(&program, "bench: robots must be > 0");
            }
        } else if tasks_per_robot.is_none() {
            tasks_per_robot = arg.parse::<usize>().ok();
            if tasks_per_robot.is_none() {
                exit_with_usage(
                    &program,
                    &format!("bench: invalid tasks_per_robot value: {arg}"),
                );
            } else if tasks_per_robot == Some(0) {
                exit_with_usage(&program, "bench: tasks_per_robot must be > 0");
            }
        } else if zones.is_none() {
            zones = arg.parse::<u64>().ok();
            if zones.is_none() {
                exit_with_usage(&program, &format!("bench: invalid zones value: {arg}"));
            } else if zones == Some(0) {
                exit_with_usage(&program, "bench: zones must be > 0");
            }
        } else if work_ms.is_none() {
            work_ms = arg.parse::<u64>().ok();
            if work_ms.is_none() {
                exit_with_usage(&program, &format!("bench: invalid work_ms value: {arg}"));
            }
        } else {
            exit_with_usage(&program, &format!("bench: unexpected argument: {arg}"));
        }
    }

    BenchArgs {
        robots,
        tasks_per_robot,
        zones,
        work_ms,
        validate,
        simulate_offline,
    }
}

fn parse_stress_args(program: &str, args: impl Iterator<Item = String>) -> StressArgs {
    let mut robot_sets: Option<Vec<usize>> = None;
    let mut task_sets: Option<Vec<usize>> = None;
    let mut zone_sets: Option<Vec<u64>> = None;
    let mut work_ms: Option<u64> = None;
    let mut robot_sets_skipped = false;
    let mut task_sets_skipped = false;
    let mut zone_sets_skipped = false;
    let mut validate = false;
    let mut simulate_offline = false;

    for arg in args {
        if parse_common_flag(&arg, &mut validate, &mut simulate_offline) {
            continue;
        }

        let mut consumed = false;
        if robot_sets.is_none() && !robot_sets_skipped {
            if arg == "-" {
                robot_sets_skipped = true;
                consumed = true;
            } else if let Some(values) = parse_usize_list(&arg) {
                if values.iter().any(|&robots| robots == 0) {
                    exit_with_usage(&program, "stress: robot_sets must be > 0");
                }
                robot_sets = Some(values);
                consumed = true;
            }
            if !consumed {
                exit_with_usage(
                    &program,
                    &format!("stress: invalid robot_sets value: {arg}"),
                );
            }
            continue;
        }
        if task_sets.is_none() && !task_sets_skipped {
            if arg == "-" {
                task_sets_skipped = true;
                consumed = true;
            } else if let Some(values) = parse_usize_list(&arg) {
                if values.iter().any(|&tasks| tasks == 0) {
                    exit_with_usage(&program, "stress: task_sets must be > 0");
                }
                task_sets = Some(values);
                consumed = true;
            }
            if !consumed {
                exit_with_usage(&program, &format!("stress: invalid task_sets value: {arg}"));
            }
            continue;
        }
        if zone_sets.is_none() && !zone_sets_skipped {
            if arg == "-" {
                zone_sets_skipped = true;
                consumed = true;
            } else if let Some(values) = parse_u64_list(&arg) {
                zone_sets = Some(values);
                consumed = true;
            }
            if !consumed {
                exit_with_usage(&program, &format!("stress: invalid zone_sets value: {arg}"));
            }
            continue;
        }
        if work_ms.is_none() {
            if let Ok(value) = arg.parse::<u64>() {
                work_ms = Some(value);
            } else {
                exit_with_usage(&program, &format!("stress: invalid work_ms value: {arg}"));
            }
            continue;
        }

        exit_with_usage(&program, &format!("stress: unexpected argument: {arg}"));
    }

    if let Some(zones) = zone_sets.as_mut() {
        let before = zones.len();
        zones.retain(|&zones| zones > 0);
        let dropped = before.saturating_sub(zones.len());
        if dropped > 0 {
            eprintln!("stress warning: ignored {dropped} zone set(s) <= 0");
        }
        if zones.is_empty() {
            exit_with_usage(&program, "stress: zones must be > 0");
        }
    }

    StressArgs {
        robot_sets,
        task_sets,
        zone_sets,
        work_ms,
        validate,
        simulate_offline,
    }
}

fn main() {
    // First arg is the program name; default to a friendly fallback.
    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "project_blaze".to_string());
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("bench") => {
            // Simple positional CLI parsing for a single benchmark run.
            let bench = parse_bench_args(&program, args);
            sim::run_benchmark(
                bench.robots,
                bench.tasks_per_robot,
                bench.zones,
                bench.work_ms,
                bench.validate,
                bench.simulate_offline,
            );
        }
        Some("stress") => {
            // Parse list-based inputs and flags for a stress sweep.
            let stress = parse_stress_args(&program, args);
            sim::run_stress(
                stress.robot_sets,
                stress.task_sets,
                stress.zone_sets,
                stress.work_ms,
                stress.validate,
                stress.simulate_offline,
            );
        }
        Some("--help") | Some("-h") | Some("help") => print_usage_stdout(&program),
        Some(other) => {
            exit_with_usage(&program, &format!("unknown command: {other}"));
        }
        None => sim::run_demo(),
    }
}
