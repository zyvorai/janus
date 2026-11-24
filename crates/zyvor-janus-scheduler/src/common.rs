// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use crate::resource::ResourceManager;
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Job, Placement};

/// Try to place one job. On success the chosen resources are marked busy
/// so later calls in the same scheduling pass see them; the caller must
/// pass every returned placement to [`release_markers`] before returning.
pub(crate) fn try_place(
    cluster: &mut Cluster,
    resource_manager: &ResourceManager,
    job: &Job,
) -> Option<Placement> {
    if !resource_manager.can_place(cluster, job) {
        return None;
    }
    let placement = resource_manager
        .allocate(cluster, job, cluster.clock)
        .ok()?;
    for resource_id in &placement.gpu_ids {
        if let Some(slice) = cluster.slice_mut(resource_id) {
            slice.running_job_id = Some(job.id.clone());
        } else if let Some(gpu) = cluster.gpu_mut(resource_id) {
            gpu.running_job_id = Some(job.id.clone());
        }
    }
    Some(placement)
}

/// Undo the temporary busy markers set by [`try_place`]; the engine commits
/// the placements itself.
pub(crate) fn release_markers(cluster: &mut Cluster, placements: &[Placement]) {
    for placement in placements {
        for resource_id in &placement.gpu_ids {
            if let Some(slice) = cluster.slice_mut(resource_id) {
                slice.running_job_id = None;
            } else if let Some(gpu) = cluster.gpu_mut(resource_id) {
                gpu.running_job_id = None;
            }
        }
    }
}

/// Try to place each job in `ordered` (already sorted by queue policy),
/// skipping jobs that don't currently fit (insufficient GPUs, quota, etc.).
pub(crate) fn place_in_order(
    cluster: &mut Cluster,
    resource_manager: &ResourceManager,
    ordered: Vec<Job>,
) -> Vec<Placement> {
    let mut placements = Vec::new();
    for job in ordered {
        if let Some(p) = try_place(cluster, resource_manager, &job) {
            placements.push(p);
        }
    }
    release_markers(cluster, &placements);
    placements
}
