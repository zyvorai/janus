// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use zyvor_janus_config::{
    compare_policies, generate_workload, generated_workload_to_yaml, import_helios_csv,
    leaderboard, load_cluster_from_config, load_sweep, load_zynera_bundle, parse_gpu_mix,
    run_simulation_report_with_options, run_trace_file, run_zynera_bundle_report,
    trace_diff_to_json, workload_to_yaml, GenerateSpec, ImportOptions, SimulationReport,
};

#[derive(Parser)]
#[command(
    name = "zyvor-janus",
    about = "Zyvor Janus GPU cluster scheduler simulator"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a simulation
    Run {
        /// Path to simulation config YAML (internal format)
        #[arg(short, long, conflicts_with = "zynera_bundle")]
        config: Option<PathBuf>,
        /// Path to Zynera export bundle directory
        #[arg(long)]
        zynera_bundle: Option<PathBuf>,
        /// Calibrated model profiles directory
        #[arg(long, default_value = "configs/profiles")]
        profiles_dir: PathBuf,
        /// GPU type to hardware profile registry
        #[arg(long, default_value = "configs/gpu_type_registry.yaml")]
        gpu_type_registry: PathBuf,
        /// Hardware profiles for cluster GPU memory
        #[arg(long, default_value = "configs/hardware")]
        hardware_profiles_dir: PathBuf,
        /// MIG partition profiles directory
        #[arg(long, default_value = "configs/mig")]
        mig_profiles_dir: PathBuf,
        /// Scheduler policy to simulate (with --zynera-bundle; --config uses its own scheduler.type)
        #[arg(long, default_value = "fifo", value_parser = ["fifo", "priority", "preemptive", "zynera", "bestfit", "easy", "sjf", "fairshare"])]
        scheduler: String,
        /// Write metrics JSON to this path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write jobs timeline JSON for visualization (M8)
        #[arg(long)]
        jobs_output: Option<PathBuf>,
        /// Override the failure-injection seed (--config only; the config needs a `failure:` section)
        #[arg(long, requires = "config")]
        seed: Option<u64>,
        /// Write the full report (metrics + quality + timeline) JSON to this path
        #[arg(long)]
        report_output: Option<PathBuf>,
    },
    /// Convert a public job trace into a Janus workload YAML
    ImportTrace {
        /// Trace format
        #[arg(long, default_value = "helios", value_parser = ["helios"])]
        format: String,
        /// Input trace file (Helios cluster_log.csv)
        #[arg(short, long)]
        input: PathBuf,
        /// Output workload YAML
        #[arg(short, long)]
        output: PathBuf,
        /// Accepted job states, comma separated; "all" accepts every state
        #[arg(long, default_value = "COMPLETED")]
        states: String,
        /// Stop after this many jobs
        #[arg(long, default_value_t = 100_000)]
        max_jobs: usize,
        /// Skip jobs asking for more GPUs than this
        #[arg(long, default_value_t = 1024)]
        max_gpus_per_job: u32,
        /// Use the trace's user column as tenant (default: virtual cluster)
        #[arg(long)]
        tenant_from_user: bool,
        /// Divide all times by this factor to compress a long trace
        #[arg(long, default_value_t = 1.0)]
        time_scale: f64,
    },
    /// Compare two scheduler policies on one config, with paired 95% CIs across seeds
    Compare {
        /// Simulation config YAML (add a `failure:` section to get seed variation)
        #[arg(short, long)]
        config: PathBuf,
        #[arg(long, default_value = "fifo", value_parser = ["fifo", "priority", "preemptive", "zynera", "bestfit", "easy", "sjf", "fairshare"])]
        baseline: String,
        #[arg(long, value_parser = ["fifo", "priority", "preemptive", "zynera", "bestfit", "easy", "sjf", "fairshare"])]
        candidate: String,
        /// Number of seeds (first-seed, first-seed+1, ...)
        #[arg(long, default_value_t = 10)]
        seeds: usize,
        #[arg(long, default_value_t = 1)]
        first_seed: u64,
        /// Write the comparison JSON here
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Run one config under several schedulers and rank them side by side
    Leaderboard {
        /// Simulation config YAML
        #[arg(short, long)]
        config: PathBuf,
        /// Schedulers to run, comma separated
        #[arg(long, value_delimiter = ',', default_values_t = ["fifo", "priority", "preemptive", "bestfit", "easy", "sjf", "fairshare"].map(String::from))]
        schedulers: Vec<String>,
        /// Ranking metric: jct_mean, slo_attainment, makespan, mean_wait_time, wait_p99, slowdown_mean, ftf_rho_max, cost_usd, gpu_utilization, jain_tenant_service
        #[arg(long, default_value = "mean_wait_time")]
        sort_by: String,
        /// Failure-injection seed (config needs a `failure:` section)
        #[arg(long)]
        seed: Option<u64>,
        /// Write the leaderboard JSON here
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate a seeded synthetic workload (Poisson arrivals, log-normal runtimes)
    GenerateWorkload {
        /// Number of jobs
        #[arg(long, default_value_t = 100)]
        jobs: usize,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Poisson arrival rate (jobs per simulated second)
        #[arg(long, default_value_t = 0.1)]
        rate: f64,
        /// Median runtime in seconds
        #[arg(long, default_value_t = 600.0)]
        median_runtime: f64,
        /// Log-normal sigma of runtimes (0 = constant)
        #[arg(long, default_value_t = 1.0)]
        sigma: f64,
        #[arg(long, default_value_t = 86400.0)]
        max_runtime: f64,
        /// GPU-size mix as SIZE:WEIGHT,... (default 1:0.7,2:0.15,4:0.1,8:0.05)
        #[arg(long)]
        gpu_mix: Option<String>,
        /// Number of tenants (0 = none)
        #[arg(long, default_value_t = 0)]
        tenants: usize,
        /// Zipf exponent for tenant popularity (0 = uniform)
        #[arg(long, default_value_t = 0.0)]
        tenant_skew: f64,
        /// Distinct priority levels
        #[arg(long, default_value_t = 1)]
        priority_levels: u32,
        /// Output workload YAML
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Replay a config at several offered loads and report latency-vs-load per scheduler
    LoadSweep {
        #[arg(short, long)]
        config: PathBuf,
        #[arg(long, value_delimiter = ',', default_values_t = ["fifo", "easy", "sjf"].map(String::from))]
        schedulers: Vec<String>,
        /// Load factors (arrival-rate multipliers), comma separated
        #[arg(long, value_delimiter = ',', default_values_t = [0.5, 1.0, 2.0, 4.0])]
        loads: Vec<f64>,
        /// A load is sustainable while wait p90 stays within this many seconds
        #[arg(long, default_value_t = 600.0)]
        max_wait_p90: f64,
        #[arg(long)]
        seed: Option<u64>,
        /// Write the sweep JSON here
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Replay a scheduler event trace and compare against a simulated policy
    Replay {
        /// Path to scheduler event trace (JSONL)
        #[arg(long)]
        trace: PathBuf,
        /// Cluster config YAML (internal format)
        #[arg(short, long, conflicts_with = "zynera_bundle")]
        config: Option<PathBuf>,
        /// Zynera export bundle directory (cluster loaded from cluster/)
        #[arg(long)]
        zynera_bundle: Option<PathBuf>,
        /// GPU type to hardware profile registry (with --zynera-bundle)
        #[arg(long, default_value = "configs/gpu_type_registry.yaml")]
        gpu_type_registry: PathBuf,
        /// Hardware profiles for cluster GPU memory (with --zynera-bundle)
        #[arg(long, default_value = "configs/hardware")]
        hardware_profiles_dir: PathBuf,
        /// Calibrated model profiles directory (with --zynera-bundle, unused for trace replay)
        #[arg(long, default_value = "configs/profiles")]
        profiles_dir: PathBuf,
        /// Scheduler policy to simulate
        #[arg(long, default_value = "fifo", value_parser = ["fifo", "priority", "preemptive", "zynera", "bestfit", "easy", "sjf", "fairshare"])]
        scheduler: String,
        /// Write diff report JSON to this path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn print_metrics(metrics: zyvor_janus_metrics::SimulationMetrics, output: Option<PathBuf>) {
    println!("Zyvor Janus results");
    println!("  makespan:          {:.2}", metrics.makespan);
    println!("  mean wait time:    {:.2}", metrics.mean_wait_time);
    println!(
        "  gpu utilization:   {:.2}%",
        metrics.gpu_utilization * 100.0
    );
    println!(
        "  jobs completed:    {}/{}",
        metrics.jobs_completed, metrics.jobs_total
    );
    if metrics.mig_reconfigs > 0 {
        println!("  mig reconfigs:     {}", metrics.mig_reconfigs);
    }
    if metrics.preemptions > 0 {
        println!("  preemptions:       {}", metrics.preemptions);
    }

    if metrics.topology_penalties > 0 {
        println!("  topology penalties: {}", metrics.topology_penalties);
    }
    if metrics.topology_runtime_inflation > 0.0 {
        println!(
            "  topology inflation: {:.2}s",
            metrics.topology_runtime_inflation
        );
    }
    if metrics.jobs_failed > 0 {
        println!("  jobs failed:       {}", metrics.jobs_failed);
    }
    if metrics.failures_injected > 0 {
        println!(
            "  injected crashes:  {} ({:.1} GPU-s lost)",
            metrics.failures_injected, metrics.lost_gpu_seconds
        );
    }

    let json = metrics.to_json_pretty();
    let out = output.unwrap_or_else(|| PathBuf::from("outputs/metrics.json"));
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&out, &json).unwrap_or_else(|e| {
        eprintln!("failed to write output: {e}");
        std::process::exit(1);
    });
    println!("  metrics written:   {}", out.display());
}

fn print_report(
    report: SimulationReport,
    output: Option<PathBuf>,
    jobs_output: Option<PathBuf>,
    report_output: Option<PathBuf>,
) {
    if let Some(q) = &report.quality {
        println!(
            "  wait p50/p90/p99:  {:.2} / {:.2} / {:.2}",
            q.wait_p50, q.wait_p90, q.wait_p99
        );
        println!(
            "  slowdown mean/p99: {:.2} / {:.2}",
            q.slowdown_mean, q.slowdown_p99
        );
        println!(
            "  jain (job/tenant): {:.3} / {:.3}",
            q.jain_job_service, q.jain_tenant_service
        );
        println!("  mean fragmentation: {:.3}", q.mean_fragmentation);
        println!(
            "  ftf rho mean/p90/max: {:.2} / {:.2} / {:.2} (worst tenant: {})",
            q.ftf_rho_mean,
            q.ftf_rho_p90,
            q.ftf_rho_max,
            q.ftf_worst_tenant.as_deref().unwrap_or("-")
        );
    }
    if let Some(q) = &report.quality {
        println!(
            "  jct mean/p50/p90/p99: {:.2} / {:.2} / {:.2} / {:.2}",
            q.jct_mean, q.jct_p50, q.jct_p90, q.jct_p99
        );
    }
    if let Some(s) = &report.slo {
        println!(
            "  slo attainment:    {:.1}% ({}/{} jobs; worst tenant: {})",
            s.attainment * 100.0,
            s.jobs_met,
            s.jobs_total,
            s.worst_tenant.as_deref().unwrap_or("-")
        );
    }
    if let Some(c) = &report.chargeback {
        println!(
            "  gpu-hours used/idle: {:.2} / {:.2} (${:.2} used, ${:.2} idle at ${:.2}/GPU-h)",
            c.used_gpu_hours, c.idle_gpu_hours, c.used_cost_usd, c.idle_cost_usd, c.gpu_hour_usd
        );
        for t in &c.tenants {
            println!(
                "    tenant {:<16} {:>8.2} GPU-h  ${:>9.2}  {:>5.1}%",
                t.tenant,
                t.gpu_hours,
                t.cost_usd,
                t.share * 100.0
            );
        }
    }
    if let Some(path) = report_output {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "scheduler": report.scheduler,
            "metrics": report.metrics,
            "quality": report.quality,
            "chargeback": report.chargeback,
            "slo": report.slo,
        }))
        .unwrap_or_else(|_| "{}".into());
        fs::write(&path, json).unwrap_or_else(|e| {
            eprintln!("failed to write report: {e}");
            std::process::exit(1);
        });
        println!("  report written:    {}", path.display());
    }
    print_metrics(report.metrics, output);

    if let Some(jobs_out) = jobs_output {
        let json = report.timeline.to_json_pretty();
        if let Some(parent) = jobs_out.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&jobs_out, &json).unwrap_or_else(|e| {
            eprintln!("failed to write jobs timeline: {e}");
            std::process::exit(1);
        });
        println!("  timeline written:  {}", jobs_out.display());
    }
}

fn print_trace_report(report: zyvor_janus_config::TraceDiffReport, output: Option<PathBuf>) {
    println!("Zyvor Janus trace replay ({})", report.scheduler);
    println!("  oracle schedules:  {}", report.oracle_schedules);
    println!(
        "  matching:          {}/{}",
        report.matching_placements, report.oracle_schedules
    );
    println!("  differing:         {}", report.differing_placements);
    println!(
        "  sim makespan:      {:.2}",
        report.simulation_metrics.makespan
    );

    for diff in &report.diffs {
        if diff.placement_match && diff.schedule_time_match {
            continue;
        }
        println!(
            "  diff {}: oracle {:?} @ {:.2} vs sim {:?} @ {:.2}",
            diff.job_id,
            diff.oracle.gpu_ids,
            diff.oracle.timestamp,
            diff.simulated.gpu_ids,
            diff.simulated.start_time
        );
    }

    let json = trace_diff_to_json(&report);
    let out = output.unwrap_or_else(|| PathBuf::from("outputs/trace_diff.json"));
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&out, &json).unwrap_or_else(|e| {
        eprintln!("failed to write output: {e}");
        std::process::exit(1);
    });
    println!("  report written:    {}", out.display());
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            config,
            zynera_bundle,
            profiles_dir,
            gpu_type_registry,
            hardware_profiles_dir,
            mig_profiles_dir,
            scheduler,
            output,
            jobs_output,
            seed,
            report_output,
        } => {
            let report = if let Some(bundle) = zynera_bundle {
                run_zynera_bundle_report(
                    &bundle,
                    &profiles_dir,
                    &gpu_type_registry,
                    &hardware_profiles_dir,
                    &mig_profiles_dir,
                    &scheduler,
                )
            } else if let Some(config) = config {
                run_simulation_report_with_options(&config, None, seed)
            } else {
                eprintln!("error: provide --config or --zynera-bundle");
                std::process::exit(1);
            }
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });

            print_report(report, output, jobs_output, report_output);
        }
        Commands::ImportTrace {
            format: _,
            input,
            output,
            states,
            max_jobs,
            max_gpus_per_job,
            tenant_from_user,
            time_scale,
        } => {
            let states: Vec<String> = if states.eq_ignore_ascii_case("all") {
                Vec::new()
            } else {
                states.split(',').map(|s| s.trim().to_string()).collect()
            };
            let opts = ImportOptions {
                max_jobs,
                max_gpus_per_job,
                states,
                tenant_from_user,
                time_scale,
            };
            let (workload, summary) = import_helios_csv(&input, &opts).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });
            let yaml = workload_to_yaml(&workload).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });
            if let Some(parent) = output.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&output, yaml).unwrap_or_else(|e| {
                eprintln!("failed to write workload: {e}");
                std::process::exit(1);
            });
            println!("Zyvor Janus trace import");
            println!("  rows read:         {}", summary.rows_read);
            println!("  jobs imported:     {}", summary.jobs_imported);
            println!(
                "  skipped:           {} state, {} no-gpu, {} too-many-gpus, {} bad-time, {} malformed, {} duplicate",
                summary.skipped_state,
                summary.skipped_no_gpu,
                summary.skipped_too_many_gpus,
                summary.skipped_bad_time,
                summary.skipped_malformed,
                summary.skipped_duplicate_id
            );
            if summary.truncated_at_max_jobs {
                println!("  truncated at --max-jobs {max_jobs}");
            }
            for e in &summary.first_errors {
                println!("  note: {e}");
            }
            println!("  workload written:  {}", output.display());
        }
        Commands::Compare {
            config,
            baseline,
            candidate,
            seeds,
            first_seed,
            output,
        } => {
            let seed_list: Vec<u64> = (0..seeds as u64)
                .map(|i| first_seed.wrapping_add(i))
                .collect();
            let report = compare_policies(&config, &baseline, &candidate, &seed_list)
                .unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                });
            println!(
                "Zyvor Janus policy comparison: {} (baseline) vs {} (candidate), {} run(s)",
                report.baseline,
                report.candidate,
                report.seeds.len().max(1)
            );
            for n in &report.notes {
                println!("  note: {n}");
            }
            for m in &report.metrics {
                match m.ci95_half_width {
                    Some(h) => println!(
                        "  {:<22} {:>12.4} -> {:>12.4}  diff {:+.4} +/- {:.4}  {}",
                        m.metric, m.baseline_mean, m.candidate_mean, m.mean_diff, h, m.verdict
                    ),
                    None => println!(
                        "  {:<22} {:>12.4} -> {:>12.4}  diff {:+.4}",
                        m.metric, m.baseline_mean, m.candidate_mean, m.mean_diff
                    ),
                }
            }
            if let Some(out) = output {
                if let Some(parent) = out.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let json = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into());
                fs::write(&out, json).unwrap_or_else(|e| {
                    eprintln!("failed to write output: {e}");
                    std::process::exit(1);
                });
                println!("  comparison written: {}", out.display());
            }
        }
        Commands::Leaderboard {
            config,
            schedulers,
            sort_by,
            seed,
            output,
        } => {
            let rows = leaderboard(&config, &schedulers, &sort_by, seed).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });
            println!("Zyvor Janus leaderboard (sorted by {sort_by})");
            println!(
                "  {:<4} {:<11} {:>10} {:>10} {:>10} {:>10} {:>9} {:>6} {:>7} {:>8} {:>10} {:>7}",
                "rank",
                "scheduler",
                "makespan",
                "mean_wait",
                "wait_p99",
                "jct_mean",
                "slowdown",
                "util",
                "jain",
                "ftf_max",
                "cost_usd",
                "slo"
            );
            for r in &rows {
                println!(
                    "  {:<4} {:<11} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>9.2} {:>5.1}% {:>7.3} {:>8.2} {:>10.2} {:>7}",
                    r.rank,
                    r.scheduler,
                    r.makespan,
                    r.mean_wait_time,
                    r.wait_p99,
                    r.jct_mean,
                    r.slowdown_mean,
                    r.gpu_utilization * 100.0,
                    r.jain_tenant_service,
                    r.ftf_rho_max,
                    r.cost_usd,
                    r.slo_attainment
                        .map(|a| format!("{:.1}%", a * 100.0))
                        .unwrap_or_else(|| "-".into())
                );
            }
            if let Some(out) = output {
                if let Some(parent) = out.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let json = serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into());
                fs::write(&out, json).unwrap_or_else(|e| {
                    eprintln!("failed to write output: {e}");
                    std::process::exit(1);
                });
                println!("  leaderboard written: {}", out.display());
            }
        }
        Commands::GenerateWorkload {
            jobs,
            seed,
            rate,
            median_runtime,
            sigma,
            max_runtime,
            gpu_mix,
            tenants,
            tenant_skew,
            priority_levels,
            output,
        } => {
            let fail = |e: zyvor_janus_config::ConfigError| -> ! {
                eprintln!("error: {e}");
                std::process::exit(1);
            };
            let mut spec = GenerateSpec {
                jobs,
                seed,
                arrival_rate_per_sec: rate,
                median_runtime_secs: median_runtime,
                runtime_sigma: sigma,
                max_runtime_secs: max_runtime,
                tenants,
                tenant_skew,
                priority_levels,
                ..Default::default()
            };
            if let Some(mix) = gpu_mix {
                spec.gpu_mix = parse_gpu_mix(&mix).unwrap_or_else(|e| fail(e));
            }
            let w = generate_workload(&spec).unwrap_or_else(|e| fail(e));
            let yaml = generated_workload_to_yaml(&w).unwrap_or_else(|e| fail(e));
            if let Some(parent) = output.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&output, yaml).unwrap_or_else(|e| {
                eprintln!("failed to write output: {e}");
                std::process::exit(1);
            });
            println!(
                "Generated {} jobs (seed {seed}, rate {rate}/s, median runtime {median_runtime}s) -> {}",
                w.jobs.len(),
                output.display()
            );
        }
        Commands::LoadSweep {
            config,
            schedulers,
            loads,
            max_wait_p90,
            seed,
            output,
        } => {
            let report = load_sweep(&config, &schedulers, &loads, max_wait_p90, seed)
                .unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                });
            println!("Zyvor Janus load sweep (sustainable while wait p90 <= {max_wait_p90}s)");
            println!(
                "  {:<11} {:>6} {:>10} {:>10} {:>10} {:>6} {:>7}",
                "scheduler", "load", "jct_mean", "jct_p99", "wait_p90", "util", "slo"
            );
            for r in &report.rows {
                println!(
                    "  {:<11} {:>6.2} {:>10.2} {:>10.2} {:>10.2} {:>5.1}% {:>7}",
                    r.scheduler,
                    r.load,
                    r.jct_mean,
                    r.jct_p99,
                    r.wait_p90,
                    r.gpu_utilization * 100.0,
                    r.slo_attainment
                        .map(|a| format!("{:.1}%", a * 100.0))
                        .unwrap_or_else(|| "-".into())
                );
            }
            for s in &report.sustainable {
                match s.sustainable_load {
                    Some(l) => println!("  sustainable load {:<11} {l}", s.scheduler),
                    None => println!(
                        "  sustainable load {:<11} none (broke at lowest load)",
                        s.scheduler
                    ),
                }
            }
            if let Some(out) = output {
                if let Some(parent) = out.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let json = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into());
                fs::write(&out, json).unwrap_or_else(|e| {
                    eprintln!("failed to write output: {e}");
                    std::process::exit(1);
                });
                println!("  sweep written: {}", out.display());
            }
        }
        Commands::Replay {
            trace,
            config,
            zynera_bundle,
            gpu_type_registry,
            hardware_profiles_dir,
            profiles_dir,
            scheduler,
            output,
        } => {
            let cluster = if let Some(bundle) = zynera_bundle {
                load_zynera_bundle(
                    &bundle,
                    &profiles_dir,
                    &gpu_type_registry,
                    &hardware_profiles_dir,
                )
                .map(|b| b.cluster)
            } else if let Some(config) = config {
                load_cluster_from_config(&config)
            } else {
                eprintln!("error: provide --config or --zynera-bundle for cluster topology");
                std::process::exit(1);
            }
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });

            let result = run_trace_file(&trace, cluster, &scheduler).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });

            print_trace_report(result.report, output);
        }
    }
}
