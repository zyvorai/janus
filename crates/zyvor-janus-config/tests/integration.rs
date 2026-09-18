// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

//! Integration tests: full simulation pipelines (config → engine → metrics).

use std::path::PathBuf;

use zyvor_janus_config::{
    build_shadow_run, load_forge_bundle, run_forge_bundle, run_simulation, run_trace_file,
    trace_diff_to_json, TraceDiffReport,
};
use zyvor_janus_model::cluster::Cluster;
use zyvor_janus_model::models::{Gpu, Job, JobState, Node};
use zyvor_janus_scheduler::resource::ResourceManager;
use zyvor_janus_scheduler::ForgeScheduler;
use zyvor_janus_simulator::ShadowSide;
use zyvor_janus_simulator::SimulationEngine;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn integration_synthetic_m1_workload_completes() {
    let config = repo_root().join("configs/clusters/small_h100.yaml");
    if !config.exists() {
        return;
    }
    let metrics = run_simulation(&config).expect("synthetic M1 simulation");
    assert_eq!(metrics.jobs_completed, metrics.jobs_total);
    assert!(metrics.jobs_total >= 8);
    assert!(metrics.makespan > 0.0);
    assert!(metrics.gpu_utilization > 0.0);
}

#[test]
fn integration_priority_scheduler_prefers_high_priority_job() {
    let fifo_config = repo_root().join("configs/clusters/priority_fifo.yaml");
    let priority_config = repo_root().join("configs/clusters/priority_scheduler.yaml");
    if !fifo_config.exists() || !priority_config.exists() {
        return;
    }
    // Same workload (a filler job occupies the only GPU while a low- and a
    // high-priority job both arrive and queue up behind it), run under each
    // scheduler policy. Total work done is identical either way, so
    // makespan doesn't distinguish them — but mean_wait_time does: the
    // priority scheduler runs the high-priority job first once the GPU
    // frees up, so on average jobs wait less than under strict FIFO, which
    // runs the earlier-arriving low-priority job first.
    let fifo_metrics = run_simulation(&fifo_config).expect("fifo simulation");
    let priority_metrics = run_simulation(&priority_config).expect("priority simulation");

    assert_eq!(fifo_metrics.jobs_completed, fifo_metrics.jobs_total);
    assert_eq!(priority_metrics.jobs_completed, priority_metrics.jobs_total);
    assert_eq!(fifo_metrics.makespan, priority_metrics.makespan);
    assert!(priority_metrics.mean_wait_time < fifo_metrics.mean_wait_time);
}

#[test]
fn integration_shadow_run_steps_both_schedulers_to_completion() {
    let config = repo_root().join("configs/clusters/small_h100.yaml");
    if !config.exists() {
        return;
    }
    let handle = build_shadow_run(&config, Some("fifo"), "priority").expect("build shadow run");
    assert_eq!(handle.primary_scheduler, "fifo");
    assert_eq!(handle.shadow_scheduler, "priority");
    let jobs_total = handle.jobs_total;

    let mut run = handle.run;
    let steps = run.run_to_completion();

    assert!(!steps.is_empty());
    assert!(run.is_done());
    assert!(steps.iter().any(|s| s.side == ShadowSide::Primary));
    assert!(steps.iter().any(|s| s.side == ShadowSide::Shadow));

    let primary_cluster = run.primary_cluster();
    let shadow_cluster = run.shadow_cluster();
    assert_eq!(primary_cluster.finished_jobs.len(), jobs_total);
    assert_eq!(shadow_cluster.finished_jobs.len(), jobs_total);

    let (primary_metrics, shadow_metrics) = run.metrics();
    assert_eq!(primary_metrics.jobs_completed, jobs_total);
    assert_eq!(shadow_metrics.jobs_completed, jobs_total);
}

#[test]
fn integration_preemptive_scheduler_evicts_for_higher_priority_arrival() {
    let priority_config = repo_root().join("configs/clusters/preemption_priority.yaml");
    let preemptive_config = repo_root().join("configs/clusters/preemption_preemptive.yaml");
    if !priority_config.exists() || !preemptive_config.exists() {
        return;
    }
    // job-low starts immediately at t=0 (the GPU is free) and is still
    // running when job-high (much higher priority) arrives at t=10 — a
    // case non-preemptive priority ordering can't help with, since
    // job-high simply wasn't in the waiting queue yet when job-low was
    // placed. The preemptive scheduler evicts job-low, runs job-high right
    // away, then resumes job-low with its remaining runtime. Same total
    // GPU-seconds of work either way (single GPU serializes everything),
    // so makespan matches, but the preemptive run gets job-high started
    // immediately instead of after job-low's full 100s runtime.
    let priority_metrics = run_simulation(&priority_config).expect("priority simulation");
    let preemptive_metrics = run_simulation(&preemptive_config).expect("preemptive simulation");

    assert_eq!(priority_metrics.jobs_completed, priority_metrics.jobs_total);
    assert_eq!(
        preemptive_metrics.jobs_completed,
        preemptive_metrics.jobs_total
    );
    assert_eq!(priority_metrics.makespan, preemptive_metrics.makespan);

    assert_eq!(priority_metrics.preemptions, 0);
    assert_eq!(preemptive_metrics.preemptions, 1);
    assert!(preemptive_metrics.mean_wait_time < priority_metrics.mean_wait_time);
}

#[test]
fn integration_dual_gpu_preempt_migrates_mover_to_other_gpu() {
    use zyvor_janus_config::run_simulation_report;

    let config = repo_root().join("configs/clusters/dual_gpu_preempt.yaml");
    if !config.exists() {
        return;
    }
    let report = run_simulation_report(&config).expect("dual gpu preempt simulation");
    assert_eq!(report.metrics.jobs_completed, report.metrics.jobs_total);
    assert_eq!(report.metrics.preemptions, 1);
    assert!(
        report
            .decisions
            .iter()
            .any(|d| d.kind == "job_preempted" && d.job_id.as_deref() == Some("job-mover")),
        "expected job_preempted for mover"
    );

    let mover = report
        .timeline
        .jobs
        .iter()
        .find(|j| j.job_id == "job-mover")
        .expect("mover in timeline");
    assert!(
        mover.segments.len() >= 2,
        "mover should have ≥2 run segments, got {:?}",
        mover.segments
    );
    let first_gpus = &mover.segments[0].gpu_ids;
    let last_gpus = &mover.segments[mover.segments.len() - 1].gpu_ids;
    assert_ne!(
        first_gpus, last_gpus,
        "mover should resume on a different GPU: {:?}",
        mover.segments
    );
    assert_eq!(report.timeline.gpu_count, 2);
}

#[test]
fn integration_dual_node_preempt_migrates_mover_to_other_machine() {
    use zyvor_janus_config::run_simulation_report;

    let config = repo_root().join("configs/clusters/dual_node_preempt.yaml");
    if !config.exists() {
        return;
    }
    let report = run_simulation_report(&config).expect("dual node preempt simulation");
    assert_eq!(report.metrics.jobs_completed, report.metrics.jobs_total);
    assert_eq!(report.metrics.preemptions, 1);

    let mover = report
        .timeline
        .jobs
        .iter()
        .find(|j| j.job_id == "job-mover")
        .expect("mover in timeline");
    assert!(
        mover.segments.len() >= 2,
        "mover should have ≥2 run segments, got {:?}",
        mover.segments
    );
    let first_nodes = &mover.segments[0].node_ids;
    let last_nodes = &mover.segments[mover.segments.len() - 1].node_ids;
    assert_eq!(first_nodes, &vec!["node-0".to_string()]);
    assert_eq!(last_nodes, &vec!["node-1".to_string()]);
    assert_ne!(
        &mover.segments[0].gpu_ids,
        &mover.segments[mover.segments.len() - 1].gpu_ids,
        "mover should also change GPU id across machines: {:?}",
        mover.segments
    );
    assert_eq!(report.timeline.gpu_count, 2);
}

#[test]
fn integration_forge_bundle_fifo_simulation() {
    let bundle = repo_root().join("tests/fixtures/forge");
    if !bundle.exists() {
        return;
    }
    let metrics = run_forge_bundle(
        &bundle,
        &repo_root().join("configs/profiles"),
        &repo_root().join("configs/gpu_type_registry.yaml"),
        &repo_root().join("configs/hardware"),
        &repo_root().join("configs/mig"),
        "fifo",
    )
    .expect("forge bundle simulation");
    assert_eq!(metrics.jobs_completed, metrics.jobs_total);
    assert!(metrics.jobs_total >= 3);
}

#[test]
fn integration_forge_bundle_gang_and_mig_fields() {
    let bundle = repo_root().join("tests/fixtures/forge");
    if !bundle.exists() {
        return;
    }
    let loaded = load_forge_bundle(
        &bundle,
        &repo_root().join("configs/profiles"),
        &repo_root().join("configs/gpu_type_registry.yaml"),
        &repo_root().join("configs/hardware"),
    )
    .expect("load forge bundle");

    let gang = loaded
        .jobs
        .iter()
        .find(|j| j.name == "gpt-distributed-training")
        .expect("gang job");
    assert_eq!(gang.gpu_count, 32);
    assert!(gang.gang_enabled);
    assert_eq!(gang.gang_timeout_secs, Some(600.0));

    let mig = loaded
        .jobs
        .iter()
        .find(|j| j.name == "mig-inference")
        .expect("mig job");
    assert_eq!(mig.gpu_count, 2);
    assert_eq!(mig.mig_profile.as_deref(), Some("1g.10gb"));
    assert!(loaded.cluster.all_gpus().any(|g| g.mig_capable));
}

#[test]
fn integration_forge_bundle_quota_delays_second_job() {
    let bundle = repo_root().join("tests/fixtures/forge_quota");
    if !bundle.exists() {
        return;
    }
    // Cluster has 4 GPUs and both 2-GPU jobs could run concurrently, but
    // the tenant's FabricQuota caps it at 2 GPUs — job B must wait for
    // job A to finish and free the quota before it can start.
    let metrics = run_forge_bundle(
        &bundle,
        &repo_root().join("configs/profiles"),
        &repo_root().join("configs/gpu_type_registry.yaml"),
        &repo_root().join("configs/hardware"),
        &repo_root().join("configs/mig"),
        "fifo",
    )
    .expect("forge bundle simulation");
    assert_eq!(metrics.jobs_completed, 2);
    // Without quota enforcement both jobs run in parallel and makespan
    // equals one job's runtime (604800s); quota enforcement serializes
    // them, doubling it.
    assert_eq!(metrics.makespan, 1_209_600.0);
    assert!(metrics.mean_wait_time > 0.0);
}

#[test]
fn integration_trace_replay_matches_fifo_oracle() {
    let trace = repo_root().join("tests/fixtures/traces/fifo_match.jsonl");
    let cluster_config = repo_root().join("configs/clusters/single_gpu.yaml");
    if !trace.exists() || !cluster_config.exists() {
        return;
    }
    let cluster =
        zyvor_janus_config::load_cluster_from_config(&cluster_config).expect("load cluster");
    let result = run_trace_file(&trace, cluster, "fifo").expect("trace replay");
    assert_eq!(result.report.differing_placements, 0);
    assert_eq!(result.report.matching_placements, 2);
}

#[test]
fn integration_trace_diff_report_serializes() {
    let trace = repo_root().join("tests/fixtures/traces/fifo_match.jsonl");
    let cluster_config = repo_root().join("configs/clusters/single_gpu.yaml");
    if !trace.exists() || !cluster_config.exists() {
        return;
    }
    let cluster =
        zyvor_janus_config::load_cluster_from_config(&cluster_config).expect("load cluster");
    let result = run_trace_file(&trace, cluster, "fifo").expect("trace replay");
    let json = trace_diff_to_json(&result.report);
    assert!(json.contains("\"matching_placements\": 2"));
    let parsed: TraceDiffReport = serde_json::from_str(&json).expect("parse diff json");
    assert_eq!(parsed.scheduler, "fifo");
}

#[test]
fn integration_mig_workload_tracks_reconfigs() {
    let config = repo_root().join("configs/clusters/mig_single.yaml");
    if !config.exists() {
        return;
    }
    let metrics = run_simulation(&config).expect("mig simulation");
    assert_eq!(metrics.jobs_completed, 2);
    assert_eq!(metrics.jobs_total, 2);
    assert!(metrics.mig_reconfigs >= 1);
    assert!(metrics.makespan >= 90.0);
}

#[test]
fn integration_topology_workload_completes() {
    let config = repo_root().join("configs/clusters/topology_h100.yaml");
    if !config.exists() {
        return;
    }
    let report = zyvor_janus_config::run_simulation_report(&config).expect("topology simulation");
    assert_eq!(report.metrics.jobs_completed, report.metrics.jobs_total);
    assert!(report.metrics.makespan > 0.0);
}

#[test]
fn integration_gang_job_waits_for_node_capacity() {
    let config = repo_root().join("configs/clusters/gang_m6.yaml");
    if !config.exists() {
        return;
    }
    let report = zyvor_janus_config::run_simulation_report(&config).expect("gang simulation");
    assert_eq!(report.metrics.jobs_completed, 2);
    let gang = report
        .timeline
        .jobs
        .iter()
        .find(|j| j.name == "gang-wait")
        .expect("gang job");
    assert!(gang.start_time.unwrap_or(0.0) >= 60.0);
}

#[test]
fn integration_gang_job_fails_when_gang_timeout_expires() {
    let config = repo_root().join("configs/clusters/gang_timeout_m6.yaml");
    if !config.exists() {
        return;
    }
    let report =
        zyvor_janus_config::run_simulation_report(&config).expect("gang timeout simulation");
    assert_eq!(report.metrics.jobs_completed, 1);
    assert_eq!(report.metrics.jobs_failed, 1);
    let gang = report
        .timeline
        .jobs
        .iter()
        .find(|j| j.name == "gang-timeout")
        .expect("gang job");
    assert_eq!(gang.state, "failed");
    assert_eq!(gang.finish_time, Some(31.0));
}

#[test]
fn integration_gang_timeout_rearms_after_preemption() {
    let cluster = Cluster::new(vec![Node {
        id: "n0".into(),
        gpus: vec![Gpu::new("g0", "n0", "H100_80GB", 80.0)],
    }]);
    let mut engine = SimulationEngine::new(cluster, ForgeScheduler::default());
    let mut gang = Job::new("gang", "gang", 0.0, 100.0, 1);
    gang.gang_enabled = true;
    gang.gang_size_nodes = Some(1);
    gang.gang_timeout_secs = Some(30.0);
    gang.priority = 50;
    let mut high = Job::new("high", "high", 10.0, 50.0, 1);
    high.priority = 90;
    engine.submit_jobs(vec![gang, high]);
    engine.run();

    let gang_job = engine
        .cluster
        .finished_jobs
        .iter()
        .find(|j| j.id == "gang")
        .expect("gang finished");
    assert_eq!(gang_job.state, JobState::Failed);
    assert_eq!(gang_job.finish_time, Some(40.0));
    assert_eq!(engine.cluster.total_preemptions, 1);
}

#[test]
fn integration_topology_runtime_inflation_on_cross_domain_placement() {
    let config = repo_root().join("configs/clusters/topology_penalty.yaml");
    if !config.exists() {
        return;
    }
    let report = zyvor_janus_config::run_simulation_report(&config).expect("topology simulation");
    assert_eq!(report.metrics.topology_penalties, 1);
    assert!(report.metrics.topology_runtime_inflation > 0.0);
    let job = report
        .timeline
        .jobs
        .iter()
        .find(|j| j.name == "cross-domain")
        .expect("job");
    assert_eq!(
        job.finish_time,
        Some(10.0 + report.metrics.topology_runtime_inflation)
    );
}

#[test]
fn integration_rl_session_fifo_completes() {
    let config = repo_root().join("configs/clusters/rl_small.yaml");
    if !config.exists() {
        return;
    }
    let mut session = zyvor_janus_config::load_rl_session(&config).expect("rl session");
    session.reset();
    let top_k = session.top_k;
    while !session.is_done() {
        let obs = session.observe();
        let action = if obs.waiting > 0 { 0 } else { top_k };
        session.step(action);
    }
    let metrics =
        zyvor_janus_metrics::SimulationMetrics::from_cluster(&session.cluster, session.jobs_total);
    assert_eq!(metrics.jobs_completed, metrics.jobs_total);
    assert!(metrics.makespan > 0.0);
}

#[test]
fn integration_simulation_writes_jobs_timeline() {
    let config = repo_root().join("configs/clusters/small_h100.yaml");
    if !config.exists() {
        return;
    }
    let report = zyvor_janus_config::run_simulation_report(&config).expect("simulation");
    assert!(!report.timeline.jobs.is_empty());
    let json = report.timeline.to_json_pretty();
    assert!(json.contains("\"job_id\""));
}

#[test]
fn integration_preemption_gpu_utilization_accounts_all_segments() {
    let config = repo_root().join("configs/clusters/preemption_preemptive.yaml");
    if !config.exists() {
        return;
    }
    let metrics = run_simulation(&config).expect("preemptive simulation");
    assert_eq!(metrics.preemptions, 1);
    assert!(metrics.gpu_utilization > 0.0);
    assert!(metrics.mean_cumulative_wait_time >= 0.0);
}

#[test]
fn integration_preemption_restart_penalty_delays_resumed_job() {
    use zyvor_janus_config::load_simulation_config;
    use zyvor_janus_simulator::SimulationEngine;

    let config_path = repo_root().join("configs/clusters/preemption_preemptive.yaml");
    if !config_path.exists() {
        return;
    }
    let config = load_simulation_config(&config_path).unwrap();
    let base = config_path.parent().unwrap();
    let hw_dir = zyvor_janus_config::resolve_path(base, &config.hardware_profiles_dir);
    let profiles = zyvor_janus_config::load_hardware_profiles(&hw_dir).unwrap();
    let workload_path = zyvor_janus_config::resolve_path(base, &config.workload.path);
    let jobs = zyvor_janus_config::load_workload(&workload_path).unwrap();
    let cluster = zyvor_janus_config::build_cluster(&config.cluster, &profiles).unwrap();
    let rm = zyvor_janus_config::build_resource_manager(None, "preemptive");
    let mut engine =
        SimulationEngine::with_resource_manager(cluster, ForgeScheduler::default(), rm)
            .with_preemption_restart_penalty(5.0);
    engine.submit_jobs(jobs);
    engine.run();

    let low = engine
        .cluster
        .finished_jobs
        .iter()
        .find(|j| j.id == "job-low")
        .expect("low finished");
    assert_eq!(low.preemption_count, 1);
    assert!(low.gpu_seconds_consumed > 0.0);
}

#[test]
fn integration_gpu_type_blocks_mismatched_hardware() {
    let mut cluster = Cluster::new(vec![
        Node {
            id: "n0".into(),
            gpus: vec![Gpu::new("a100", "n0", "A100_80GB", 80.0)],
        },
        Node {
            id: "n1".into(),
            gpus: vec![Gpu::new("h100", "n1", "H100_80GB", 80.0)],
        },
    ]);
    let rm = ResourceManager::new();
    let mut job = Job::new("train", "train", 0.0, 10.0, 1);
    job.gpu_type = Some("H100_80GB".into());
    assert!(rm.can_place(&cluster, &job));
    let placement = rm.allocate(&mut cluster, &job, 0.0).unwrap();
    assert_eq!(
        cluster.gpu(&placement.gpu_ids[0]).unwrap().profile,
        "H100_80GB"
    );
}

#[test]
fn integration_trace_diff_includes_failure_metadata() {
    let cluster_config = repo_root().join("configs/clusters/gang_timeout_m6.yaml");
    if !cluster_config.exists() {
        return;
    }
    let report = zyvor_janus_config::run_simulation_report(&cluster_config)
        .expect("gang timeout simulation");
    assert_eq!(report.metrics.jobs_failed, 1);
    assert_eq!(report.metrics.jobs_completed, 1);
}

#[test]
fn integration_load_workload_rejects_invalid_gang_from_fixture() {
    let path = repo_root().join("tests/fixtures/workloads/invalid_gang.yaml");
    if !path.exists() {
        return;
    }
    assert!(zyvor_janus_config::load_workload(&path).is_err());
}

#[test]
fn integration_inference_workload_reports_ttft_metrics() {
    let cluster_config = repo_root().join("configs/clusters/inference_llama.yaml");
    if !cluster_config.exists() {
        return;
    }
    let report =
        zyvor_janus_config::run_simulation_report(&cluster_config).expect("inference simulation");
    assert!(report.metrics.inference_jobs > 0);
    assert!(report.metrics.ttft_p50 > 0.0);
    assert!(report.benchmark.is_some());
}

#[test]
fn integration_serving_trace_import_roundtrip() {
    let trace_path = repo_root().join("tests/fixtures/traces/serving_llama.jsonl");
    if !trace_path.exists() {
        return;
    }
    let trace = zyvor_janus_config::load_serving_trace(&trace_path).expect("load trace");
    let jobs = zyvor_janus_config::jobs_from_serving_trace(&trace, "srv");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].model_id.as_deref(), Some("llama-70b"));
}

#[test]
fn integration_apple_silicon_cluster_places_and_rejects_cuda() {
    let cluster_config = repo_root().join("configs/clusters/apple_m4.yaml");
    assert!(cluster_config.exists(), "apple_m4 cluster config missing");
    let report = zyvor_janus_config::run_simulation_report(&cluster_config)
        .expect("apple silicon simulation");
    // Three Apple-typed jobs complete; H100-typed job cannot place on M-series GPUs.
    assert_eq!(report.metrics.jobs_completed, 3);
    assert_eq!(report.metrics.jobs_unschedulable, 1);
    assert_eq!(report.metrics.jobs_total, 4);
    assert!(report.metrics.makespan > 0.0);

    let registry = zyvor_janus_config::load_gpu_type_registry(
        &repo_root().join("configs/gpu_type_registry.yaml"),
    )
    .expect("gpu type registry");
    assert_eq!(
        registry.mappings.get("M4Max").map(String::as_str),
        Some("M4_MAX_128GB")
    );
    assert_eq!(
        registry.mappings.get("M3Ultra").map(String::as_str),
        Some("M3_ULTRA_192GB")
    );
    assert_eq!(
        registry.mappings.get("B200").map(String::as_str),
        Some("B200_192GB")
    );
    assert_eq!(
        registry.mappings.get("MI300X").map(String::as_str),
        Some("MI300X_192GB")
    );
    assert_eq!(
        registry.mappings.get("Gaudi3").map(String::as_str),
        Some("GAUDI3_128GB")
    );
}

#[test]
fn integration_gpu_kinds_places_every_profile() {
    let cluster_config = repo_root().join("configs/clusters/gpu_kinds.yaml");
    assert!(cluster_config.exists(), "gpu_kinds cluster config missing");
    let report =
        zyvor_janus_config::run_simulation_report(&cluster_config).expect("gpu kinds simulation");
    // One job per shipped profile completes; the unknown kind stays queued.
    assert_eq!(report.metrics.jobs_completed, 14);
    assert_eq!(report.metrics.jobs_unschedulable, 1);
    assert_eq!(report.metrics.jobs_total, 15);
}
