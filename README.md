# Medical Care Robot Coordination System (MCRoCS)

Lightweight OS core for coordinating medical-care robots with safe concurrency.  
This repository is scoped to the three mandatory components in Project B:

1. Task queue
2. Zone access control
3. Health monitor

The implementation prioritizes correctness and clarity over performance, consistent with project rules.

## Project B Alignment

This repository is aligned with:

- `Project-B.pdf`
- `project_B_guidelines.md`
- `AGENTS.md`

Implemented mandatory behaviors:

- Multiple robots concurrently request and consume tasks.
- Zone access is mutually exclusive (no two robots in the same zone simultaneously).
- Heartbeat timeout detection marks robots offline.

Explicit non-goals:

- No preemption
- No deadlock prevention algorithms
- No complex scheduling policies

## Core Modules

- `src/task_queue.rs`
  - `TaskQueue`: `Mutex<VecDeque<Task>>` + `Condvar`
  - Supports non-blocking (`try_pop`) and blocking (`pop_blocking_or_closed`) task fetch
  - Provides queue shutdown behavior (`close`) that unblocks all waiting consumers
- `src/zones.rs`
  - `ZoneAccess`: `Mutex<Vec<Option<RobotId>>>` + per-zone `Condvar`s
  - Uses a fixed, 1-indexed zone table sized at construction time
  - Enforces single-owner occupancy per zone; `acquire` blocks until the zone is free
- `src/health_monitor.rs`
  - `HealthMonitor`: `Mutex<HealthState>` where `HealthState` stores both heartbeat timestamps and the offline set
  - Keeps heartbeat updates and offline detection in one coherent critical section
- `src/sim.rs`
  - Demo runner (`run_demo`): 3 robots, 2 zones, deterministic offline target (robot 1)
  - Benchmark runner (`run_benchmark`): single parameterized run, boxed summary output
  - Stress sweep runner (`run_stress`): iterates robot/task/zone sets, aligned table output
  - Benchmark validation uses a fixed seen-table keyed by task id instead of a channel
  - The background health-monitor thread is only spawned for `offline-demo` benchmark/stress runs
- `src/main.rs`
  - CLI entry point; subcommands: _(no args)_ demo, `bench`, `stress`, `--help`
- `src/logging.rs`
  - `log_dev!` macro: debug-only structured logs (no-op in release builds)
- `src/types.rs`
  - Shared type aliases: `TaskId`, `RobotId`, `ZoneId`; `Task` struct
- `tests/cli_demo.rs`
  - Integration tests: verify demo summary fields, zone safety, and offline detection

## Concurrency and Safety Invariants

- A task is consumed at most once.
- A zone is occupied by at most one robot at a time.
- Offline robots are detected by heartbeat timeout.
- Shared mutable state is protected by synchronization primitives.
- Lock scopes are short, with no nested lock cycles in core logic.

## Build and Test Gates

Required project gates:

```bash
cargo build --release
cargo test
```

Additional recommended verification:

```bash
cargo test --release
```

## CLI Usage

```bash
cargo run --release -- --help
```

Usage summary:

- `project_blaze` (no subcommand): run demo
- `project_blaze bench [robots] [tasks_per_robot] [zones] [work_ms] [validate] [offline-demo]`
- `project_blaze stress [robot_sets] [task_sets] [zone_sets] [work_ms] [validate] [offline-demo]`

Argument notes:

- `robot_sets`, `task_sets`, `zone_sets` are comma-separated lists, for example `1,2,4`.
- Use `-` to keep default sets in stress mode.
- `validate` enables extra runtime safety checks in benchmark/stress output.
- `offline-demo`, `--offline-demo`, and `offline` are equivalent flag aliases.

Defaults:

- bench: `robots=4 tasks_per_robot=25 zones=2 work_ms=5`
- stress: `robots=1,2,4,8,12 tasks_per_robot=10,25,50 zones=1,2,4 work_ms=5`

## Grader Verification Guide

This section is intentionally step-by-step so graders can verify required behaviors quickly.

### 1) Compile and run tests

```bash
cargo build --release
cargo test
```

Expected:

- Build succeeds.
- All unit and integration tests pass.

### 2) Verify required demo behaviors

Run demo in release mode:

```bash
cargo run --release
```

Expected summary fields:

- `DEMO SUMMARY`
- `zone_violation=false`
- `offline_target=1`
- `detected=true`
- `offline_robots={1}`

Interpretation:

- Concurrency is active (`per_robot_done` vector covers all robots).
- Zone exclusivity holds (`zone_violation=false`).
- Offline detection is deterministic for grading (`offline_target=1` and detected).

For thread-by-thread logs (optional):

```bash
cargo run
```

Debug builds print detailed queue/zone/health transitions.

### 3) Verify benchmark summary output

Standard benchmark:

```bash
cargo run --release -- bench 4 25 2 5 validate
```

Expected key rows:

- `zone_violation=false`
- `duplicate_tasks=false`

Offline benchmark:

```bash
cargo run --release -- bench 4 50 2 20 validate --offline-demo
```

Expected:

- `offline_robots >= 1`
- `zone_violation=false`
- `duplicate_tasks=false`

### 4) Verify stress sweep behavior

Standard stress sweep:

```bash
cargo run --release -- stress 1,2,4 10,25 1,2 5 validate
```

Offline stress sweep:

```bash
cargo run --release -- stress 1,2,4 10,25 1,2 5 validate --offline-demo
```

Expected across rows:

- `zone_violation=false`
- `duplicate_tasks=false`
- In offline mode: `offline_robots >= 1` is acceptable

Important semantics:

- Demo mode uses deterministic offline target verification.
- Benchmark/stress offline mode validates timeout behavior under workloads and may mark multiple robots offline by the end of a run.

## Output Reference

### Demo summary fields

- `robots`
- `tasks_total`
- `per_robot_done`
- `max_zone_occ`
- `zone_violation`
- `offline_target`
- `detected`
- `offline_robots`

### Benchmark/Stress output fields

The benchmark command prints a boxed summary with these fields:

- `robots`
- `tasks_per_robot`
- `zones`
- `total_tasks`
- `elapsed_ms`
- `throughput`
- `avg_zone_wait_µs`
- `cpu_user_s`
- `cpu_sys_s`
- `max_occupancy`
- `zone_violation`
- `duplicate_tasks`
- `offline_robots`

The stress command prints the same metrics as aligned table columns using the shorter labels shown in the CLI (`tasks/r`, `tput(t/s)`, `wait_µs`, and so on).

Platform note:

- `cpu_user_s` and `cpu_sys_s` are populated on Unix platforms.
- Non-Unix builds output `NA` in CPU columns.

## Project Layout

```text
project_blaze/
|-- Cargo.toml
|-- README.md
|-- DIAGRAMS.md
|-- CLAUDE.md
|-- AGENTS.md
|-- project_B_guidelines.md
|-- Project-B.pdf
|-- written_report_draft.tex
|-- src/
|   |-- main.rs
|   |-- sim.rs
|   |-- task_queue.rs
|   |-- zones.rs
|   |-- health_monitor.rs
|   |-- logging.rs
|   `-- types.rs
`-- tests/
    `-- cli_demo.rs
```

## Diagrams

- Architecture and flow diagrams: `DIAGRAMS.md`

## Notes

- Official requirements remain the source of truth (`Project-B.pdf`, `project_B_guidelines.md`).
- Simulation timings are tuned for demonstrability and reproducibility, not realism.
