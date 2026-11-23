// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    JobArrival,
    JobComplete,
    GangTimeout,
    /// Injected crash of a running job (failure injection).
    JobFailure,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub time: f64,
    pub kind: EventKind,
    pub job_id: String,
    /// For `JobComplete`: the `Job::run_generation` this event completes.
    /// If a job is preempted and restarted before this event fires, its
    /// generation moves on and the event is stale — the engine ignores it
    /// instead of finishing a run that isn't the one it was scheduled for.
    /// Unused for `JobArrival`.
    pub run_generation: u32,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time.to_bits() == other.time.to_bits()
            && self.kind == other.kind
            && self.job_id == other.job_id
    }
}

impl Eq for Event {}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .time
            .partial_cmp(&self.time)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.job_id.cmp(&other.job_id))
    }
}

#[derive(Debug, Default)]
pub struct EventQueue {
    heap: std::collections::BinaryHeap<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, event: Event) {
        self.heap.push(event);
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.heap.pop()
    }

    pub fn peek(&self) -> Option<&Event> {
        self.heap.peek()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_pop_in_time_order() {
        let mut q = EventQueue::new();
        q.push(Event {
            time: 10.0,
            kind: EventKind::JobComplete,
            job_id: "a".into(),
            run_generation: 1,
        });
        q.push(Event {
            time: 5.0,
            kind: EventKind::JobArrival,
            job_id: "b".into(),
            run_generation: 0,
        });
        assert_eq!(q.pop().unwrap().time, 5.0);
        assert_eq!(q.pop().unwrap().time, 10.0);
    }
}
