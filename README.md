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
  - Supports non-blocking and blocking task fetch
  - Provides queue shutdown behavior (`close`)
- `src/zones.rs`
  - `ZoneAccess`: `Mutex<HashMap<ZoneId, RobotId>>` + `Condvar`
  - Enforces single-owner occupancy per zone
- `src/health_monitor.rs`
  - `HealthMonitor`: `Mutex<HealthState>`
  - Tracks `last_seen` and `offline` robot sets
- `src/sim.rs`
  - Demo runner (`run_demo`)
  - Benchmark runner (`run_benchmark`)
  - Stress sweep runner (`run_stress`)
- `src/main.rs`
  - CLI parsing and argument validation
- `tests/cli_demo.rs`
  - Integration checks for grader-visible demo summary output

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
- `offline_target_detected=true`
- `offline_robots={1}`

Interpretation:

- Concurrency is active (`tasks_per_robot_done` vector covers all robots).
- Zone exclusivity holds (`zone_violation=false`).
- Offline detection is deterministic for grading (`offline_target=1` and detected).

For thread-by-thread logs (optional):

```bash
cargo run
```

Debug builds print detailed queue/zone/health transitions.

### 3) Verify benchmark CSV behavior

Standard benchmark:

```bash
cargo run --release -- bench 4 25 2 5 validate
```

Expected key columns:

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
- `tasks_per_robot_done`
- `max_zone_occupancy_observed`
- `zone_violation`
- `offline_target`
- `offline_target_detected`
- `offline_robots`

### Benchmark/Stress CSV columns

```text
robots,tasks_per_robot,zones,total_tasks,elapsed_ms,throughput_tasks_per_s,avg_zone_wait_us,cpu_user_s,cpu_sys_s,max_occupancy,zone_violation,duplicate_tasks,offline_robots
```

Platform note:

- `cpu_user_s` and `cpu_sys_s` are populated on Unix platforms.
- Non-Unix builds output `NA` in CPU columns.

## Project Layout

```text
project_blaze/
|-- Cargo.toml
|-- README.md
|-- ROADMAP.md
|-- DIAGRAMS.md
|-- project_B_guidelines.md
|-- Project-B.pdf
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

## Diagrams and Roadmap

- Architecture and flow diagrams: `DIAGRAMS.md`
- Milestones and compliance gates: `ROADMAP.md`

## Notes

- Official requirements remain the source of truth (`Project-B.pdf`, `project_B_guidelines.md`).
- Simulation timings are tuned for demonstrability and reproducibility, not realism.
