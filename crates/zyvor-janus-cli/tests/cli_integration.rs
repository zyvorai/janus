// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

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
