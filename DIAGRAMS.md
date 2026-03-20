# Medical Care Robot Coordination System (MCRoCS) Diagrams

These Mermaid diagrams reflect the current implementation in:

- `src/main.rs`
- `src/sim.rs`
- `src/task_queue.rs`
- `src/zones.rs`
- `src/health_monitor.rs`

## 1) High-Level Architecture

```mermaid
flowchart TD
    CLI["CLI (main.rs)"] --> MODE{"Subcommand"}
    MODE --> DEMO["run_demo()"]
    MODE --> BENCH["run_benchmark()"]
    MODE --> STRESS["run_stress()"]

    DEMO --> TQ["TaskQueue"]
    DEMO --> ZA["ZoneAccess"]
    DEMO --> HM["HealthMonitor"]

    BENCH --> TQ
    BENCH --> ZA
    BENCH --> HM

    STRESS --> BENCH

    TQ --> TQSYNC["Mutex&lt;VecDeque&lt;Task&gt;&gt; + Condvar"]
    ZA --> ZASYNC["Mutex&lt;HashMap&lt;ZoneId, RobotId&gt;&gt; + Vec&lt;Condvar&gt; (per-zone)"]
    HM --> HMSYNC["RwLock&lt;HashMap&gt; (heartbeats) + Mutex&lt;HashSet&gt; (offline)"]

    LOG["log_dev! macro (debug builds only)"]
    DEMO -.-> LOG
    BENCH -.-> LOG
```

Note: `log_dev!` emits structured logs in debug builds and is a no-op in release. It is used throughout `sim.rs` — dashed lines indicate optional/debug-only coupling. `ZoneAccess` uses per-zone `Condvar`s (indexed by `zone % len`) so `notify_one` wakes only contenders for the released zone. `HealthMonitor` uses a split-lock: `RwLock` for concurrent heartbeat reads and a separate `Mutex` for the offline set.

## 2) Demo Flow (Deterministic Offline Target)

Demo constants: `robots=3`, `tasks_per_robot=3`, `zones_total=2`, `offline_target=1`.
Robot 1 sends heartbeats only after its first 2 tasks (`stop_heartbeat_after=2`), then stops.

```mermaid
sequenceDiagram
    participant R0 as Robot 0
    participant R1 as Robot 1 (offline target)
    participant R2 as Robot 2
    participant Q as TaskQueue
    participant Z as ZoneAccess
    participant H as HealthMonitor
    participant M as Monitor Thread
    participant Main as Main Thread

    par Robots fetch tasks concurrently (3 tasks each)
        R0->>Q: pop_blocking_or_closed()
        R1->>Q: pop_blocking_or_closed()
        R2->>Q: pop_blocking_or_closed()
    end

    note over Z: zone = (task.id % 2) + 1, so only zones 1 and 2 exist

    R0->>Z: acquire(zone)
    R1->>Z: acquire(zone)
    R2->>Z: acquire(zone)
    Z-->>R0: granted (blocks if zone busy)
    Z-->>R1: granted (blocks if zone busy)
    Z-->>R2: granted (blocks if zone busy)

    R0->>H: heartbeat() after each task
    R1->>H: heartbeat() after tasks 1 and 2 only
    R2->>H: heartbeat() after each task

    M->>H: detect_offline(200ms) every 50ms

    note over R1: stops heartbeats after task 2 completed

    note over Main: all robot threads join, then...
    Main->>H: keepalive heartbeats for robots 0 and 2
    Main->>H: wait_for_specific_offline(target=1, timeout=200ms)
    H-->>Main: robot 1 offline detected
```

## 3) TaskQueue: Push and Blocking Pop

```mermaid
flowchart TD
    P["Producer calls push(task)"] --> LOCK1["Lock Mutex"]
    LOCK1 --> CLOSED{"queue closed?"}
    CLOSED -- "yes" --> ERR["Return Err(task)"]
    CLOSED -- "no" --> ENQUEUE["push_back to VecDeque"]
    ENQUEUE --> NOTIFY["notify_one (wake a consumer)"]
    NOTIFY --> UNLOCK1["Unlock Mutex"]

    C["Consumer calls pop_blocking_or_closed()"] --> LOCK2["Lock Mutex"]
    LOCK2 --> CHECK{"task available?"}
    CHECK -- "yes" --> POP["pop_front, return Some(task)"]
    CHECK -- "no" --> ISCLOSE{"queue closed?"}
    ISCLOSE -- "yes" --> NONE["Return None"]
    ISCLOSE -- "no" --> WAIT["Condvar::wait (releases lock)"]
    WAIT --> CHECK
```

## 4) Zone Access Control

```mermaid
flowchart TD
    A["Robot calls acquire(zone, robot)"] --> LOCK["Lock Mutex"]
    LOCK --> IDX["Compute condvar index\n(zone % condvars.len())"]
    IDX --> OCC{"zone in occupied map?"}
    OCC -- "no" --> INSERT["Insert zone → robot_id"]
    INSERT --> UNLOCK_A["Unlock, return"]
    OCC -- "yes" --> WAIT["condvars[idx].wait (releases lock)"]
    WAIT --> OCC

    R["Robot calls release(zone, robot)"] --> LOCKR["Lock Mutex"]
    LOCKR --> OWNER{"caller is owner?"}
    OWNER -- "yes" --> REMOVE["Remove zone from map"]
    REMOVE --> NOTIFYONE["condvars[idx].notify_one (wake one waiter)"]
    NOTIFYONE --> UNLOCK_R["Unlock, return true"]
    OWNER -- "no / unoccupied" --> FALSE["return false (caller error)"]
```

## 5) HealthMonitor State Transitions

The monitor uses a split-lock design: `RwLock<HashMap<RobotId, Instant>>` for heartbeat timestamps (allowing concurrent reads during detection) and a separate `Mutex<HashSet<RobotId>>` for the offline set.

```mermaid
stateDiagram-v2
    [*] --> Unregistered
    Unregistered --> Online: register_robot() or heartbeat()
    Online --> Online: heartbeat() — write-locks heartbeats, clears offline mark
    Online --> Offline: detect_offline() — read-locks heartbeats to scan, then write-locks offline set
    Offline --> Online: heartbeat() — clears offline mark
```

## 6) Benchmark/Stress Offline Semantics

```mermaid
flowchart TD
    A["Start benchmark/stress run"] --> B{"offline-demo flag?"}
    B -- "no" --> C["All robots send heartbeats throughout"]
    B -- "yes" --> D["Robot 0 stops heartbeats after tasks_per_robot/2"]
    C --> E["Workers finish all tasks"]
    D --> E
    E --> F["wait_for_offline(any, 500ms timeout, 1000ms max)"]
    F --> G["Collect offline_robots count"]
    G --> H["Emit CSV row"]
```

Interpretation:

- In demo mode, the offline target is deterministic (robot 1 stops after 2 of 3 tasks).
- In benchmark/stress offline mode, robot 0 stops heartbeats at the halfway point; `offline_robots >= 1` is expected and acceptable at run end.
