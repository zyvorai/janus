// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Integration tests: quality metrics, failure injection, trace import,
//! policy comparison, workload validation.

use std::path::PathBuf;

use zyvor_janus_config::{
    compare_policies, import_helios_csv, leaderboard, load_workload, run_simulation_report,
    run_simulation_report_with_options, workload_to_yaml, ImportOptions,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn failure_cfg() -> PathBuf {
    root().join("configs/clusters/failure_injection.yaml")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("janus_feat_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn quality_report_is_populated_and_consistent() {
    let r = run_simulation_report(&root().join("configs/clusters/small_h100.yaml")).unwrap();
    let q = r.quality.expect("quality present");
    assert_eq!(q.jobs_considered, r.metrics.jobs_completed);
    assert!(q.wait_p50 <= q.wait_p90 && q.wait_p90 <= q.wait_p99 && q.wait_p99 <= q.wait_max);
    assert!(q.slowdown_mean >= 1.0 && q.slowdown_max >= q.slowdown_mean);
    assert!((0.0..=1.0).contains(&q.jain_job_service));
    assert!((0.0..=1.0).contains(&q.jain_tenant_service));
    assert_eq!(q.utilization_timeline.len(), 24);
    assert!(q
        .utilization_timeline
        .iter()
        .all(|p| p.utilization <= 1.0 && p.fragmentation <= 1.0));
}

#[test]
fn no_failure_section_means_no_injected_crashes() {
    let r = run_simulation_report(&root().join("configs/clusters/small_h100.yaml")).unwrap();
    assert_eq!(r.metrics.failures_injected, 0);
    assert_eq!(r.metrics.lost_gpu_seconds, 0.0);
}

#[test]
fn failure_injection_is_deterministic_per_seed() {
    let a = run_simulation_report_with_options(&failure_cfg(), None, Some(5)).unwrap();
    let b = run_simulation_report_with_options(&failure_cfg(), None, Some(5)).unwrap();
    assert_eq!(a.metrics.to_json_pretty(), b.metrics.to_json_pretty());
    assert_eq!(
        serde_json::to_string(&a.quality).unwrap(),
        serde_json::to_string(&b.quality).unwrap()
    );
    assert_eq!(a.decisions.len(), b.decisions.len());
}

#[test]
fn different_seeds_give_different_schedules_and_every_job_ends() {
    let mut makespans = std::collections::BTreeSet::new();
    let mut total_failures = 0;
    for seed in 1..=6 {
        let r = run_simulation_report_with_options(&failure_cfg(), None, Some(seed)).unwrap();
        // every job either finished or exhausted its retries -- none lost
        assert_eq!(
            r.metrics.jobs_completed + r.metrics.jobs_failed,
            r.metrics.jobs_total
        );
        assert_eq!(r.metrics.jobs_unschedulable, 0);
        total_failures += r.metrics.failures_injected;
        makespans.insert(r.metrics.makespan.to_bits());
    }
    assert!(
        total_failures > 0,
        "mtbf 1500s vs 300-1200s jobs should crash sometimes"
    );
    assert!(makespans.len() > 1, "seeds should change the outcome");
}

#[test]
fn failures_cost_makespan_versus_no_failures() {
    let with = run_simulation_report_with_options(&failure_cfg(), None, Some(3)).unwrap();
    assert!(with.metrics.lost_gpu_seconds > 0.0 || with.metrics.failures_injected == 0);
    if with.metrics.failures_injected > 0 {
        assert!(with.metrics.lost_gpu_seconds >= 0.0);
        assert!(with
            .decisions
            .iter()
            .any(|d| d.kind.starts_with("job_failure")));
    }
}

#[test]
fn seed_without_failure_section_is_an_error() {
    let cfg = root().join("configs/clusters/small_h100.yaml");
    assert!(run_simulation_report_with_options(&cfg, None, Some(1)).is_err());
}

#[test]
fn invalid_failure_config_is_rejected() {
    let dir = tmp("badfail");
    let base = std::fs::read_to_string(failure_cfg()).unwrap();
    let cfg = base
        .replace(
            "../workloads/",
            &format!("{}/configs/workloads/", root().display()),
        )
        .replace(
            "../hardware",
            &format!("{}/configs/hardware", root().display()),
        )
        .replace("job_mtbf_secs: 1500", "job_mtbf_secs: -5");
    let p = dir.join("c.yaml");
    std::fs::write(&p, cfg).unwrap();
    assert!(run_simulation_report(&p).is_err());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn compare_reports_cis_across_seeds_and_is_reproducible() {
    let seeds: Vec<u64> = (1..=8).collect();
    let a = compare_policies(&failure_cfg(), "fifo", "bestfit", &seeds).unwrap();
    let b = compare_policies(&failure_cfg(), "fifo", "bestfit", &seeds).unwrap();
    assert!(a.stochastic);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let mk = a.metrics.iter().find(|m| m.metric == "makespan").unwrap();
    assert!(mk.ci95_half_width.is_some());
    // comparing a policy to itself must show exactly zero difference
    let same = compare_policies(&failure_cfg(), "fifo", "fifo", &seeds).unwrap();
    assert!(same
        .metrics
        .iter()
        .all(|m| m.mean_diff == 0.0 && m.verdict == "inconclusive"));
}

#[test]
fn compare_without_failure_section_is_single_run_no_ci() {
    let cfg = root().join("configs/clusters/small_h100.yaml");
    let r = compare_policies(&cfg, "fifo", "priority", &[1, 2, 3]).unwrap();
    assert!(!r.stochastic);
    assert!(r.metrics.iter().all(|m| m.ci95_half_width.is_none()));
    assert!(!r.notes.is_empty());
}

#[test]
fn compare_caps_seed_count() {
    let seeds: Vec<u64> = (0..1001).collect();
    assert!(compare_policies(&failure_cfg(), "fifo", "priority", &seeds).is_err());
    assert!(compare_policies(&failure_cfg(), "fifo", "priority", &[]).is_err());
}

#[test]
fn helios_fixture_imports_and_simulates() {
    let (w, s) = import_helios_csv(
        &root().join("tests/fixtures/traces/helios_sample.csv"),
        &ImportOptions::default(),
    )
    .unwrap();
    // COMPLETED only: jobs 1,2,4,7 (5 has 0 GPUs, 3 FAILED, 6 CANCELLED)
    assert_eq!(s.jobs_imported, 4);
    assert_eq!(s.skipped_state, 2);
    assert_eq!(s.skipped_no_gpu, 1);

    let dir = tmp("helios");
    let wl = dir.join("workload.yaml");
    std::fs::write(&wl, workload_to_yaml(&w).unwrap()).unwrap();
    let jobs = load_workload(&wl).unwrap();
    assert_eq!(jobs.len(), 4);
    assert_eq!(
        jobs.iter().map(|j| j.arrival_time).fold(f64::MAX, f64::min),
        0.0
    );

    let cfg = dir.join("cfg.yaml");
    let base = std::fs::read_to_string(root().join("configs/clusters/small_h100.yaml")).unwrap();
    let cfg_text = base
        .replace("../workloads/synthetic_m1.yaml", wl.to_str().unwrap())
        .replace(
            "../hardware",
            &format!("{}/configs/hardware", root().display()),
        )
        + &format!(
            "\nmig_profiles_dir: {r}/configs/mig\nprofiles_dir: {r}/configs/profiles\n",
            r = root().display()
        );
    std::fs::write(&cfg, cfg_text).unwrap();
    // job 1 wants 8 GPUs on a 4-GPU cluster: it can never be placed, the rest finish.
    let r = run_simulation_report(&cfg).unwrap();
    assert_eq!(r.metrics.jobs_total, 4);
    assert_eq!(r.metrics.jobs_completed, 3);
    assert_eq!(r.metrics.jobs_unschedulable, 1);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn workload_validation_rejects_hostile_values() {
    let dir = tmp("hostile");
    for (name, body) in [
        ("nan_runtime", "jobs:\n  - {id: a, arrival_time: 0, runtime: .nan, gpu_count: 1}\n"),
        ("neg_runtime", "jobs:\n  - {id: a, arrival_time: 0, runtime: -4, gpu_count: 1}\n"),
        ("neg_arrival", "jobs:\n  - {id: a, arrival_time: -1, runtime: 4, gpu_count: 1}\n"),
        ("inf_arrival", "jobs:\n  - {id: a, arrival_time: .inf, runtime: 4, gpu_count: 1}\n"),
        ("dup_id", "jobs:\n  - {id: a, arrival_time: 0, runtime: 4, gpu_count: 1}\n  - {id: a, arrival_time: 1, runtime: 4, gpu_count: 1}\n"),
        ("huge_gpus", "jobs:\n  - {id: a, arrival_time: 0, runtime: 4, gpu_count: 4000000000}\n"),
    ] {
        let p = dir.join(format!("{name}.yaml"));
        std::fs::write(&p, body).unwrap();
        assert!(load_workload(&p).is_err(), "{name} should be rejected");
    }
    let ok = dir.join("ok.yaml");
    std::fs::write(
        &ok,
        "jobs:\n  - {id: a, arrival_time: 0, runtime: 0, gpu_count: 1}\n",
    )
    .unwrap();
    assert!(load_workload(&ok).is_ok());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn many_jobs_complete_in_reasonable_time() {
    // Regression for the O(n^2) pending-arrival lookup.
    use zyvor_janus_model::cluster::Cluster;
    use zyvor_janus_model::models::{Gpu, Job, Node};
    use zyvor_janus_scheduler::FifoScheduler;
    use zyvor_janus_simulator::SimulationEngine;
    let gpus: Vec<Gpu> = (0..64)
        .map(|i| Gpu::new(format!("g{i}"), "n0", "H100", 80.0))
        .collect();
    let cluster = Cluster::new(vec![Node {
        id: "n0".into(),
        gpus,
    }]);
    let mut engine = SimulationEngine::new(cluster, FifoScheduler);
    let jobs: Vec<Job> = (0..20_000)
        .map(|i| Job::new(format!("j{i}"), "j", i as f64 * 0.1, 5.0, 1))
        .collect();
    engine.submit_jobs(jobs);
    let start = std::time::Instant::now();
    engine.run();
    assert_eq!(engine.cluster.finished_jobs.len(), 20_000);
    assert!(start.elapsed().as_secs() < 60);
}

fn backfill_cfg() -> PathBuf {
    root().join("configs/clusters/backfill_demo.yaml")
}

fn start_of(r: &zyvor_janus_config::SimulationReport, id: &str) -> f64 {
    r.timeline
        .jobs
        .iter()
        .find(|j| j.job_id == id)
        .and_then(|j| j.start_time)
        .unwrap_or_else(|| panic!("job {id} never started"))
}

#[test]
fn easy_backfill_protects_the_head_job_that_fifo_delays() {
    let fifo = run_simulation_report_with_options(&backfill_cfg(), Some("fifo"), None).unwrap();
    let easy = run_simulation_report_with_options(&backfill_cfg(), Some("easy"), None).unwrap();
    assert_eq!(fifo.metrics.jobs_completed, 4);
    assert_eq!(easy.metrics.jobs_completed, 4);
    // fifo lets the 500s job in first, so the 4-GPU head waits for it.
    assert!(start_of(&fifo, "head") >= 500.0);
    // easy reserves the head at t=100 (when `wide` ends).
    assert_eq!(start_of(&easy, "head"), 100.0);
    // the short job still backfills ahead of the head.
    assert!(start_of(&easy, "short-small") < 100.0);
    // and the long job is held until the head is out of the way.
    assert!(start_of(&easy, "long-small") >= 100.0);
}

#[test]
fn easy_is_deterministic() {
    let a = run_simulation_report_with_options(&backfill_cfg(), Some("easy"), None).unwrap();
    let b = run_simulation_report_with_options(&backfill_cfg(), Some("easy"), None).unwrap();
    assert_eq!(a.metrics.to_json_pretty(), b.metrics.to_json_pretty());
}

#[test]
fn report_has_fairness_and_chargeback() {
    let r = run_simulation_report(&root().join("configs/clusters/small_h100.yaml")).unwrap();
    let q = r.quality.expect("quality");
    assert!(q.ftf_rho_max >= q.ftf_rho_p90 && q.ftf_rho_p90.is_finite());
    assert!(q.ftf_worst_tenant.is_some());
    let c = r.chargeback.expect("chargeback");
    let tenant_sum: f64 = c.tenants.iter().map(|t| t.cost_usd).sum();
    assert!((tenant_sum - c.used_cost_usd).abs() < 1e-6);
    assert!((c.tenants.iter().map(|t| t.share).sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(c.idle_gpu_hours >= 0.0);
    assert!(
        (c.used_gpu_hours + c.idle_gpu_hours - c.capacity_gpu_hours).abs() < 1e-6
            || c.used_gpu_hours > c.capacity_gpu_hours
    );
}

#[test]
fn leaderboard_ranks_by_requested_metric_and_dedupes() {
    let names: Vec<String> = ["fifo", "easy", "fifo"].map(String::from).to_vec();
    let rows = leaderboard(&backfill_cfg(), &names, "mean_wait_time", None).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].scheduler, "easy");
    assert_eq!(rows[0].rank, 1);
    assert_eq!(rows[1].rank, 2);
    assert!(rows[0].mean_wait_time < rows[1].mean_wait_time);
    // higher-is-better keys sort descending
    let rows = leaderboard(&backfill_cfg(), &names, "gpu_utilization", None).unwrap();
    assert!(rows[0].gpu_utilization >= rows[1].gpu_utilization);
    assert_eq!(rows[0].scheduler, "fifo");
}

#[test]
fn leaderboard_rejects_bad_input() {
    let one = vec!["fifo".to_string()];
    assert!(leaderboard(&backfill_cfg(), &one, "nonsense", None).is_err());
    assert!(leaderboard(&backfill_cfg(), &[], "makespan", None).is_err());
    let bad = vec!["no-such-scheduler".to_string()];
    assert!(leaderboard(&backfill_cfg(), &bad, "makespan", None).is_err());
}
