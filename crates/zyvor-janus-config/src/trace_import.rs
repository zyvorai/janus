// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Importer for the public Microsoft Helios `cluster_log.csv` job trace
//! (Hu et al., SC'21, <https://github.com/S-Lab-System-Group/HeliosData>)
//! into a Janus workload file.
//!
//! Field mapping and limits are documented in docs/TRACE_IMPORT.md. The
//! importer is a pure function of its input bytes and options.
//!
//! What it does NOT do: it does not recover per-job GPU type, memory,
//! topology demand or the original scheduler's decisions -- Helios does not
//! record them. `runtime` is the *observed* execution time, which for
//! cancelled/failed jobs is truncated.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use serde::Serialize;

use crate::{ConfigError, ConfigResult, MAX_JOB_GPUS};

/// Largest input file accepted (bytes).
pub const MAX_IMPORT_BYTES: u64 = 1024 * 1024 * 1024;
/// Hard ceiling on imported jobs, whatever `ImportOptions::max_jobs` says.
pub const MAX_IMPORT_JOBS: usize = 2_000_000;

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub max_jobs: usize,
    /// Jobs requesting more GPUs than this are skipped.
    pub max_gpus_per_job: u32,
    /// Accepted `state` values, case-insensitive. Empty accepts every state.
    pub states: Vec<String>,
    /// Use the `user` column as tenant instead of `vc`.
    pub tenant_from_user: bool,
    /// Divide all times by this (compress a weeks-long trace). Must be > 0.
    pub time_scale: f64,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            max_jobs: 100_000,
            max_gpus_per_job: 1024,
            states: vec!["COMPLETED".into()],
            tenant_from_user: false,
            time_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ImportSummary {
    pub rows_read: usize,
    pub jobs_imported: usize,
    pub skipped_malformed: usize,
    pub skipped_state: usize,
    pub skipped_no_gpu: usize,
    pub skipped_too_many_gpus: usize,
    pub skipped_bad_time: usize,
    pub skipped_duplicate_id: usize,
    pub truncated_at_max_jobs: bool,
    /// First few problems seen (capped), for diagnosis.
    pub first_errors: Vec<String>,
    /// Arrival time of the last imported job after normalisation.
    pub arrival_span_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ImportedJob {
    id: String,
    name: String,
    arrival_time: f64,
    runtime: f64,
    gpu_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant: Option<String>,
}

#[derive(Serialize)]
struct WorkloadOut<'a> {
    jobs: &'a [ImportedJob],
}

/// Serialises imported jobs as a workload YAML the simulator loads.
pub struct ImportedWorkload {
    jobs: Vec<ImportedJob>,
}

impl ImportedWorkload {
    pub fn len(&self) -> usize {
        self.jobs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

pub fn workload_to_yaml(w: &ImportedWorkload) -> ConfigResult<String> {
    Ok(serde_yaml::to_string(&WorkloadOut { jobs: &w.jobs })?)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parses seconds as a plain number, or `YYYY-MM-DD[ T]HH:MM:SS[.fff]`.
/// The Helios README does not state the timestamp format [U], so both are
/// accepted. Returns `None` for anything else, including NaN/inf.
pub fn parse_timestamp(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Ok(v) = s.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    let (date, time) = s.split_once([' ', 'T'])?;
    let mut d = date.split('-');
    let (y, mo, da): (i64, i64, i64) = (
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
    );
    if d.next().is_some() || !(1..=12).contains(&mo) || !(1..=31).contains(&da) {
        return None;
    }
    let mut t = time.split(':');
    let h: f64 = t.next()?.parse().ok()?;
    let mi: f64 = t.next()?.parse().ok()?;
    let se: f64 = t.next().unwrap_or("0").parse().ok()?;
    if t.next().is_some() || !(0.0..24.0).contains(&h) || !(0.0..60.0).contains(&mi) {
        return None;
    }
    if !se.is_finite() || !(0.0..61.0).contains(&se) {
        return None;
    }
    Some(days_from_civil(y, mo, da) as f64 * 86_400.0 + h * 3600.0 + mi * 60.0 + se)
}

pub fn import_helios_csv(
    path: &Path,
    opts: &ImportOptions,
) -> ConfigResult<(ImportedWorkload, ImportSummary)> {
    let len = std::fs::metadata(path)?.len();
    if len > MAX_IMPORT_BYTES {
        return Err(ConfigError::Invalid(format!(
            "trace file is {len} bytes; the limit is {MAX_IMPORT_BYTES}"
        )));
    }
    let file = File::open(path)?;
    import_helios_reader(BufReader::new(file.take(MAX_IMPORT_BYTES)), opts)
}

pub fn import_helios_reader<R: BufRead>(
    reader: R,
    opts: &ImportOptions,
) -> ConfigResult<(ImportedWorkload, ImportSummary)> {
    if !opts.time_scale.is_finite() || opts.time_scale <= 0.0 {
        return Err(ConfigError::Invalid(
            "time_scale must be a finite number > 0".into(),
        ));
    }
    if opts.max_jobs == 0 || opts.max_jobs > MAX_IMPORT_JOBS {
        return Err(ConfigError::Invalid(format!(
            "max_jobs must be in 1..={MAX_IMPORT_JOBS}"
        )));
    }
    let max_gpus = opts.max_gpus_per_job.min(MAX_JOB_GPUS);
    let states: HashSet<String> = opts
        .states
        .iter()
        .map(|s| s.trim().to_uppercase())
        .collect();

    let mut lines = reader.lines();
    let header = lines
        .next()
        .ok_or_else(|| ConfigError::Invalid("trace file is empty".into()))?
        .map_err(|e| ConfigError::Invalid(format!("trace is not valid UTF-8 text: {e}")))?;
    let cols: HashMap<String, usize> = header
        .trim_start_matches('\u{feff}')
        .split(',')
        .enumerate()
        .map(|(i, c)| (c.trim().to_lowercase(), i))
        .collect();
    for required in ["job_id", "gpu_num", "state", "submit_time", "duration"] {
        if !cols.contains_key(required) {
            return Err(ConfigError::Invalid(format!(
                "trace header is missing required column '{required}' (expected Helios cluster_log.csv)"
            )));
        }
    }
    let col = |n: &str| cols.get(n).copied();
    let (c_id, c_gpu, c_state, c_submit, c_dur) = (
        cols["job_id"],
        cols["gpu_num"],
        cols["state"],
        cols["submit_time"],
        cols["duration"],
    );
    let c_tenant = if opts.tenant_from_user {
        col("user")
    } else {
        col("vc")
    };
    let width = cols.values().copied().max().unwrap_or(0) + 1;

    let mut summary = ImportSummary::default();
    let note = |s: &mut ImportSummary, msg: String| {
        if s.first_errors.len() < 5 {
            s.first_errors.push(msg);
        }
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut raw: Vec<(f64, ImportedJob)> = Vec::new();

    for (n, line) in lines.enumerate() {
        let lineno = n + 2;
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                summary.rows_read += 1;
                summary.skipped_malformed += 1;
                note(&mut summary, format!("line {lineno}: unreadable ({e})"));
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        summary.rows_read += 1;
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() < width {
            summary.skipped_malformed += 1;
            note(
                &mut summary,
                format!("line {lineno}: {} fields, expected {width}", f.len()),
            );
            continue;
        }
        if !states.is_empty() && !states.contains(&f[c_state].to_uppercase()) {
            summary.skipped_state += 1;
            continue;
        }
        let gpus: u64 = match f[c_gpu].parse::<f64>() {
            Ok(v) if v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v < 1e12 => v as u64,
            _ => {
                summary.skipped_malformed += 1;
                note(
                    &mut summary,
                    format!("line {lineno}: bad gpu_num '{}'", f[c_gpu]),
                );
                continue;
            }
        };
        if gpus == 0 {
            summary.skipped_no_gpu += 1;
            continue;
        }
        if gpus > max_gpus as u64 {
            summary.skipped_too_many_gpus += 1;
            continue;
        }
        let submit = parse_timestamp(f[c_submit]);
        let dur = f[c_dur]
            .parse::<f64>()
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0);
        let (Some(submit), Some(dur)) = (submit, dur) else {
            summary.skipped_bad_time += 1;
            note(
                &mut summary,
                format!(
                    "line {lineno}: bad submit_time '{}' or duration '{}'",
                    f[c_submit], f[c_dur]
                ),
            );
            continue;
        };
        let id = format!("helios-{}", f[c_id]);
        if f[c_id].is_empty() || !seen.insert(id.clone()) {
            summary.skipped_duplicate_id += 1;
            continue;
        }
        if raw.len() >= opts.max_jobs {
            summary.truncated_at_max_jobs = true;
            break;
        }
        raw.push((
            submit,
            ImportedJob {
                id,
                name: "helios-job".into(),
                arrival_time: 0.0,
                runtime: dur / opts.time_scale,
                gpu_count: gpus as u32,
                tenant: c_tenant
                    .map(|c| f[c])
                    .filter(|t| !t.is_empty())
                    .map(str::to_string),
            },
        ));
    }

    let t0 = raw.iter().map(|(t, _)| *t).fold(f64::INFINITY, f64::min);
    let mut jobs: Vec<ImportedJob> = raw
        .into_iter()
        .map(|(t, mut j)| {
            j.arrival_time = (t - t0) / opts.time_scale;
            j
        })
        .collect();
    jobs.sort_by(|a, b| {
        a.arrival_time
            .partial_cmp(&b.arrival_time)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    summary.jobs_imported = jobs.len();
    summary.arrival_span_secs = jobs.last().map(|j| j.arrival_time).unwrap_or(0.0);
    Ok((ImportedWorkload { jobs }, summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const HDR: &str = "job_id,user,vc,gpu_num,cpu_num,node_num,state,submit_time,start_time,end_time,duration,queue\n";

    fn run(body: &str, opts: &ImportOptions) -> ConfigResult<(ImportedWorkload, ImportSummary)> {
        import_helios_reader(Cursor::new(format!("{HDR}{body}")), opts)
    }

    #[test]
    fn maps_fields_and_normalises_time() {
        let body = "1,_u_a,_vc_x,4,8,1,COMPLETED,2020-04-01 00:00:10,2020-04-01 00:00:20,2020-04-01 00:01:20,60,10\n\
                    2,_u_b,_vc_y,1,2,1,COMPLETED,2020-04-01 00:00:00,2020-04-01 00:00:05,2020-04-01 00:00:15,10,5\n";
        let (w, s) = run(body, &ImportOptions::default()).unwrap();
        assert_eq!(s.jobs_imported, 2);
        assert_eq!(w.jobs[0].id, "helios-2");
        assert_eq!(w.jobs[0].arrival_time, 0.0);
        assert_eq!(w.jobs[1].arrival_time, 10.0);
        assert_eq!(w.jobs[1].gpu_count, 4);
        assert_eq!(w.jobs[1].runtime, 60.0);
        assert_eq!(w.jobs[1].tenant.as_deref(), Some("_vc_x"));
    }

    #[test]
    fn tenant_from_user_and_time_scale() {
        let body = "1,_u_a,_vc_x,2,8,1,COMPLETED,100,110,170,60,10\n";
        let o = ImportOptions {
            tenant_from_user: true,
            time_scale: 10.0,
            ..Default::default()
        };
        let (w, _) = run(body, &o).unwrap();
        assert_eq!(w.jobs[0].tenant.as_deref(), Some("_u_a"));
        assert_eq!(w.jobs[0].runtime, 6.0);
    }

    #[test]
    fn skips_with_reasons_and_never_panics_on_hostile_rows() {
        let body = "\
1,u,v,2,1,1,FAILED,0,0,5,5,0\n\
2,u,v,0,1,1,COMPLETED,0,0,5,5,0\n\
3,u,v,99999,1,1,COMPLETED,0,0,5,5,0\n\
4,u,v,2,1,1,COMPLETED,0,0,5,NaN,0\n\
5,u,v,2,1,1,COMPLETED,0,0,5,-3,0\n\
6,u,v,2,1,1,COMPLETED,notatime,0,5,5,0\n\
7,u,v,2,1,1,COMPLETED,0,0,5,inf,0\n\
8,u,v,-1,1,1,COMPLETED,0,0,5,5,0\n\
9,u,v,1.5,1,1,COMPLETED,0,0,5,5,0\n\
10,short\n\
11,u,v,2,1,1,COMPLETED,0,0,5,5,0\n\
11,u,v,2,1,1,COMPLETED,0,0,5,5,0\n\
\n\
12,u,v,1,1,1,COMPLETED,2020-13-45 99:99:99,0,5,5,0\n";
        let (w, s) = run(body, &ImportOptions::default()).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(s.skipped_state, 1);
        assert_eq!(s.skipped_no_gpu, 1);
        assert_eq!(s.skipped_too_many_gpus, 1);
        assert_eq!(s.skipped_bad_time, 5);
        assert_eq!(s.skipped_malformed, 3);
        assert_eq!(s.skipped_duplicate_id, 1);
        assert!(s.first_errors.len() <= 5);
    }

    #[test]
    fn rejects_bad_header_options_and_empty_input() {
        assert!(import_helios_reader(Cursor::new(""), &ImportOptions::default()).is_err());
        assert!(
            import_helios_reader(Cursor::new("a,b\n1,2\n"), &ImportOptions::default()).is_err()
        );
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let o = ImportOptions {
                time_scale: bad,
                ..Default::default()
            };
            assert!(run("", &o).is_err());
        }
        let o = ImportOptions {
            max_jobs: 0,
            ..Default::default()
        };
        assert!(run("", &o).is_err());
        let o = ImportOptions {
            max_jobs: MAX_IMPORT_JOBS + 1,
            ..Default::default()
        };
        assert!(run("", &o).is_err());
    }

    #[test]
    fn max_jobs_caps_a_huge_trace() {
        let mut body = String::new();
        for i in 0..5000 {
            body.push_str(&format!("{i},u,v,1,1,1,COMPLETED,{i},0,5,5,0\n"));
        }
        let o = ImportOptions {
            max_jobs: 100,
            ..Default::default()
        };
        let (w, s) = run(&body, &o).unwrap();
        assert_eq!(w.len(), 100);
        assert!(s.truncated_at_max_jobs);
    }

    #[test]
    fn accepts_all_states_when_list_empty_and_is_deterministic() {
        let body = "1,u,v,1,1,1,FAILED,5,0,9,9,0\n2,u,v,1,1,1,CANCELLED,1,0,9,9,0\n";
        let o = ImportOptions {
            states: vec![],
            ..Default::default()
        };
        let a = workload_to_yaml(&run(body, &o).unwrap().0).unwrap();
        let b = workload_to_yaml(&run(body, &o).unwrap().0).unwrap();
        assert_eq!(a, b);
        assert!(a.contains("helios-1") && a.contains("helios-2"));
    }

    #[test]
    fn timestamp_parser_edge_cases() {
        assert_eq!(parse_timestamp("1970-01-02 00:00:00"), Some(86_400.0));
        assert_eq!(parse_timestamp("1970-01-01T00:00:01.5"), Some(1.5));
        assert_eq!(parse_timestamp("12.5"), Some(12.5));
        for bad in [
            "NaN",
            "inf",
            "",
            "2020-02-30x 00:00:00",
            "2020-01-01 25:00:00",
            "2020-00-10 00:00:00",
        ] {
            assert_eq!(parse_timestamp(bad), None, "{bad}");
        }
    }

    #[test]
    fn imported_yaml_loads_as_a_workload() {
        let body = "1,u,vc1,2,1,1,COMPLETED,0,0,5,30,0\n2,u,vc2,1,1,1,COMPLETED,4,0,5,10,0\n";
        let (w, _) = run(body, &ImportOptions::default()).unwrap();
        let dir = std::env::temp_dir().join(format!("janus_import_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("w.yaml");
        std::fs::write(&p, workload_to_yaml(&w).unwrap()).unwrap();
        let jobs = crate::load_workload(&p).unwrap();
        assert_eq!(jobs.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
