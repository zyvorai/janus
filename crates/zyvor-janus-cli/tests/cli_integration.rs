// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! CLI integration tests: invoke the zyvor-janus binary end-to-end.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn zyvor_janus() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_zyvor-janus"));
    cmd.current_dir(repo_root());
    cmd
}

#[test]
fn cli_run_internal_config_emits_metrics() {
    let config = repo_root().join("configs/clusters/small_h100.yaml");
    if !config.exists() {
        return;
    }
    let output = zyvor_janus()
        .args([
            "run",
            "--config",
            config.to_str().expect("utf8 path"),
            "--output",
            "outputs/test_metrics_cli.json",
        ])
        .output()
        .expect("spawn zyvor-janus");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("jobs completed"));
    assert!(repo_root().join("outputs/test_metrics_cli.json").exists());
}

#[test]
fn cli_run_zynera_bundle_completes() {
    let bundle = repo_root().join("tests/fixtures/zynera");
    if !bundle.exists() {
        return;
    }
    let output = zyvor_janus()
        .args([
            "run",
            "--zynera-bundle",
            bundle.to_str().expect("utf8 path"),
            "--profiles-dir",
            "configs/profiles",
        ])
        .output()
        .expect("spawn zyvor-janus");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("jobs completed"));
}

#[test]
fn cli_replay_trace_reports_zero_diffs() {
    let trace = repo_root().join("tests/fixtures/traces/fifo_match.jsonl");
    let config = repo_root().join("configs/clusters/single_gpu.yaml");
    if !trace.exists() || !config.exists() {
        return;
    }
    let output = zyvor_janus()
        .args([
            "replay",
            "--trace",
            trace.to_str().expect("utf8 path"),
            "--config",
            config.to_str().expect("utf8 path"),
            "--output",
            "outputs/test_trace_diff_cli.json",
        ])
        .output()
        .expect("spawn zyvor-janus replay");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("matching:"));
    assert!(stdout.contains("2/2"));
}

#[test]
fn cli_run_zynera_bundle_with_priority_scheduler_completes() {
    let bundle = repo_root().join("tests/fixtures/zynera");
    if !bundle.exists() {
        return;
    }
    let output = zyvor_janus()
        .args([
            "run",
            "--zynera-bundle",
            bundle.to_str().expect("utf8 path"),
            "--profiles-dir",
            "configs/profiles",
            "--scheduler",
            "priority",
        ])
        .output()
        .expect("spawn zyvor-janus");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("jobs completed"));
}

#[test]
fn cli_run_internal_config_with_preemptive_scheduler_reports_preemptions() {
    let config = repo_root().join("configs/clusters/preemption_preemptive.yaml");
    if !config.exists() {
        return;
    }
    let output = zyvor_janus()
        .args(["run", "--config", config.to_str().expect("utf8 path")])
        .output()
        .expect("spawn zyvor-janus");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("jobs completed"));
    assert!(stdout.contains("preemptions:"));
}

#[test]
fn cli_run_zynera_bundle_with_unknown_scheduler_fails() {
    let bundle = repo_root().join("tests/fixtures/zynera");
    if !bundle.exists() {
        return;
    }
    let output = zyvor_janus()
        .args([
            "run",
            "--zynera-bundle",
            bundle.to_str().expect("utf8 path"),
            "--profiles-dir",
            "configs/profiles",
            "--scheduler",
            "not-a-real-scheduler",
        ])
        .output()
        .expect("spawn zyvor-janus");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-real-scheduler"));
}

#[test]
fn cli_run_mig_config_reports_reconfigs() {
    let config = repo_root().join("configs/clusters/mig_single.yaml");
    if !config.exists() {
        return;
    }
    let output = zyvor_janus()
        .args(["run", "--config", config.to_str().expect("utf8 path")])
        .output()
        .expect("spawn zyvor-janus");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mig reconfigs:"));
}

#[test]
fn cli_run_requires_config_or_bundle() {
    let output = zyvor_janus()
        .args(["run"])
        .output()
        .expect("spawn zyvor-janus");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--config") || stderr.contains("--zynera-bundle"));
}

#[test]
fn cli_leaderboard_ranks_schedulers_and_writes_json() {
    let config = repo_root().join("configs/clusters/backfill_demo.yaml");
    let out = repo_root().join("outputs/test_leaderboard_cli.json");
    let output = zyvor_janus()
        .args([
            "leaderboard",
            "--config",
            config.to_str().expect("utf8 path"),
            "--schedulers",
            "fifo,easy",
            "--sort-by",
            "mean_wait_time",
            "--output",
            out.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("spawn zyvor-janus leaderboard");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let easy_line = stdout.lines().position(|l| l.contains("easy")).unwrap();
    let fifo_line = stdout.lines().position(|l| l.contains(" fifo ")).unwrap();
    assert!(
        easy_line < fifo_line,
        "easy should rank above fifo:\n{stdout}"
    );
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["scheduler"], "easy");
    assert_eq!(rows[0]["rank"], 1);
}

#[test]
fn cli_leaderboard_rejects_unknown_sort_key() {
    let config = repo_root().join("configs/clusters/backfill_demo.yaml");
    let output = zyvor_janus()
        .args([
            "leaderboard",
            "--config",
            config.to_str().expect("utf8 path"),
            "--sort-by",
            "bogus",
        ])
        .output()
        .expect("spawn zyvor-janus leaderboard");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown sort key"));
}

#[test]
fn cli_rejects_unknown_scheduler_name() {
    let config = repo_root().join("configs/clusters/backfill_demo.yaml");
    let output = zyvor_janus()
        .args([
            "compare",
            "--config",
            config.to_str().expect("utf8 path"),
            "--candidate",
            "sjf-magic",
        ])
        .output()
        .expect("spawn zyvor-janus compare");
    assert!(!output.status.success());
}

#[test]
fn cli_compare_easy_against_fifo_and_report_has_chargeback() {
    let config = repo_root().join("configs/clusters/backfill_demo.yaml");
    let out = repo_root().join("outputs/test_compare_easy_cli.json");
    let output = zyvor_janus()
        .args([
            "compare",
            "--config",
            config.to_str().expect("utf8 path"),
            "--candidate",
            "easy",
            "--output",
            out.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("spawn zyvor-janus compare");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("ftf_rho_max"));

    let report = repo_root().join("outputs/test_report_cli.json");
    let output = zyvor_janus()
        .args([
            "run",
            "--config",
            config.to_str().expect("utf8 path"),
            "--report-output",
            report.to_str().expect("utf8 path"),
            "--output",
            "outputs/test_metrics_backfill_cli.json",
        ])
        .output()
        .expect("spawn zyvor-janus run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gpu-hours used/idle"));
    assert!(stdout.contains("ftf rho"));
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert!(v["chargeback"]["tenants"].as_array().unwrap().len() >= 2);
    assert!(v["quality"]["ftf_rho_max"].is_number());
}
