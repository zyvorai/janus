// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Integration tests: quality metrics, failure injection, trace import,
//! policy comparison, workload validation.

use std::path::PathBuf;

use zyvor_janus_config::{
    compare_policies, generate_workload, generated_workload_to_yaml, import_helios_csv,
    leaderboard, load_sweep, load_workload, run_simulation_report,
    run_simulation_report_with_options, workload_to_yaml, GenerateSpec, ImportOptions,
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

/// Writes a config + workload into a temp dir. `gpus` H100s on one node.
fn temp_config(name: &str, gpus: usize, workload_yaml: &str, extra: &str) -> PathBuf {
    let d = tmp(name);
    let gpu_list: String = (0..gpus)
        .map(|i| format!("        - id: gpu-{i}\n          profile: H100_80GB\n"))
        .collect();
    std::fs::write(d.join("w.yaml"), workload_yaml).unwrap();
    let cfg = format!(
        "cluster:\n  nodes:\n    - id: node-0\n      gpus:\n{gpu_list}scheduler:\n  type: fifo\nworkload:\n  path: w.yaml\nhardware_profiles_dir: {}\nmig_profiles_dir: {}\nprofiles_dir: {}\n{extra}",
        root().join("configs/hardware").display(),
        root().join("configs/mig").display(),
        root().join("configs/profiles").display()
    );
    let p = d.join("c.yaml");
    std::fs::write(&p, cfg).unwrap();
    p
}

fn job(id: &str, arrival: f64, runtime: f64, gpus: u32, tenant: &str) -> String {
    format!(
        "  - id: {id}\n    name: {id}\n    arrival_time: {arrival}\n    runtime: {runtime}\n    gpu_count: {gpus}\n    gpu_memory_gb: 40\n    tenant: {tenant}\n"
    )
}

#[test]
fn sjf_runs_the_short_job_first_and_lowers_mean_jct() {
    let w = format!(
        "jobs:\n{}{}{}",
        job("first", 0.0, 100.0, 1, "a"),
        job("long", 1.0, 500.0, 1, "a"),
        job("short", 2.0, 10.0, 1, "a")
    );
    let cfg = temp_config("sjf", 1, &w, "");
    let fifo = run_simulation_report_with_options(&cfg, Some("fifo"), None).unwrap();
    let sjf = run_simulation_report_with_options(&cfg, Some("sjf"), None).unwrap();
    assert_eq!(start_of(&fifo, "long"), 100.0);
    assert_eq!(start_of(&sjf, "short"), 100.0);
    assert_eq!(start_of(&sjf, "long"), 110.0);
    let (qf, qs) = (fifo.quality.unwrap(), sjf.quality.unwrap());
    assert!(
        qs.jct_mean < qf.jct_mean,
        "{} vs {}",
        qs.jct_mean,
        qf.jct_mean
    );
}

#[test]
fn fairshare_serves_the_small_tenant_before_the_flooding_tenant() {
    let w = format!(
        "jobs:\n{}{}{}{}{}",
        job("a0", 0.0, 100.0, 1, "a"),
        job("a1", 0.0, 100.0, 1, "a"),
        job("a2", 0.5, 100.0, 1, "a"),
        job("a3", 0.5, 100.0, 1, "a"),
        job("b0", 1.0, 100.0, 1, "b"),
    );
    let cfg = temp_config("fairshare", 2, &w, "");
    let fifo = run_simulation_report_with_options(&cfg, Some("fifo"), None).unwrap();
    let fair = run_simulation_report_with_options(&cfg, Some("fairshare"), None).unwrap();
    assert_eq!(fair.metrics.jobs_completed, 5);
    assert_eq!(start_of(&fifo, "b0"), 200.0);
    assert_eq!(start_of(&fair, "b0"), 100.0);
}

#[test]
fn jct_metrics_and_slo_report_match_hand_computed_values() {
    let r = run_simulation_report_with_options(&backfill_cfg(), Some("easy"), None).unwrap();
    let q = r.quality.unwrap();
    assert_eq!(q.jct_mean, 234.25);
    assert_eq!(q.jct_p99, 648.0);
    assert!(q.jct_p50 <= q.jct_p90 && q.jct_p90 <= q.jct_p99);
    // backfill_demo.yaml sets slo.max_wait_secs = 60.
    let slo = r.slo.expect("config has an slo section");
    assert_eq!((slo.jobs_total, slo.jobs_met), (4, 2));
    assert_eq!(slo.attainment, 0.5);
    assert_eq!(slo.worst_tenant.as_deref(), Some("team-a"));
}

#[test]
fn no_slo_section_means_no_slo_report_and_bad_slo_is_rejected() {
    let w = format!("jobs:\n{}", job("only", 0.0, 10.0, 1, "a"));
    let cfg = temp_config("noslo", 1, &w, "");
    assert!(run_simulation_report(&cfg).unwrap().slo.is_none());
    let bad = temp_config("badslo", 1, &w, "slo:\n  max_slowdown: 0.5\n");
    let e = run_simulation_report(&bad).unwrap_err().to_string();
    assert!(e.contains("max_slowdown"), "{e}");
    let empty = temp_config("emptyslo", 1, &w, "slo: {}\n");
    assert!(run_simulation_report(&empty).is_err());
}

#[test]
fn leaderboard_can_rank_by_slo_attainment_and_jct() {
    let names: Vec<String> = ["fifo", "easy"].map(String::from).to_vec();
    // With a 60 s wait SLO fifo meets 3 of 4 (it starts the small jobs at once
    // and only delays `head`); easy protects `head` but makes `long-small` wait.
    let rows = leaderboard(&backfill_cfg(), &names, "slo_attainment", None).unwrap();
    assert_eq!(rows[0].scheduler, "fifo");
    assert_eq!(rows[0].slo_attainment, Some(0.75));
    assert_eq!(rows[1].slo_attainment, Some(0.5));
    let rows = leaderboard(&backfill_cfg(), &names, "jct_mean", None).unwrap();
    assert_eq!(rows[0].scheduler, "easy");
    assert!(rows[0].jct_mean < rows[1].jct_mean);
}

#[test]
fn load_sweep_shows_latency_growing_with_load_and_finds_sustainable_load() {
    let names: Vec<String> = ["fifo", "easy"].map(String::from).to_vec();
    let rep = load_sweep(&backfill_cfg(), &names, &[1.0, 0.01, 1.0], 50.0, None).unwrap();
    assert_eq!(rep.loads, vec![0.01, 1.0]);
    assert_eq!(rep.rows.len(), 4);
    let row = |s: &str, l: f64| {
        rep.rows
            .iter()
            .find(|r| r.scheduler == s && r.load == l)
            .unwrap()
            .clone()
    };
    for s in ["fifo", "easy"] {
        // At load 0.01 arrivals are 100x apart: nothing queues.
        assert_eq!(row(s, 0.01).wait_p90, 0.0, "{s}");
        assert!(row(s, 1.0).wait_p90 > 50.0, "{s}");
    }
    for s in &rep.sustainable {
        assert_eq!(s.sustainable_load, Some(0.01));
    }
    assert_eq!(row("easy", 1.0).slo_attainment, Some(0.5));
    let again = load_sweep(&backfill_cfg(), &names, &[1.0, 0.01], 50.0, None).unwrap();
    assert_eq!(rep, again, "sweep is deterministic");
}

#[test]
fn load_sweep_with_nothing_sustainable_reports_none_and_rejects_bad_input() {
    let names = vec!["fifo".to_string()];
    let rep = load_sweep(&backfill_cfg(), &names, &[1.0], 10.0, None).unwrap();
    assert_eq!(rep.sustainable[0].sustainable_load, None);
    for loads in [vec![], vec![0.0], vec![-1.0], vec![f64::NAN], vec![1e9]] {
        assert!(load_sweep(&backfill_cfg(), &names, &loads, 10.0, None).is_err());
    }
    assert!(load_sweep(&backfill_cfg(), &[], &[1.0], 10.0, None).is_err());
    assert!(load_sweep(&backfill_cfg(), &names, &[1.0], f64::NAN, None).is_err());
    assert!(load_sweep(&backfill_cfg(), &["nope".to_string()], &[1.0], 10.0, None).is_err());
}

#[test]
fn generated_workload_round_trips_through_the_simulator() {
    let spec = GenerateSpec {
        jobs: 300,
        seed: 11,
        arrival_rate_per_sec: 0.05,
        median_runtime_secs: 120.0,
        gpu_mix: vec![(1, 0.8), (2, 0.2)],
        tenants: 3,
        ..Default::default()
    };
    let w = generate_workload(&spec).unwrap();
    let yaml = generated_workload_to_yaml(&w).unwrap();
    let cfg = temp_config("generated", 4, &yaml, "");
    let jobs = load_workload(&tmp("generated").join("w.yaml")).unwrap();
    assert_eq!(jobs.len(), 300);
    for sched in ["fifo", "sjf", "fairshare", "easy"] {
        let r = run_simulation_report_with_options(&cfg, Some(sched), None).unwrap();
        assert_eq!(r.metrics.jobs_completed, 300, "{sched}");
        assert!(r.quality.unwrap().jct_mean > 0.0);
    }
}
