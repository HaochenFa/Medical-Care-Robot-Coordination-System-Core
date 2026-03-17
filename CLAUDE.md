# CLAUDE.md — Project Blaze

## Quick Start

```bash
cargo build --release          # verify build passes
cargo run                      # run built-in deterministic demo (default mode)
cargo test                     # unit tests
cargo test --test cli_demo     # integration test (grader-visible output)
```

## CLI Usage

```
cargo run -- [robots] [zones] [tasks] [offline_robot_ids] [heartbeat_ms] [timeout_ms]
# Use "-" for any arg to accept defaults.
# Example (custom): cargo run -- 4 3 10 2 200 800
# Example (default demo): cargo run
```

## Module Map

| File | Role |
|------|------|
| `src/main.rs` | CLI entry point, arg parsing |
| `src/sim.rs` | `run_demo`, `run_benchmark`, `run_stress` orchestrators |
| `src/task_queue.rs` | `TaskQueue`: `Mutex<VecDeque<Task>>` + `Condvar` |
| `src/zones.rs` | `ZoneAccess`: `Mutex<HashMap<ZoneId,RobotId>>` + `Condvar` |
| `src/health_monitor.rs` | `HealthMonitor`: heartbeat tracking, offline detection |
| `src/types.rs` | Shared types (`Task`, `RobotId`, `ZoneId`, …) |
| `src/logging.rs` | Structured log helpers |
| `tests/cli_demo.rs` | Integration tests against binary stdout |

## Purpose and Authority

This file defines strict, durable working rules aligned with `Project-B.pdf` and `project_B_guidelines.md`. If any conflict exists, those official documents are the source of truth and must be followed.

## Project Summary (Project Blaze)

Build a lightweight OS core that coordinates multiple medical-care robots safely and efficiently. The emphasis is on core OS concurrency concepts: concurrency control, synchronization, and coordination.

## Minimal Scope (Mandatory)

Implement exactly these three components:

1. **Task queue**: store incoming tasks and allow robots to fetch tasks safely.
2. **Zone access control**: prevent two robots from occupying the same zone at the same time.
3. **Health monitor**: track robot heartbeats and mark missing robots as offline.

Keep the design minimal. Do not implement preemption, deadlock prevention, or complex scheduling policies.

## Demonstration Requirements (Mandatory)

The demo must clearly show all three behaviors:

- Multiple robots concurrently requesting tasks.
- Safe access to shared zones (no two robots in the same zone).
- A robot timing out and being marked offline.

## Core Concepts to Demonstrate (Mandatory)

- **Concurrency control**: safe access to shared state with threads.
- **Synchronization**: preventing race conditions and inconsistent state.
- **Coordination**: organizing multiple worker threads with clear ownership.

## Safety and Correctness Invariants (Must Always Hold)

- A task is assigned/consumed at most once.
- A zone is occupied by at most one robot at any time.
- Offline robots are detected when heartbeat timeouts occur.
- Shared state is accessed only under correct synchronization.
- Critical sections are minimal and deadlock-free (consistent lock ordering).

## Implementation Rules (Strict)

- Use safe Rust synchronization primitives (e.g., `Mutex`, `RwLock`, `Condvar`, channels).
- Avoid unnecessary shared state; prefer clear ownership and narrow lock scopes.
- Keep module structure readable and idiomatic.
- Provide observable behavior (logs or outputs) for demo and debugging.

## Required Build and Test Gates

- `cargo build --release` must succeed.
- `cargo test` must pass with meaningful coverage for:
  - task queue safety and single-consumer behavior,
  - zone exclusivity under concurrent access,
  - heartbeat timeout/offline detection.

## Deliverables Awareness (Non-Code)

- Written report must follow the required structure and word counts.
- 3-minute video demo must show concurrency, synchronization, and safe coordination.
- Maintain a reasonable commit history showing progress.

## Decision Rule

When uncertain, prioritize: **correctness > clarity > performance**. Always align with the official requirements.
