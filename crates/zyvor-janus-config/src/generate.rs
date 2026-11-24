// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Seeded synthetic workload generator.
//!
//! Follows the shape used by the Gavel simulator trace generator
//! (Poisson arrivals at a configurable rate, multi-GPU jobs drawn from a
//! discrete size distribution) with log-normal runtimes, the usual heavy
//! tail of DL job durations in the Helios/Philly/PAI studies. Every draw is
//! a stateless hash of `(seed, stream, job index)`, so the same spec always
//! yields the same workload and no RNG crate version can change it.
//! This is a *shape* generator for what-if studies, not a fitted model of
//! any specific production trace.

use serde::{Deserialize, Serialize};

use crate::{validate_workload, ConfigError, ConfigResult, WorkloadConfig, WorkloadJobSpec};

pub const MAX_GENERATED_JOBS: usize = 100_000;
pub const MAX_GENERATED_TENANTS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateSpec {
    pub jobs: usize,
    #[serde(default)]
    pub seed: u64,
    /// Poisson arrival rate, jobs per simulated second (mean gap = 1/rate).
    pub arrival_rate_per_sec: f64,
    /// Median runtime in seconds (the log-normal location, exp(mu)).
    pub median_runtime_secs: f64,
    /// Log-normal sigma; 0 makes every runtime equal the median.
    #[serde(default = "default_sigma")]
    pub runtime_sigma: f64,
    /// Runtimes are clamped to [1, max_runtime_secs].
    #[serde(default = "default_max_runtime")]
    pub max_runtime_secs: f64,
    /// (gpu_count, weight) pairs; weights need not sum to 1.
    #[serde(default = "default_gpu_mix")]
    pub gpu_mix: Vec<(u32, f64)>,
    /// Number of tenants (0 = jobs carry no tenant).
    #[serde(default)]
    pub tenants: usize,
    /// Zipf exponent for tenant popularity: 0 = uniform, 1 = classic skew.
    #[serde(default)]
    pub tenant_skew: f64,
    /// Distinct priority levels (1 = all jobs equal); level k gets priority 10*k.
    #[serde(default = "default_levels")]
    pub priority_levels: u32,
    #[serde(default = "default_mem")]
    pub gpu_memory_gb: f64,
}

fn default_sigma() -> f64 {
    1.0
}
fn default_max_runtime() -> f64 {
    86_400.0
}
fn default_gpu_mix() -> Vec<(u32, f64)> {
    vec![(1, 0.70), (2, 0.15), (4, 0.10), (8, 0.05)]
}
fn default_levels() -> u32 {
    1
}
fn default_mem() -> f64 {
    40.0
}

impl Default for GenerateSpec {
    fn default() -> Self {
        Self {
            jobs: 100,
            seed: 0,
            arrival_rate_per_sec: 0.1,
            median_runtime_secs: 600.0,
            runtime_sigma: default_sigma(),
            max_runtime_secs: default_max_runtime(),
            gpu_mix: default_gpu_mix(),
            tenants: 0,
            tenant_skew: 0.0,
            priority_levels: 1,
            gpu_memory_gb: default_mem(),
        }
    }
}

impl GenerateSpec {
    pub fn validate(&self) -> ConfigResult<()> {
        let bad = |m: String| Err(ConfigError::Invalid(m));
        if self.jobs == 0 || self.jobs > MAX_GENERATED_JOBS {
            return bad(format!("jobs must be in 1..={MAX_GENERATED_JOBS}"));
        }
        let pos = |v: f64| v.is_finite() && v > 0.0;
        if !pos(self.arrival_rate_per_sec) {
            return bad("arrival_rate_per_sec must be a finite number > 0".into());
        }
        if !pos(self.median_runtime_secs) {
            return bad("median_runtime_secs must be a finite number > 0".into());
        }
        if !self.runtime_sigma.is_finite() || !(0.0..=4.0).contains(&self.runtime_sigma) {
            return bad("runtime_sigma must be within 0..=4".into());
        }
        if !self.max_runtime_secs.is_finite() || self.max_runtime_secs < 1.0 {
            return bad("max_runtime_secs must be a finite number >= 1".into());
        }
        if self.gpu_mix.is_empty() {
            return bad("gpu_mix must not be empty".into());
        }
        for (g, w) in &self.gpu_mix {
            if *g == 0 || *g > crate::MAX_JOB_GPUS {
                return bad(format!(
                    "gpu_mix size {g} must be in 1..={}",
                    crate::MAX_JOB_GPUS
                ));
            }
            if !w.is_finite() || *w < 0.0 {
                return bad("gpu_mix weights must be finite and >= 0".into());
            }
        }
        if self.gpu_mix.iter().map(|(_, w)| w).sum::<f64>() <= 0.0 {
            return bad("gpu_mix weights must not all be zero".into());
        }
        if self.tenants > MAX_GENERATED_TENANTS {
            return bad(format!("tenants must be <= {MAX_GENERATED_TENANTS}"));
        }
        if !self.tenant_skew.is_finite() || !(0.0..=5.0).contains(&self.tenant_skew) {
            return bad("tenant_skew must be within 0..=5".into());
        }
        if self.priority_levels == 0 || self.priority_levels > 100 {
            return bad("priority_levels must be in 1..=100".into());
        }
        if !self.gpu_memory_gb.is_finite() || self.gpu_memory_gb < 0.0 {
            return bad("gpu_memory_gb must be a finite number >= 0".into());
        }
        Ok(())
    }
}

/// Parses `"1:0.7,2:0.2,8:0.1"` into a GPU-size mix.
pub fn parse_gpu_mix(s: &str) -> ConfigResult<Vec<(u32, f64)>> {
    let err =
        |p: &str| ConfigError::Invalid(format!("bad gpu_mix entry '{p}', expected SIZE:WEIGHT"));
    s.split(',')
        .map(|p| {
            let (g, w) = p.trim().split_once(':').ok_or_else(|| err(p))?;
            Ok((
                g.trim().parse().map_err(|_| err(p))?,
                w.trim().parse().map_err(|_| err(p))?,
            ))
        })
        .collect()
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in the open interval (0, 1), stateless in (seed, stream, index).
fn uniform(seed: u64, stream: u64, index: u64) -> f64 {
    let h = splitmix64(splitmix64(seed ^ stream.wrapping_mul(0xA24B_AED4_963E_E407)) ^ index);
    (((h >> 11) as f64) + 0.5) / (1u64 << 53) as f64
}

/// Index drawn proportionally to `weights` (must have positive sum).
fn pick(weights: &[f64], u: f64) -> usize {
    let total: f64 = weights.iter().sum();
    let mut acc = 0.0;
    for (i, w) in weights.iter().enumerate() {
        acc += w / total;
        if u < acc {
            return i;
        }
    }
    weights.len() - 1
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Generates a validated workload. Arrivals are cumulative exponential gaps.
pub fn generate_workload(spec: &GenerateSpec) -> ConfigResult<WorkloadConfig> {
    spec.validate()?;
    let sizes: Vec<u32> = spec.gpu_mix.iter().map(|(g, _)| *g).collect();
    let size_w: Vec<f64> = spec.gpu_mix.iter().map(|(_, w)| *w).collect();
    let tenant_w: Vec<f64> = (0..spec.tenants)
        .map(|i| 1.0 / ((i + 1) as f64).powf(spec.tenant_skew))
        .collect();
    let width = spec.jobs.to_string().len();
    let mut t = 0.0_f64;
    let mut jobs = Vec::with_capacity(spec.jobs);
    for i in 0..spec.jobs {
        let k = i as u64;
        t += -(1.0 - uniform(spec.seed, 1, k)).ln() / spec.arrival_rate_per_sec;
        // Box-Muller from two independent uniforms.
        let (u1, u2) = (uniform(spec.seed, 2, k), uniform(spec.seed, 3, k));
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let runtime = (spec.median_runtime_secs * (spec.runtime_sigma * z).exp())
            .clamp(1.0, spec.max_runtime_secs);
        let gpu_count = sizes[pick(&size_w, uniform(spec.seed, 4, k))];
        let tenant = (spec.tenants > 0)
            .then(|| format!("tenant-{}", pick(&tenant_w, uniform(spec.seed, 5, k))));
        let level = (uniform(spec.seed, 6, k) * spec.priority_levels as f64) as u32;
        let id = format!("gen-{i:0width$}");
        jobs.push(WorkloadJobSpec {
            id: id.clone(),
            name: Some(id),
            arrival_time: round3(t),
            runtime: round3(runtime),
            gpu_count,
            gpu_memory_gb: spec.gpu_memory_gb,
            priority: 10 * level.min(spec.priority_levels - 1),
            tenant,
            network_bw_gbps: None,
            gpu_type: None,
            mig_profile: None,
            mig_count: None,
            gang_enabled: false,
            gang_size_nodes: None,
            gang_timeout_secs: None,
            model_id: None,
            input_tokens: None,
            output_tokens: None,
            batch_size: None,
            concurrency: None,
            request_id: None,
        });
    }
    let w = WorkloadConfig { jobs };
    validate_workload(&w)?;
    Ok(w)
}

/// Serialises a generated workload as the YAML the simulator loads.
pub fn generated_workload_to_yaml(w: &WorkloadConfig) -> ConfigResult<String> {
    #[derive(Serialize)]
    struct Out<'a> {
        jobs: Vec<Row<'a>>,
    }
    #[derive(Serialize)]
    struct Row<'a> {
        id: &'a str,
        name: &'a str,
        arrival_time: f64,
        runtime: f64,
        gpu_count: u32,
        gpu_memory_gb: f64,
        priority: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        tenant: Option<&'a str>,
    }
    let rows = w
        .jobs
        .iter()
        .map(|j| Row {
            id: &j.id,
            name: j.name.as_deref().unwrap_or(&j.id),
            arrival_time: j.arrival_time,
            runtime: j.runtime,
            gpu_count: j.gpu_count,
            gpu_memory_gb: j.gpu_memory_gb,
            priority: j.priority,
            tenant: j.tenant.as_deref(),
        })
        .collect();
    Ok(serde_yaml::to_string(&Out { jobs: rows })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> GenerateSpec {
        GenerateSpec {
            jobs: 2000,
            seed: 7,
            arrival_rate_per_sec: 0.5,
            median_runtime_secs: 300.0,
            tenants: 4,
            tenant_skew: 1.0,
            priority_levels: 3,
            ..Default::default()
        }
    }

    #[test]
    fn same_seed_same_workload_different_seed_differs() {
        let a = generate_workload(&spec()).unwrap();
        let b = generate_workload(&spec()).unwrap();
        assert_eq!(
            generated_workload_to_yaml(&a).unwrap(),
            generated_workload_to_yaml(&b).unwrap()
        );
        let mut s2 = spec();
        s2.seed = 8;
        let c = generate_workload(&s2).unwrap();
        assert_ne!(
            generated_workload_to_yaml(&a).unwrap(),
            generated_workload_to_yaml(&c).unwrap()
        );
    }

    #[test]
    fn arrivals_are_sorted_with_poisson_rate() {
        let w = generate_workload(&spec()).unwrap();
        assert!(w
            .jobs
            .windows(2)
            .all(|p| p[0].arrival_time <= p[1].arrival_time));
        let span = w.jobs.last().unwrap().arrival_time;
        let rate = w.jobs.len() as f64 / span;
        assert!((rate - 0.5).abs() < 0.05, "rate {rate}");
    }

    #[test]
    fn runtime_median_and_gpu_mix_follow_the_spec() {
        let w = generate_workload(&spec()).unwrap();
        let mut rt: Vec<f64> = w.jobs.iter().map(|j| j.runtime).collect();
        rt.sort_by(|a, b| a.total_cmp(b));
        let median = rt[rt.len() / 2];
        assert!((median / 300.0 - 1.0).abs() < 0.15, "median {median}");
        let singles = w.jobs.iter().filter(|j| j.gpu_count == 1).count() as f64;
        assert!((singles / 2000.0 - 0.70).abs() < 0.05);
        assert!(w.jobs.iter().all(|j| [1, 2, 4, 8].contains(&j.gpu_count)));
    }

    #[test]
    fn tenant_skew_and_priority_levels() {
        let w = generate_workload(&spec()).unwrap();
        let count = |t: &str| {
            w.jobs
                .iter()
                .filter(|j| j.tenant.as_deref() == Some(t))
                .count()
        };
        assert!(count("tenant-0") > count("tenant-3") * 2, "zipf skew");
        assert!(w.jobs.iter().all(|j| [0, 10, 20].contains(&j.priority)));
        let mut u = spec();
        u.tenants = 0;
        assert!(generate_workload(&u)
            .unwrap()
            .jobs
            .iter()
            .all(|j| j.tenant.is_none()));
    }

    #[test]
    fn zero_sigma_gives_constant_runtime_and_clamps_hold() {
        let mut s = spec();
        s.jobs = 50;
        s.runtime_sigma = 0.0;
        assert!(generate_workload(&s)
            .unwrap()
            .jobs
            .iter()
            .all(|j| j.runtime == 300.0));
        s.runtime_sigma = 4.0;
        s.max_runtime_secs = 400.0;
        assert!(generate_workload(&s)
            .unwrap()
            .jobs
            .iter()
            .all(|j| (1.0..=400.0).contains(&j.runtime)));
    }

    #[test]
    fn gpu_mix_parser() {
        assert_eq!(
            parse_gpu_mix("1:0.7, 8:0.3").unwrap(),
            vec![(1, 0.7), (8, 0.3)]
        );
        assert!(parse_gpu_mix("1").is_err());
        assert!(parse_gpu_mix("x:1").is_err());
    }

    #[test]
    fn hostile_specs_are_rejected() {
        let ok = spec();
        type Mutation = Box<dyn Fn(&mut GenerateSpec)>;
        let cases: Vec<Mutation> = vec![
            Box::new(|s| s.jobs = 0),
            Box::new(|s| s.jobs = MAX_GENERATED_JOBS + 1),
            Box::new(|s| s.arrival_rate_per_sec = 0.0),
            Box::new(|s| s.arrival_rate_per_sec = f64::NAN),
            Box::new(|s| s.median_runtime_secs = -1.0),
            Box::new(|s| s.runtime_sigma = 9.0),
            Box::new(|s| s.gpu_mix = vec![]),
            Box::new(|s| s.gpu_mix = vec![(0, 1.0)]),
            Box::new(|s| s.gpu_mix = vec![(1, 0.0)]),
            Box::new(|s| s.tenants = MAX_GENERATED_TENANTS + 1),
            Box::new(|s| s.priority_levels = 0),
        ];
        for mutate in cases {
            let mut s = ok.clone();
            mutate(&mut s);
            assert!(generate_workload(&s).is_err(), "{s:?}");
        }
    }
}
