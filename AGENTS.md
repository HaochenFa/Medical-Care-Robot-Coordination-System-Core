# AGENTS.md

## Purpose and Precedence

This file is the durable instruction set for AI agents working in this repository.

Use it for:

- project scope boundaries,
- correctness and synchronization invariants,
- required quality gates,
- documentation/update expectations.

Do not treat this file as a quick-start guide or an implementation snapshot. Fast-changing operational reference belongs in `CLAUDE.md`, while user-facing explanation belongs in `README.md`.

If any conflict exists, follow this order:

1. `Project-B.pdf`
2. `project_B_guidelines.md`
3. `AGENTS.md`
4. `CLAUDE.md`
5. `README.md`

## Project Intent

Project Blaze is a lightweight OS core for coordinating medical-care robots. The goal is to demonstrate core OS concepts through a small, clear Rust implementation:

- concurrency control,
- synchronization,
- coordination.

This is a teaching project, not a production scheduler.

## Non-Negotiable Scope

Implement exactly these three components:

1. Task queue
2. Zone access control
3. Health monitor

Stay minimal. Do not expand the project into a larger OS or robotics platform.

Explicit non-goals:

- preemption,
- deadlock-prevention frameworks,
- complex scheduling policies,
- speculative work stealing,
- multi-zone routing/planning,
- distributed coordination,
- unrelated infrastructure or UI features.

## Demonstration Requirements

The runnable system must clearly show:

- multiple robots concurrently requesting tasks,
- safe shared-zone access with no overlapping occupancy,
- a robot timing out and being marked offline.

Implementation or optimization work is only acceptable if these three visible behaviors remain easy to demonstrate and explain.

## Core Invariants

These must always hold:

- each task is assigned/consumed at most once,
- each zone is occupied by at most one robot at a time,
- heartbeat timeouts can mark robots offline,
- shared mutable state is accessed only through correct synchronization,
- critical sections stay small and deadlock-free,
- correctness takes priority over performance.

When changing synchronization internals, preserve behavior before chasing speed.

## Agent Working Rules

- Prefer simple, idiomatic Rust synchronization primitives.
- Prefer narrow lock scopes and clear ownership over clever concurrency.
- Internal performance improvements are allowed only when they reduce contention or bookkeeping cost without changing project scope or public behavior.
- Keep implementation details explainable in a short demo and written report.
- Treat benchmark/stress improvements as supportive evidence, not as justification for feature creep.
- If architecture changes, update any stale technical docs that describe the current implementation.

## Required Gates

Before considering work complete, ensure:

- `cargo build --release` succeeds,
- `cargo test` passes,
- tests continue to cover:
  - task queue single-consumer / no-duplicate behavior,
  - zone exclusivity under contention,
  - heartbeat timeout / offline detection.

## Documentation Expectations

Use each document for its intended role:

- `AGENTS.md`: durable rules and repository policy for agents.
- `CLAUDE.md`: quick reference for current commands, module map, and workflow memory.
- `README.md`: human-facing project overview, usage, and verification steps.
- `DIAGRAMS.md`: architecture and behavior diagrams that match the current code.

Avoid duplicating volatile implementation detail in `AGENTS.md`. If code structure changes, update `CLAUDE.md`, `README.md`, and `DIAGRAMS.md` first, and only update `AGENTS.md` when the durable rules or project policy change.

## Decision Rule

When uncertain, prioritize:

**correctness > clarity > performance**

If a change improves performance but makes the concurrency story harder to verify, demo, or explain, reject it.
