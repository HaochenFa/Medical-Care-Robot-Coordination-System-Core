//! Thread-safe FIFO task queue with blocking and non-blocking consumers.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

use crate::types::Task;

/// A minimal, synchronized FIFO queue for robot tasks.
pub struct TaskQueue {
    inner: Mutex<TaskQueueState>,
    available: Condvar,
}

struct TaskQueueState {
    queue: VecDeque<Task>,
    closed: bool,
}

impl TaskQueue {
    /// Create an empty task queue.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(TaskQueueState {
                queue: VecDeque::new(),
                closed: false,
            }),
            available: Condvar::new(),
        }
    }

    /// Push a task; returns the task back if the queue is closed.
    pub fn push(&self, task: Task) -> Result<(), Task> {
        let mut guard = self.inner.lock().expect("task queue mutex poisoned");
        if guard.closed {
            return Err(task);
        }
        guard.queue.push_back(task);
        self.available.notify_one();
        Ok(())
    }

    /// Try to pop immediately without blocking.
    pub fn try_pop(&self) -> Option<Task> {
        let mut guard = self.inner.lock().expect("task queue mutex poisoned");
        guard.queue.pop_front()
    }

    #[deprecated(note = "use pop_blocking_or_closed for shutdown-aware waits")]
    #[allow(dead_code)]
    pub fn pop_blocking(&self) -> Task {
        self.pop_blocking_or_closed().expect("task queue closed")
    }

    /// Block until a task is available or the queue is closed.
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

    /// Close the queue and wake all blocked consumers.
    #[allow(dead_code)]
    pub fn close(&self) {
        let mut guard = self.inner.lock().expect("task queue mutex poisoned");
        guard.closed = true;
        self.available.notify_all();
    }

    /// Current number of queued tasks.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        let guard = self.inner.lock().expect("task queue mutex poisoned");
        guard.queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::mpsc;
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn tasks_are_consumed_once() {
        let queue = Arc::new(TaskQueue::new());
        let total_tasks = 100;
        for id in 0..total_tasks {
            queue
                .push(Task::new(id, format!("task-{id}")))
                .expect("task queue closed");
        }

        let consumers = 4;
        let barrier = Arc::new(Barrier::new(consumers));
        let seen: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(HashSet::new()));

        let mut handles = Vec::new();
        for _ in 0..consumers {
            let queue = Arc::clone(&queue);
            let barrier = Arc::clone(&barrier);
            let seen = Arc::clone(&seen);
            handles.push(thread::spawn(move || {
                barrier.wait();
                loop {
                    match queue.try_pop() {
                        Some(task) => {
                            let mut guard = seen.lock().expect("seen mutex poisoned");
                            // Each task id should be observed at most once.
                            assert!(guard.insert(task.id));
                        }
                        None => break,
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("consumer thread panicked");
        }

        let guard = seen.lock().expect("seen mutex poisoned");
        assert_eq!(guard.len(), total_tasks as usize);
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn pop_blocking_wakes_on_push() {
        let queue = Arc::new(TaskQueue::new());
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        let queue_clone = Arc::clone(&queue);
        let handle = thread::spawn(move || {
            ready_tx.send(()).expect("send ready");
            let task = queue_clone
                .pop_blocking_or_closed()
                .expect("task queue closed");
            tx.send(task.id).expect("send task id");
        });

        ready_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("ready");
        // Pushing after the consumer blocks should wake it.
        queue
            .push(Task::new(99, "wake"))
            .expect("task queue closed");

        let received = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("receive task id");
        assert_eq!(received, 99);
        handle.join().expect("blocking pop thread panicked");
    }

    #[test]
    fn blocking_consumers_each_get_unique_task() {
        let queue = Arc::new(TaskQueue::new());
        let consumers = 4;
        let barrier = Arc::new(Barrier::new(consumers));
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();

        let mut handles = Vec::new();
        for _ in 0..consumers {
            let queue = Arc::clone(&queue);
            let barrier = Arc::clone(&barrier);
            let ready_tx = ready_tx.clone();
            let done_tx = done_tx.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                ready_tx.send(()).expect("ready");
                let task = queue.pop_blocking_or_closed().expect("task queue closed");
                done_tx.send(task.id).expect("done");
            }));
        }

        for _ in 0..consumers {
            ready_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("ready recv");
        }

        // Provide exactly one task per consumer.
        for id in 0..consumers as u64 {
            queue
                .push(Task::new(id, format!("task-{id}")))
                .expect("task queue closed");
        }

        let mut seen = HashSet::new();
        for _ in 0..consumers {
            let id = done_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("done recv");
            assert!(seen.insert(id));
        }

        for handle in handles {
            handle.join().expect("consumer thread panicked");
        }
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn pop_blocking_or_closed_unblocks_on_close() {
        let queue = Arc::new(TaskQueue::new());
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();

        let queue_clone = Arc::clone(&queue);
        let handle = thread::spawn(move || {
            ready_tx.send(()).expect("ready");
            let task = queue_clone.pop_blocking_or_closed();
            done_tx.send(task.is_none()).expect("done");
        });

        ready_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("ready");
        queue.close();

        let closed = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("done recv");
        assert!(closed);
        handle.join().expect("consumer thread panicked");
    }

    #[test]
    fn push_fails_after_close() {
        let queue = TaskQueue::new();
        queue.close();
        let result = queue.push(Task::new(1, "late"));
        assert!(result.is_err());
    }
}
