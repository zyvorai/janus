// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

mod bestfit;
mod common;
mod easy;
mod fairshare;
mod fifo;
mod preemptive;
mod priority;
pub mod resource;
mod sjf;
mod zynera;

pub use bestfit::BestFitScheduler;
pub use easy::EasyBackfillScheduler;
pub use fairshare::FairShareScheduler;
pub use fifo::FifoScheduler;
pub use preemptive::PreemptivePriorityScheduler;
pub use priority::PriorityScheduler;
pub use resource::{GpuSelectionPolicy, ResourceManager};
pub use sjf::SjfScheduler;
pub use zynera::ZyneraScheduler;

use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::Placement;

pub trait Scheduler {
    fn schedule(
        &mut self,
        cluster: &mut Cluster,
        resource_manager: &ResourceManager,
    ) -> Vec<Placement>;
}
