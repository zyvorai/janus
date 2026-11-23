// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

mod bestfit;
mod common;
mod fifo;
mod preemptive;
mod priority;
pub mod resource;
mod zynera;

pub use bestfit::BestFitScheduler;
pub use fifo::FifoScheduler;
pub use preemptive::PreemptivePriorityScheduler;
pub use priority::PriorityScheduler;
pub use resource::{GpuSelectionPolicy, ResourceManager};
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
