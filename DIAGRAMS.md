# Medical Care Robot Coordination System (MCRoCS) Diagrams

These Mermaid diagrams are aligned with the current implementation in:

- `src/main.rs`
- `src/sim.rs`
- `src/task_queue.rs`
- `src/zones.rs`
- `src/health_monitor.rs`

## 1) High-Level Architecture

```mermaid
flowchart TD
    CLI["CLI (main.rs)"] --> MODE{"Mode"}
    MODE --> DEMO["Demo: run_demo()"]
    MODE --> BENCH["Benchmark: run_benchmark()"]
    MODE --> STRESS["Stress: run_stress()"]

    DEMO --> TQ["TaskQueue"]
    DEMO --> ZA["ZoneAccess"]
    DEMO --> HM["HealthMonitor"]
    DEMO --> LOG["Debug logging macro"]

    BENCH --> TQ
    BENCH --> ZA
    BENCH --> HM

    STRESS --> BENCH

    TQ --> TQSYNC["Mutex + Condvar (VecDeque)"]
    ZA --> ZASYNC["Mutex + Condvar (zone ownership map)"]
    HM --> HMSYNC["Mutex (last_seen + offline set)"]
```

## 2) Demo Flow (Deterministic Offline Target)

```mermaid
sequenceDiagram
    participant R0 as Robot 0
    participant R1 as Robot 1 (offline target)
    participant R2 as Robot 2
    participant Q as TaskQueue
    participant Z as ZoneAccess
    participant H as HealthMonitor
    participant M as Monitor Thread
    participant W as Main Wait Logic

    par Concurrent task fetch
        R0->>Q: pop_blocking_or_closed()
        R1->>Q: pop_blocking_or_closed()
        R2->>Q: pop_blocking_or_closed()
    end

    R0->>Z: acquire(zone)
    R1->>Z: acquire(zone)
    R2->>Z: acquire(zone)
    Z-->>R0: granted when free
    Z-->>R1: granted when free
    Z-->>R2: granted when free

    R0->>H: heartbeat()
    R1->>H: heartbeat() until cutoff
    R2->>H: heartbeat()

    M->>H: detect_offline(timeout)
    W->>H: keepalive heartbeats for non-target robots
    W->>H: wait_for_specific_offline(target=1)
    H-->>W: target robot offline detected
```

## 3) Zone Access Control Logic

```mermaid
flowchart TD
    A["Robot requests zone"] --> B{"Zone occupied?"}
    B -- "no" --> C["Insert owner(robot_id)"]
    B -- "yes" --> D["Wait on condvar"]
    D --> B
    C --> E["Robot performs work"]
    E --> F["Release by owner"]
    F --> G["Remove owner entry"]
    G --> H["Notify waiters"]
```

## 4) Benchmark/Stress Offline Semantics

```mermaid
flowchart TD
    A["Start benchmark/stress run"] --> B{"offline-demo enabled?"}
    B -- "no" --> C["Normal run"]
    B -- "yes" --> D["Robot 0 stops heartbeats early"]
    C --> E["Workers finish tasks"]
    D --> E
    E --> F["wait_for_offline(any) with timeout window"]
    F --> G["Collect offline_robots count"]
    G --> H["Emit CSV row"]
```

Interpretation note:

- In demo mode, the offline target is deterministic (`robot 1`).
- In benchmark/stress offline mode, `offline_robots` may be greater than 1 near run end; this is acceptable and expected.
