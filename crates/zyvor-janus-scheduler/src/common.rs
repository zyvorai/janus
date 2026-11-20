// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use crate::resource::ResourceManager;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, Placement};

/// Try to place each job in `ordered` (already sorted by queue policy),
/// skipping jobs that don't currently fit (insufficient GPUs, quota, etc.).
pub(crate) fn place_in_order(
    cluster: &mut Cluster,
    resource_manager: &ResourceManager,
    ordered: Vec<Job>,
) -> Vec<Placement> {
    let mut placements = Vec::new();

    for job in ordered {
        if !resource_manager.can_place(cluster, &job) {
            continue;
        }
        match resource_manager.allocate(cluster, &job, cluster.clock) {
            Ok(placement) => {
                for resource_id in &placement.gpu_ids {
                    if let Some(slice) = cluster.slice_mut(resource_id) {
                        slice.running_job_id = Some(job.id.clone());
                    } else if let Some(gpu) = cluster.gpu_mut(resource_id) {
                        gpu.running_job_id = Some(job.id.clone());
                    }
                }
                placements.push(placement);
            }
            Err(_) => continue,
        }
    }

    for placement in &placements {
        for resource_id in &placement.gpu_ids {
            if let Some(slice) = cluster.slice_mut(resource_id) {
                slice.running_job_id = None;
            } else if let Some(gpu) = cluster.gpu_mut(resource_id) {
                gpu.running_job_id = None;
            }
        }
    }

    placements
}
