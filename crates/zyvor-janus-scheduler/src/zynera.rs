// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

//! Zynera-inspired baseline scheduler: priority ordering with preemption.
//! Quotas, gang spread, and topology locality are enforced by `ResourceManager`,
//! not here — this is not full kube-scheduler / Zynera plugin parity.

pub use crate::preemptive::PreemptivePriorityScheduler as ZyneraScheduler;
