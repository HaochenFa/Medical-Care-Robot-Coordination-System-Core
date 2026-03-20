# Project Blaze — Video Demonstration Guide

**Total duration: 3 minutes**
**Format: Screen recording with clear audio narration, submitted as MP4 or similar common format**
**Technical requirement: Keep the file under 50 MB, or use an unlisted YouTube link if needed**

This guide is aligned with the current repository and the Project B requirements in `Project-B.pdf` and `project_B_guidelines.md`.

The video should stay focused on exactly these three required components:

1. Task queue
2. Zone access control
3. Health monitor

The grader must be able to see these three required behaviors clearly:

- multiple robots concurrently requesting tasks,
- safe shared-zone access with no overlapping occupancy,
- a robot timing out and being marked offline.

Do not spend time on non-goals such as advanced scheduling policies, deadlock-prevention frameworks, or broader robotics-platform features.

---

## Pre-Recording Preparation

Run these once before recording so the video stays smooth:

```bash
cargo build --release
cargo test
```

Recording setup:

- Use `cargo run` for the first live demo section because debug builds show the detailed `[QUEUE]`, `[ZONE]`, and `[HEALTH]` logs.
- Use release mode for benchmark and stress sections because the output is cleaner and easier to read on screen.
- Increase terminal font size before recording.
- Keep all commands ready to paste.
- If you want a cleaner recording, run each command once before the final take so compilation does not dominate the video.

Important note:

- Demo timings vary slightly from run to run.
- For benchmark and stress, focus on the field names and correctness indicators, not exact numeric timings.

---

## Section 1 — System Overview (0:00 – 0:30)

### Goal

Give the grader a quick mental model of the system and state exactly what the video will prove.

### What to show on screen

- the project root in the terminal,
- optionally `ls src`,
- optionally `DIAGRAMS.md` or a simple architecture sketch.

### What to say

> "Project Blaze is a lightweight Rust coordination layer for medical-care robots. It is intentionally scoped to the three components required in Project B: a task queue, a zone access controller, and a health monitor."

> "The task queue uses a `Mutex` and `Condvar` to hand out tasks safely. The zone controller uses a mutex-protected zone table with one condition variable per zone to enforce mutual exclusion. The health monitor tracks robot heartbeats and records robots as offline when they miss the timeout."

> "In this video, I will show all three required OS behaviors: multiple robots requesting tasks concurrently, safe shared-zone access with no overlap, and deterministic offline detection when one robot stops heartbeats."

### What to point at

- `src/task_queue.rs`
- `src/zones.rs`
- `src/health_monitor.rs`

### Why this matters

This opening tells the grader that the system is minimal, on-scope, and directly aligned with the assignment rubric.

---

## Section 2 — Live Demonstration (0:30 – 2:30)

### Goal

Show the required behaviors in action, then reinforce them with structured benchmark and stress output.

### Live demo flow

1. `cargo run` for visible thread interleaving and offline behavior.
2. `cargo run --release -- bench 4 25 2 5 validate offline-demo` for one clean benchmark summary.
3. `cargo run --release -- stress 1,2,4 10 1,2 5 validate` for a compact scalability sweep.

### Short transition line

> "I’ll first show the live thread-level behavior, then I’ll use the benchmark and stress modes as extra evidence that the same correctness properties still hold under heavier workloads."

---

### Sub-section 2a — Demo Run: Required Behaviors Live (0:30 – 1:20)

### Command to run

```bash
cargo run
```

### What this run does

This launches the built-in demo in `src/sim.rs`:

- 3 robots,
- 9 total tasks,
- 2 zones,
- robot `1` as the fixed offline target.

### What to show on screen

Let the full demo run in the terminal. Focus on:

- the interleaved `[robot-0]`, `[robot-1]`, and `[robot-2]` log lines,
- the `entered zone` and `left zone` messages,
- the `robot-1 stops heartbeats` message,
- the final `DEMO SUMMARY` box.

### Output highlights to point at

Representative lines from the current repo look like this:

```text
[QUEUE]  robot-0 fetched task 0
[QUEUE]  robot-1 fetched task 1
[QUEUE]  robot-2 fetched task 2
[ZONE]   robot-0 entered zone 1 for task 0
[ZONE]   robot-0 left zone 1 for task 0
[ZONE]   robot-2 entered zone 1 for task 2
[HEALTH] robot-1 stops heartbeats
```

The stable summary fields to call out are:

```text
DEMO SUMMARY
robots
tasks_total
per_robot_done
max_zone_occ
zone_violation
offline_target
detected
offline_robots
```

On the current implementation, the important end-state is:

- `per_robot_done [3, 3, 3]`
- `zone_violation false`
- `offline_target 1`
- `detected true`
- `offline_robots {1}`

### What to say

> "At the top, the three robot threads begin fetching tasks at nearly the same time. The interleaved queue logs show real concurrency, and each task ID appears once because the task queue hands out tasks under a single mutex."

> "For zone access, watch the order of the zone messages. When two tasks map to the same zone, one robot enters first and the next robot does not enter that same zone until after the first robot leaves. That is the mutual-exclusion guarantee required by Project B."

> "For health monitoring, robot 1 is the fixed offline target in this demo. After it stops sending heartbeats, the monitor eventually marks it offline. The summary confirms this with `offline_target 1`, `detected true`, and `offline_robots {1}`."

> "The most important correctness field here is `zone_violation false`. `max_zone_occ` can be 2 in this demo because two different zones may be occupied at the same time. That is safe concurrency, not a bug."

### Why this matters

This is the clearest on-screen proof of concurrency, synchronization, and coordination in one short run.

---

### Sub-section 2b — Benchmark Run: Clean Correctness Summary (1:20 – 1:50)

### Command to run

```bash
cargo run --release -- bench 4 25 2 5 validate offline-demo
```

### What this run does

This runs one heavier workload:

- 4 robots,
- 25 tasks per robot,
- 2 zones,
- 5 ms simulated work per task,
- runtime validation enabled,
- offline simulation enabled.

### What to show on screen

Keep the terminal centered on the `BENCH RESULTS` box.

### What to point at

The current benchmark output is a boxed summary, not CSV. The key fields are:

- `zone_violation`
- `duplicate_tasks`
- `offline_robots`
- `avg_zone_wait_µs`
- `throughput`

### What to say

> "This benchmark keeps the same coordination logic but runs a larger workload and reports the results in a structured summary. The two most important correctness fields are `zone_violation` and `duplicate_tasks`, and both should remain false."

> "That means no two robots were observed in the same zone at the same time, and no task was consumed twice. Those are runtime-checked invariants, not just visual guesses from the logs."

> "The benchmark also reports waiting time and throughput. I use those numbers only as supporting evidence to show contention cost under synchronization, while correctness still remains the priority."

> "Because this run includes `offline-demo`, the summary also shows that offline detection still works correctly under a heavier workload."

### Why this matters

This section shows that the same safety guarantees still hold when the system handles more work than the small demo.

---

### Sub-section 2c — Stress Sweep: Correctness Across Configurations (1:50 – 2:30)

### Command to run

```bash
cargo run --release -- stress 1,2,4 10 1,2 5 validate
```

### What this run does

This sweeps across multiple configurations:

- robots: `1,2,4`
- tasks per robot: `10`
- zones: `1,2`
- work per task: `5 ms`
- validation enabled

### What to show on screen

Show the whole table, then point to:

- the `violation` column,
- the `dupes` column,
- the `wait_µs` column,
- the final summary line at the bottom.

### What to say

> "Stress mode runs several configurations back to back and prints an aligned table. The columns I want to focus on are `violation` and `dupes`, which should stay false across every row."

> "The `wait_µs` column helps explain contention. With fewer zones and more robots, waiting increases because more threads compete for the same shared resource. With more zones, contention drops and the system moves more smoothly."

> "The important result is that correctness does not change across configurations. Even as the workload changes, the synchronization logic still prevents duplicate task assignment and overlapping zone occupancy."

> "The final line summarizing zero violations and zero duplicates reinforces that the design remains correct across the whole sweep."

### Why this matters

This section demonstrates that the project is not only correct for one hand-picked demo run, but also stable across a small range of workloads.

---

## Section 3 — Code Walkthrough (2:30 – 3:00)

### Goal

Show that you understand the synchronization logic inside the implementation, not just the terminal output.

### What to show on screen

- your editor,
- one source file already open,
- your cursor highlighting the key lines as you speak.

### What to say before the walkthrough

> "To close the video, I’ll show one synchronization decision directly in the code. The project uses simple Rust primitives and keeps correctness and clarity ahead of performance tricks."

### Recommendation

Pick **one** of the three options below for the recording itself. All three are valid, but only one needs to appear in the final video.

---

### Option A — Task Queue Walkthrough

Open `src/task_queue.rs` and go to `pop_blocking_or_closed` around lines `55-66`.

Highlight:

```rust
pub fn pop_blocking_or_closed(&self) -> Option<Task> {
    let mut guard = self.inner.lock().expect("task queue mutex poisoned");
    loop {
        if let Some(task) = guard.queue.pop_front() {
            return Some(task);
        }
        if guard.closed {
            return None;
        }
        // Wait releases the lock and re-acquires it before returning.
        guard = self.available.wait(guard).expect("condvar wait failed");
    }
}
```

### What to say

> "This is the core of the task queue. A robot locks the queue, tries to pop a task, and if the queue is empty it waits on the condition variable instead of spinning in a busy loop."

> "The `loop` is important because a condition variable can wake up spuriously. The correct pattern is lock, check, wait, and then re-check after waking."

> "This design is simple and safe: the queue state is protected by one mutex, and each task is removed exactly once from the `VecDeque`."

### Key design decision

If you mention `push`, explain:

> "When a producer adds one new task, it uses `notify_one` because only one waiting robot should wake up and consume that task."

---

### Option B — Zone Access Walkthrough

Open `src/zones.rs` and highlight `acquire` around lines `32-42`, then `release` around lines `46-55`.

Highlight:

```rust
pub fn acquire(&self, zone: ZoneId, robot: RobotId) {
    let idx = self.zone_index(zone);
    let mut guard = self.occupied.lock().expect("zone mutex poisoned");
    loop {
        if guard[idx].is_none() {
            guard[idx] = Some(robot);
            return;
        }
        // Wait releases the lock; on wake, re-check the condition.
        guard = self.condvars[idx].wait(guard).expect("condvar wait failed");
    }
}
```

```rust
pub fn release(&self, zone: ZoneId, robot: RobotId) -> bool {
    let idx = self.zone_index(zone);
    let mut guard = self.occupied.lock().expect("zone mutex poisoned");
    match guard[idx] {
        Some(owner) if owner == robot => {
            guard[idx] = None;
            drop(guard);
            self.condvars[idx].notify_one();
            true
        }
        ...
    }
}
```

### What to say

> "The zone controller stores ownership in a fixed vector where each zone has its own slot. A zone is free when its slot is `None`, and occupied when it stores a robot ID."

> "If a robot requests a busy zone, it sleeps on that zone’s own condition variable. When the current owner releases the zone, exactly one waiting contender is woken up for that same zone."

> "This is why the project can enforce mutual exclusion while still allowing safe concurrency across different zones."

### Key design decision

> "The important detail is that each zone has its own condition variable. Because waiting is already split per zone, `notify_one` is enough here and avoids waking unrelated waiters."

---

### Option C — Health Monitor Walkthrough

Open `src/health_monitor.rs` and highlight `heartbeat` around lines `40-47`, then the offline refresh logic around lines `60-84`.

Highlight:

```rust
pub fn heartbeat(&self, robot: RobotId) {
    let mut guard = self
        .inner
        .lock()
        .expect("health monitor mutex poisoned");
    guard.heartbeats.insert(robot, Instant::now());
    guard.offline.remove(&robot);
}
```

```rust
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
```

### What to say

> "The health monitor keeps both the heartbeat timestamps and the offline set inside one mutex-protected state. That keeps the update rules coherent."

> "When a robot sends a heartbeat, the code refreshes its timestamp and removes any stale offline mark in the same critical section."

> "When offline detection runs, it scans for overdue heartbeat timestamps and then marks those robots offline. Keeping the related state together makes the logic easier to reason about and avoids cross-lock coordination problems."

### Key design decision

> "The main design choice here is to keep `heartbeats` and `offline` under one lock so heartbeat updates and offline detection cannot drift out of sync."

---

## Closing Line (last 5 seconds)

> "This demo shows the three required OS concepts in Project Blaze: concurrent task handling, synchronized zone access, and reliable offline detection. Thank you."

---

## Timing Summary

| Section | Time | Duration |
|---|---|---|
| System Overview | 0:00 - 0:30 | 30 seconds |
| Demo Run | 0:30 - 1:20 | 50 seconds |
| Benchmark Run | 1:20 - 1:50 | 30 seconds |
| Stress Sweep | 1:50 - 2:30 | 40 seconds |
| Code Walkthrough | 2:30 - 3:00 | 30 seconds |
| **Total** | | **3:00** |

---

## Recording Checklist

- [ ] `cargo build --release` succeeds before recording
- [ ] `cargo test` passes before recording
- [ ] terminal font is large enough to read clearly
- [ ] audio is clear with no background noise
- [ ] `cargo run` is used for the live demo section with visible debug logs
- [ ] release mode is used for benchmark and stress sections
- [ ] `zone_violation false` is explicitly pointed out
- [ ] `offline_target 1`, `detected true`, and `offline_robots {1}` are explicitly pointed out in the demo summary
- [ ] `duplicate_tasks false` is explicitly pointed out in the benchmark section
- [ ] the stress table’s `violation` and `dupes` columns are explicitly pointed out
- [ ] the code walkthrough shows actual source code from the current repo
- [ ] the final video stays within 3 minutes
- [ ] the final file is under 50 MB, or submitted as an unlisted YouTube link if needed
