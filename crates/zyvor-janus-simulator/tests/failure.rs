// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Gpu, Job, JobState, Node};
use zyvor_janus_scheduler::FifoScheduler;
use zyvor_janus_simulator::{FailureConfig, SimulationEngine};

fn cluster() -> Cluster {
    Cluster::new(vec![Node {
        id: "n0".into(),
        gpus: vec![Gpu::new("g0", "n0", "H100", 80.0)],
    }])
}

fn run(cfg: FailureConfig, runtime: f64) -> Cluster {
    let mut e = SimulationEngine::new(cluster(), FifoScheduler).with_failure_injection(cfg);
    e.submit_jobs(vec![Job::new("j", "j", 0.0, runtime, 1)]);
    e.run();
    e.cluster
}

#[test]
fn hopeless_job_exhausts_retries_and_fails() {
    let c = run(
        FailureConfig {
            seed: 1,
            job_mtbf_secs: 0.001,
            checkpoint_secs: 0.0,
            max_retries: 2,
        },
        1000.0,
    );
    assert_eq!(c.finished_jobs.len(), 1);
    assert_eq!(c.finished_jobs[0].state, JobState::Failed);
    assert_eq!(c.finished_jobs[0].failure_count, 3);
    assert_eq!(c.total_failures, 3);
    assert_eq!(c.finished_jobs[0].preemption_count, 0);
    assert!(c.waiting_queue.is_empty() && c.running_jobs.is_empty());
    assert!(c.gpu("g0").unwrap().is_free());
}

#[test]
fn generous_mtbf_never_crashes() {
    let c = run(
        FailureConfig {
            seed: 1,
            job_mtbf_secs: 1e12,
            checkpoint_secs: 0.0,
            max_retries: 2,
        },
        100.0,
    );
    assert_eq!(c.total_failures, 0);
    assert_eq!(c.finished_jobs[0].state, JobState::Finished);
    assert_eq!(c.finished_jobs[0].finish_time, Some(100.0));
}

#[test]
fn checkpoints_reduce_lost_work() {
    let base = FailureConfig {
        seed: 9,
        job_mtbf_secs: 60.0,
        checkpoint_secs: 0.0,
        max_retries: 1000,
    };
    let none = run(base.clone(), 300.0);
    let ckpt = run(
        FailureConfig {
            checkpoint_secs: 10.0,
            ..base
        },
        300.0,
    );
    // With frequent checkpoints, almost nothing is lost and the job finishes sooner.
    assert!(ckpt.lost_gpu_seconds < none.lost_gpu_seconds);
    assert_eq!(ckpt.finished_jobs[0].state, JobState::Finished);
    // Run time can never be shorter than the work itself.
    assert!(ckpt.finished_jobs[0].finish_time.unwrap() >= 300.0);
}

#[test]
fn work_accounting_balances() {
    // finish = work + lost time (single job, single GPU, no queueing)
    let c = run(
        FailureConfig {
            seed: 4,
            job_mtbf_secs: 80.0,
            checkpoint_secs: 25.0,
            max_retries: 1000,
        },
        400.0,
    );
    let j = &c.finished_jobs[0];
    assert_eq!(j.state, JobState::Finished);
    let expected = 400.0 + c.lost_gpu_seconds;
    assert!((j.finish_time.unwrap() - expected).abs() < 1e-6);
}
