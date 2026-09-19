// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use zyvor_janus_config::{
    load_cluster_from_config, load_zynera_bundle, run_zynera_bundle_report, run_simulation_report,
    run_trace_file, trace_diff_to_json, SimulationReport,
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
        #[arg(long, default_value = "fifo", value_parser = ["fifo", "priority", "preemptive", "zynera", "bestfit"])]
        scheduler: String,
        /// Write metrics JSON to this path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write jobs timeline JSON for visualization (M8)
        #[arg(long)]
        jobs_output: Option<PathBuf>,
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
        #[arg(long, default_value = "fifo", value_parser = ["fifo", "priority", "preemptive", "zynera", "bestfit"])]
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

fn print_report(report: SimulationReport, output: Option<PathBuf>, jobs_output: Option<PathBuf>) {
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
                run_simulation_report(&config)
            } else {
                eprintln!("error: provide --config or --zynera-bundle");
                std::process::exit(1);
            }
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });

            print_report(report, output, jobs_output);
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
