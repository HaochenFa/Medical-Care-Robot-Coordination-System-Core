# ROADMAP.md

This roadmap is aligned with `Project-B.pdf`, `project_B_guidelines.md`, and `AGENTS.md`.

## Status Snapshot

- Core scope implementation: complete
- Required build/test gates: complete
- Deterministic demo evidence path: complete
- Documentation for grading workflow: complete
- Optional hardening and polish: open

## Milestone 0: Requirements Baseline (Complete)

Goals:

- Lock the minimal scope (task queue, zone access control, health monitor).
- Record non-goals (no preemption, deadlock prevention, complex scheduling).
- Define invariants and demo success criteria.

Exit criteria met:

- Requirements and invariants are captured in project docs.

## Milestone 1: Architecture and Interfaces (Complete)

Goals:

- Define task, robot, and zone identifiers and task model.
- Separate core modules for queue, zones, and health monitoring.
- Keep ownership boundaries clear and synchronization explicit.

Exit criteria met:

- Readable module structure in `src/`.
- Architecture represented in `DIAGRAMS.md`.

## Milestone 2: Core Implementation (Complete)

Goals:

- Implement synchronized task queue with safe blocking behavior.
- Implement strict mutual exclusion for zone occupancy.
- Implement timeout-based offline detection for robot heartbeats.
- Implement simulation runners for demo/benchmark/stress.

Exit criteria met:

- All three mandatory components are implemented and integrated.

## Milestone 3: Correctness Tests (Complete)

Goals:

- Unit tests for task single-consumer semantics and queue shutdown behavior.
- Concurrency test for zone exclusivity under contention.
- Health monitor timeout and offline-clearing tests.
- Integration test for grader-visible demo summary outputs.

Exit criteria met:

- `cargo test` passes.
- Demo integration tests validate deterministic offline target reporting.

## Milestone 4: Observability and Benchmarking (Complete)

Goals:

- Emit demo summary fields for grading visibility.
- Emit benchmark/stress CSV for throughput, latency, CPU, and safety flags.
- Support validation mode for runtime integrity checks.

Exit criteria met:

- Demo/bench/stress commands run with clear, parseable output.
- Safety flags (`zone_violation`, `duplicate_tasks`) are surfaced in CSV.

## Milestone 5: Demo Readiness (Complete)

Goals:

- Ensure required behaviors are observable in a short runtime.
- Make offline demonstration deterministic for grading confidence.

Exit criteria met:

- Demo summary includes:
  - `zone_violation=false`
  - `offline_target=1`
  - `offline_target_detected=true`
  - `offline_robots={1}`

## Milestone 6: Report Readiness (In Progress)

Goals:

- Keep report structure aligned with required sections and constraints.
- Maintain benchmark narrative consistent with current CLI behavior.

Current state:

- Report draft exists in `written_report_draft.tex`.
- Section length thresholds are currently satisfied.
- Reference list meets the minimum count requirement.

## Quality Gates (Always On)

- Build: `cargo build --release` succeeds.
- Tests: `cargo test` succeeds.
- Demo: required three behaviors are visible.
- Synchronization correctness: no race-indicative violations in provided tests/runs.

## Optional Next Improvements

- Add benchmark/stress mode tests that distinguish intentional offline targets from natural post-work timeouts.
- Export example benchmark/stress outputs as versioned artifacts for report reproducibility.
- Add lightweight CI workflow to run build + tests on push.
