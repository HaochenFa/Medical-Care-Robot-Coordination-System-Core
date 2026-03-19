# Project Blaze — Video Demonstration Guide

**Total duration: 3 minutes (strictly enforced)**
**Format: Screen recording with clear audio narration, submitted as MP4 (<50 MB)**

---

## Section 1 — System Overview (0:00 – 0:30)

### Goal

Give the grader a mental model of your system before they see it run. This is your 30-second pitch.

### What to show on screen

- Your terminal, with the project directory open (e.g., `ls src/`)
- Optionally: a simple architecture diagram (can be hand-drawn or ASCII) showing the three components and how robot threads interact with them

### What to say (narrate all of this)

1. **Introduce the system** (2-3 sentences):
   > "Project Blaze is a lightweight OS coordination layer for autonomous hospital robots, written in Rust. It implements three core components: a thread-safe task queue, a zone access controller, and a health monitor."

2. **Name each module and its role** (one sentence each):
   - `task_queue.rs` — a `Mutex<VecDeque<Task>>` with a `Condvar` that lets robot threads safely fetch tasks without races
   - `zones.rs` — a `Mutex<HashMap<ZoneId, RobotId>>` with a `Condvar` that enforces mutual exclusion on hospital zones
   - `health_monitor.rs` — tracks per-robot heartbeat timestamps and marks a robot offline when its deadline is missed

3. **State what the demo will prove**:
   > "In the next two minutes, you will see all three behaviors live: concurrent task fetching, zone exclusivity, and offline detection — first in a qualitative demo, then verified quantitatively with benchmark and stress runs."

### Tips

- Keep this tight. Do not go over 30 seconds.
- You do not need slides. A clean terminal window is fine.

---

## Section 2 — Live Demonstration (0:30 – 2:30)

### Goal

Prove all three required behaviors across three progressively more demanding runs: the default demo (qualitative, log-level visibility), a benchmark run (quantitative correctness metrics), and a stress sweep (scalability across configurations). Narrate each run to connect the output to the OS concepts being demonstrated.

### Preparation (before recording)

- Use `cargo run` (debug build) for the demo — this enables the structured `[TAG]` log lines. Release mode suppresses them.
- Have all three commands ready to paste so you don't waste time typing.
- Increase your terminal font size so log lines are readable on screen.
- The demo log format is: `[{unix_timestamp_ms}][{thread-name}] [TAG] message`

---

### Sub-section 2a — Default Demo: All Three Behaviors (0:30 – 1:20, ~50 seconds)

**Command to run:**

```bash
cargo run
```

This launches the default demo: 3 robots, 9 tasks total (3 per robot), 2 zones, with robot-1 as the designated offline target.

**Full expected output:**

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
 Running `target/debug/project_blaze`
[...ms][main] [DEMO] start
[...ms][main] [QUEUE] loaded tasks total=9 per_robot=3
[...ms][robot-0] [QUEUE] robot-0 fetched task 0
[...ms][robot-0] [ZONE] robot-0 entered zone 1 for task 0
[...ms][robot-2] [QUEUE] robot-2 fetched task 2
[...ms][robot-1] [QUEUE] robot-1 fetched task 1
[...ms][robot-1] [ZONE] robot-1 entered zone 2 for task 1
[...ms][robot-0] [ZONE] robot-0 left zone 1 for task 0
[...ms][robot-2] [ZONE] robot-2 entered zone 1 for task 2    <- robot-2 was waiting; enters only after robot-0 left
[...ms][robot-0] [HEALTH] robot-0 heartbeat
[...ms][robot-0] [QUEUE] robot-0 fetched task 3
...
[...ms][robot-1] [HEALTH] robot-1 stops heartbeats           <- robot-1 goes silent after task 2
...
[...ms][health-monitor] [HEALTH] robot 1 marked offline      <- background thread detects timeout ~200ms later
...
[...ms][main] [HEALTH] offline robots at end = 1
[...ms][main] [DEMO] finished in 649ms (dev logs suppressed in release mode)
DEMO SUMMARY
robots=3 tasks_total=9
tasks_per_robot_done=[3, 3, 3]
max_zone_occupancy_observed=2
zone_violation=false
offline_target=1
offline_target_detected=true
offline_robots={1}
```

**What to narrate — point to each section as it appears:**

*On task fetching (first few lines):*
> "Three robot threads start simultaneously and immediately race for tasks. Robot-0, robot-1, and robot-2 all fetch their first tasks within the same millisecond — that's true concurrency. Each task ID appears exactly once across all robots, because the `Mutex` inside `TaskQueue` ensures only one thread can pop from the `VecDeque` at a time. No task is ever assigned twice."

*On zone contention (the `entered`/`left` sequence):*
> "Zone assignment is deterministic: task ID modulo number of zones. Tasks 0 and 2 both map to zone 1, so robot-0 and robot-2 compete. Robot-0 enters first. Robot-2 does not appear as 'entered' until after robot-0 logs 'left' — that gap is robot-2 blocked inside `ZoneAccess::acquire`, sleeping on the `Condvar`. The moment robot-0 releases and calls `notify_all`, robot-2 wakes, re-checks the map, finds the zone free, and enters."

*On offline detection:*
> "Robot-1 is the designated offline target. It sends heartbeats for its first two tasks, then stops — simulating a crash. The `health-monitor` background thread polls every 50ms. Once the gap since robot-1's last heartbeat exceeds 200ms, it calls `detect_offline`, moves robot-1 into the offline set, and logs it. The other robots are completely unaffected."

*On the summary block:*
> "The summary is a machine-readable assertion. `zone_violation=false` means the per-zone occupancy counter never exceeded 1 — tracked atomically at runtime, not just visually. `offline_target_detected=true` confirms the monitor caught it. All three invariants hold."

**Key points to make explicit:**
- Every task ID appears in the log exactly once, assigned to exactly one robot — proving single-consumer correctness.
- The `entered` log for a contending robot always appears after the `left` log of the current occupant — proving mutual exclusion.
- The offline detection is driven by a dedicated background thread (`health-monitor`), completely decoupled from the robot worker threads.

---

### Sub-section 2b — Benchmark: Quantitative Correctness and Performance (1:20 – 1:50, ~30 seconds)

**Command to run:**

```bash
cargo run -- bench 4 25 2 5 validate offline-demo
```

This runs a single benchmark: 4 robots, 25 tasks each (100 total), 2 zones, 5ms simulated work per task, with safety validation and offline simulation enabled.

**Expected output:**

```
robots,tasks_per_robot,zones,total_tasks,elapsed_ms,throughput_tasks_per_s,avg_zone_wait_us,cpu_user_s,cpu_sys_s,max_occupancy,zone_violation,duplicate_tasks,offline_robots
4,25,2,100,734.00,136.24,5547.04,0.0012,0.0021,2,false,false,1
```

**What to narrate:**
> "The benchmark mode runs a heavier workload — 100 tasks across 4 robots — and outputs structured CSV metrics. The key correctness columns are `zone_violation=false` and `duplicate_tasks=false`: these are runtime-checked invariants, not just observations. `avg_zone_wait_us` shows the average time a robot spent blocked waiting for a zone — here about 5.5ms — which is the measurable cost of mutual exclusion under contention. `offline_robots=1` confirms the health monitor still works correctly under load. `throughput_tasks_per_s` gives us a performance baseline to compare against the stress sweep."

**Key points to make explicit:**
- `zone_violation` and `duplicate_tasks` are computed by atomic counters inside the simulation, not inferred from logs — they are hard correctness guarantees.
- The benchmark proves the system is correct not just in the minimal demo, but under a realistic workload.

---

### Sub-section 2c — Stress Sweep: Scalability Across Configurations (1:50 – 2:30, ~40 seconds)

**Command to run:**

```bash
cargo run -- stress 1,2,4 10 1,2 5 validate
```

This sweeps across robot counts (1, 2, 4), a fixed task count (10 per robot), and zone counts (1, 2), producing one CSV row per configuration combination.

**Expected output:**

```
robots,tasks_per_robot,zones,total_tasks,elapsed_ms,throughput_tasks_per_s,avg_zone_wait_us,...,zone_violation,duplicate_tasks,offline_robots
1,10,1,10,101.00, 99.01,   0.00,...,false,false,0
1,10,2,10,105.00, 95.24,   0.20,...,false,false,0
2,10,1,20,105.00,190.48,4398.80,...,false,false,0
2,10,2,20,104.00,192.31,   0.80,...,false,false,0
4,10,1,40,210.00,190.48,8900.00,...,false,false,0
4,10,2,40,105.00,380.95,1200.00,...,false,false,0
```

**What to narrate:**
> "The stress mode sweeps all combinations automatically. Look at the `avg_zone_wait_us` column — with 1 zone and 4 robots, contention is high and wait time spikes to nearly 9ms, because all robots funnel through a single zone. With 2 zones, contention drops and throughput nearly doubles. This directly demonstrates the scalability trade-off between resource availability and synchronization overhead."

Then point to the correctness columns across all rows:
> "Critically, `zone_violation=false` and `duplicate_tasks=false` hold across every single configuration — 1 robot or 4, 1 zone or 2. The synchronization primitives are correct regardless of scale. That's the core claim of this project."

**Key points to make explicit:**
- Correctness invariants hold across all configurations — this is not a lucky result for one specific parameter set.
- The `avg_zone_wait_us` trend directly illustrates the cost of contention and the benefit of more zones.

---

## Section 3 — Code Walkthrough (2:30 – 3:00)

### Goal

Show the grader that you understand the critical synchronization code, not just that it runs. Pick **one** of the three options below. Open the file in your editor before recording so you can navigate to it instantly. Highlight the relevant lines with your cursor as you speak.

### What to show on screen

- Your editor with the source file open, scrolled to the highlighted snippet
- Move your cursor over the key lines as you narrate each point

### Recommended focus: pick ONE of the following

---

**Option A — Task Queue (`src/task_queue.rs`, lines 55-67)**

Open `src/task_queue.rs` and scroll to `pop_blocking_or_closed`. Highlight this block:

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

**What to say:**
> "This is the heart of the task queue. When a robot calls `pop_blocking_or_closed`, it first acquires the `Mutex`. If the queue is empty, instead of spinning in a busy loop, it calls `Condvar::wait` — which atomically releases the lock and parks the thread. When `push` adds a new task and calls `notify_one`, exactly one waiting robot wakes up, re-acquires the lock, and retries the check. The `loop` is essential: it guards against spurious wakeups, which the Rust standard library explicitly allows. This pattern — lock, check, wait, recheck — is the canonical correct use of a condition variable."

**Key design decision to highlight:**
> "We use `notify_one` in `push` rather than `notify_all` because only one robot should consume each task. Waking all waiters would cause a thundering herd where every robot races for one task, then all but one go back to sleep — wasteful and unnecessary."

---

**Option B — Zone Access (`src/zones.rs`, lines 24-34 and 37-44)**

Open `src/zones.rs` and scroll to `acquire`. Highlight this block:

```rust
pub fn acquire(&self, zone: ZoneId, robot: RobotId) {
    let mut guard = self.occupied.lock().expect("zone mutex poisoned");
    loop {
        if !guard.contains_key(&zone) {
            guard.insert(zone, robot);
            return;
        }
        // Wait releases the lock; on wake, re-check the condition.
        guard = self.available.wait(guard).expect("condvar wait failed");
    }
}
```

Then scroll down to `release` and highlight:

```rust
pub fn release(&self, zone: ZoneId, robot: RobotId) -> bool {
    let mut guard = self.occupied.lock().expect("zone mutex poisoned");
    match guard.get(&zone) {
        Some(owner) if *owner == robot => {
            guard.remove(&zone);
            self.available.notify_all();
            true
        }
        ...
    }
}
```

**What to say:**
> "Zone access uses the same `Mutex` + `Condvar` pattern, but for mutual exclusion rather than producer-consumer. The `occupied` map is the shared state — it maps a zone ID to the robot currently inside it. When a robot calls `acquire`, it checks if the zone key exists. If it does, another robot is inside, so it waits. When the occupying robot calls `release`, it removes itself from the map and calls `notify_all` to wake every waiting robot. Each woken robot re-checks the map — only the one that finds the zone free proceeds; the rest go back to sleep."

**Key design decision to highlight:**
> "We use `notify_all` here instead of `notify_one` because multiple robots may be waiting for different zones. If we used `notify_one`, a robot waiting for zone 2 might stay asleep even though zone 2 just became free, because the one woken robot was waiting for zone 1. `notify_all` is the safe, correct choice when waiters have heterogeneous conditions."

---

**Option C — Health Monitor (`src/health_monitor.rs`, lines 20-36 and 62-71)**

Open `src/health_monitor.rs` and scroll to `overdue_robots` and `detect_offline`. Highlight:

```rust
fn overdue_robots(
    state: &HealthState,
    now: Instant,
    timeout: Duration,
) -> Vec<RobotId> {
    state
        .last_seen
        .iter()
        .filter_map(|(&robot, &last)| {
            if now.duration_since(last) > timeout {
                Some(robot)
            } else {
                None
            }
        })
        .collect()
}
```

Then highlight `detect_offline`:

```rust
pub fn detect_offline(&self, timeout: Duration) -> HashSet<RobotId> {
    let mut guard = self.state.lock().expect("health monitor mutex poisoned");
    let now = Instant::now();
    // Collect overdue robots first to avoid mutating while iterating.
    let overdue = Self::overdue_robots(&guard, now, timeout);
    for robot in overdue {
        guard.offline.insert(robot);
    }
    guard.offline.clone()
}
```

**What to say:**
> "The health monitor stores two pieces of shared state under a single `Mutex`: a `last_seen` map of robot IDs to `Instant` timestamps, and an `offline` set. When a robot calls `heartbeat`, it updates its entry in `last_seen` and clears itself from `offline`. The background monitor thread calls `detect_offline` on every poll tick — it snapshots `Instant::now()`, computes which robots have exceeded the timeout, and moves them into the `offline` set."

**Key design decision to highlight:**
> "Notice that `overdue_robots` is a pure function that takes an immutable reference to the state — it collects the overdue list without mutating anything. We then mutate `offline` in a second pass. This two-step approach avoids mutating the map while iterating over it, which would be a borrow-checker violation in Rust. It also keeps the critical section minimal: we do the computation, then do the mutation, rather than interleaving them."

---

### Closing line (last 5 seconds)

> "That covers the three core OS concepts — concurrency, synchronization, and coordination — demonstrated live and verified quantitatively. Thank you."

---

## Timing Summary

| Section | Time | Duration |
|---|---|---|
| System Overview | 0:00 - 0:30 | 30 seconds |
| Default Demo (`cargo run`) | 0:30 - 1:20 | 50 seconds |
| Benchmark (`cargo run -- bench ...`) | 1:20 - 1:50 | 30 seconds |
| Stress Sweep (`cargo run -- stress ...`) | 1:50 - 2:30 | 40 seconds |
| Code Walkthrough | 2:30 - 3:00 | 30 seconds |
| **Total** | | **3:00** |

---

## Recording Checklist

- [ ] Terminal font is large enough to read clearly
- [ ] Audio is clear with no background noise
- [ ] All three commands are run and narrated
- [ ] Zone contention (`entered` after `left`) is explicitly pointed out
- [ ] `zone_violation=false` and `duplicate_tasks=false` are called out in bench/stress output
- [ ] Code walkthrough shows actual source with cursor on key lines
- [ ] Video is under 3 minutes and under 50 MB (or uploaded as unlisted YouTube)
- [ ] Exported as MP4 or similar common format
