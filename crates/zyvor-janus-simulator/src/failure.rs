// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Seeded failure injection.
//!
//! Model (deliberately simple, see docs/FAILURE_INJECTION.md): every *run* of
//! a job (a job start or restart) draws an independent exponential
//! time-to-crash with mean `job_mtbf_secs`. If the crash lands before the
//! run would complete, a `JobFailure` event is scheduled. The draw is a pure
//! function of `(seed, job id, run generation)` -- not of a shared stream --
//! so results do not depend on event ordering and the same seed and config
//! always produce the same result. This is NOT a model of GPU/node hardware
//! failure: a crash kills one job, never takes a GPU out of service.

use serde::{Deserialize, Serialize};

/// Upper bound on `max_retries` so a hostile config cannot create unbounded events.
pub const MAX_RETRIES_CAP: u32 = 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureConfig {
    pub seed: u64,
    /// Mean time between crashes of a running job, in simulated seconds.
    pub job_mtbf_secs: f64,
    /// Progress is saved every this many seconds of run time (0 = never).
    #[serde(default)]
    pub checkpoint_secs: f64,
    /// Restarts allowed per job; the next crash marks the job failed.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_max_retries() -> u32 {
    3
}

impl FailureConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.job_mtbf_secs.is_finite() || self.job_mtbf_secs <= 0.0 {
            return Err("failure.job_mtbf_secs must be a finite number > 0".into());
        }
        if !self.checkpoint_secs.is_finite() || self.checkpoint_secs < 0.0 {
            return Err("failure.checkpoint_secs must be a finite number >= 0".into());
        }
        if self.max_retries > MAX_RETRIES_CAP {
            return Err(format!("failure.max_retries must be <= {MAX_RETRIES_CAP}"));
        }
        Ok(())
    }

    /// Time into this run at which the job would crash.
    pub fn crash_after(&self, job_id: &str, run_generation: u32) -> f64 {
        let u = unit_uniform(self.seed, job_id, run_generation);
        -self.job_mtbf_secs * (1.0 - u).ln()
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Uniform in [0, 1) from a stateless hash of the inputs.
pub fn unit_uniform(seed: u64, key: &str, generation: u32) -> f64 {
    let h = splitmix64(splitmix64(seed ^ fnv1a(key)) ^ generation as u64);
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(seed: u64) -> FailureConfig {
        FailureConfig {
            seed,
            job_mtbf_secs: 100.0,
            checkpoint_secs: 0.0,
            max_retries: 3,
        }
    }

    #[test]
    fn same_inputs_same_draw() {
        assert_eq!(cfg(7).crash_after("a", 1), cfg(7).crash_after("a", 1));
    }

    #[test]
    fn seed_job_and_generation_change_the_draw() {
        let base = cfg(7).crash_after("a", 1);
        assert_ne!(base, cfg(8).crash_after("a", 1));
        assert_ne!(base, cfg(7).crash_after("b", 1));
        assert_ne!(base, cfg(7).crash_after("a", 2));
    }

    #[test]
    fn draws_are_finite_nonnegative_with_roughly_the_right_mean() {
        let c = cfg(42);
        let n = 20_000;
        let mut sum = 0.0;
        for i in 0..n {
            let t = c.crash_after(&format!("job-{i}"), 1);
            assert!(t.is_finite() && t >= 0.0);
            sum += t;
        }
        let mean = sum / n as f64;
        assert!((mean - 100.0).abs() < 5.0, "mean {mean}");
    }

    #[test]
    fn validation_rejects_hostile_values() {
        for bad in [f64::NAN, -1.0, 0.0, f64::INFINITY] {
            let mut c = cfg(1);
            c.job_mtbf_secs = bad;
            assert!(c.validate().is_err());
        }
        let mut c = cfg(1);
        c.checkpoint_secs = f64::NAN;
        assert!(c.validate().is_err());
        let mut c = cfg(1);
        c.max_retries = MAX_RETRIES_CAP + 1;
        assert!(c.validate().is_err());
        assert!(cfg(1).validate().is_ok());
    }
}
