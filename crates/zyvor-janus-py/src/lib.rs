// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use zyvor_janus_config::{load_rl_session, run_simulation, run_simulation_report_with_scheduler};
use zyvor_janus_metrics::SimulationMetrics;
use zyvor_janus_simulator::rl::RlSession;
use zyvor_janus_simulator::ClusterSnapshot;

#[pyclass]
#[derive(Clone)]
struct SimResult {
    #[pyo3(get)]
    makespan: f64,
    #[pyo3(get)]
    mean_wait_time: f64,
    #[pyo3(get)]
    gpu_utilization: f64,
    #[pyo3(get)]
    jobs_completed: usize,
    #[pyo3(get)]
    jobs_total: usize,
    #[pyo3(get)]
    mig_reconfigs: u32,
    #[pyo3(get)]
    preemptions: u32,
    #[pyo3(get)]
    topology_penalties: u32,
    #[pyo3(get)]
    topology_runtime_inflation: f64,
    #[pyo3(get)]
    jobs_failed: usize,
    #[pyo3(get)]
    jobs_unschedulable: usize,
    #[pyo3(get)]
    queue_max_length: usize,
}

impl From<SimulationMetrics> for SimResult {
    fn from(m: SimulationMetrics) -> Self {
        Self {
            makespan: m.makespan,
            mean_wait_time: m.mean_wait_time,
            gpu_utilization: m.gpu_utilization,
            jobs_completed: m.jobs_completed,
            jobs_total: m.jobs_total,
            mig_reconfigs: m.mig_reconfigs,
            preemptions: m.preemptions,
            topology_penalties: m.topology_penalties,
            topology_runtime_inflation: m.topology_runtime_inflation,
            jobs_failed: m.jobs_failed,
            jobs_unschedulable: m.jobs_unschedulable,
            queue_max_length: m.queue_max_length,
        }
    }
}

#[pymethods]
impl SimResult {
    fn __repr__(&self) -> String {
        format!(
            "SimResult(makespan={:.2}, util={:.1}%, jobs={}/{}, failed={})",
            self.makespan,
            self.gpu_utilization * 100.0,
            self.jobs_completed,
            self.jobs_total,
            self.jobs_failed
        )
    }

    fn to_json(&self) -> String {
        let m = SimulationMetrics {
            makespan: self.makespan,
            mean_wait_time: self.mean_wait_time,
            gpu_utilization: self.gpu_utilization,
            jobs_completed: self.jobs_completed,
            jobs_total: self.jobs_total,
            queue_max_length: self.queue_max_length,
            mean_cumulative_wait_time: self.mean_wait_time,
            jobs_unschedulable: self.jobs_unschedulable,
            mig_reconfigs: self.mig_reconfigs,
            preemptions: self.preemptions,
            topology_penalties: self.topology_penalties,
            topology_runtime_inflation: self.topology_runtime_inflation,
            jobs_failed: self.jobs_failed,
            ..Default::default()
        };
        m.to_json_pretty()
    }
}

#[pyclass]
struct SimSession {
    inner: RlSession,
}

#[pymethods]
impl SimSession {
    #[new]
    fn new(config_path: &str) -> PyResult<Self> {
        let session = load_rl_session(std::path::Path::new(config_path))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { inner: session })
    }

    #[getter]
    fn obs_size(&self) -> usize {
        self.inner.obs_size()
    }

    #[getter]
    fn action_space_n(&self) -> usize {
        self.inner.action_space_n()
    }

    #[getter]
    fn top_k(&self) -> usize {
        self.inner.top_k
    }

    #[getter]
    fn is_done(&self) -> bool {
        self.inner.is_done()
    }

    fn reset(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snap = self.inner.reset();
        snapshot_to_py(py, &snap)
    }

    fn observe(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        snapshot_to_py(py, &self.inner.observe())
    }

    fn step(&mut self, py: Python<'_>, action: usize) -> PyResult<Py<PyAny>> {
        let result = self.inner.step(action);
        step_result_to_py(py, &result)
    }

    fn step_fifo(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let result = self.inner.step_fifo();
        step_result_to_py(py, &result)
    }

    fn metrics(&self) -> SimResult {
        SimResult::from(SimulationMetrics::from_cluster(
            &self.inner.cluster,
            self.inner.jobs_total,
        ))
    }

    fn decisions(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        decisions_to_py(py, self.inner.decisions())
    }
}

fn job_snapshot_to_py(
    py: Python<'_>,
    job: &zyvor_janus_simulator::JobSnapshot,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("id", &job.id)?;
    dict.set_item("name", &job.name)?;
    dict.set_item("arrival_time", job.arrival_time)?;
    dict.set_item("runtime", job.runtime)?;
    dict.set_item("gpu_count", job.gpu_count)?;
    dict.set_item("priority", job.priority)?;
    dict.set_item("tenant", &job.tenant)?;
    dict.set_item("state", &job.state)?;
    dict.set_item("wait_proxy", job.wait_proxy)?;
    dict.set_item("placeable", job.placeable)?;
    Ok(dict.into())
}

fn snapshot_to_py(py: Python<'_>, snap: &ClusterSnapshot) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("clock", snap.clock)?;
    dict.set_item("free_gpus", snap.free_gpus)?;
    dict.set_item("waiting", snap.waiting)?;
    dict.set_item("running", snap.running)?;
    dict.set_item("finished", snap.finished)?;
    dict.set_item("node_count", snap.node_count)?;
    dict.set_item("gpu_count", snap.gpu_count)?;
    dict.set_item("features", snap.to_feature_vector())?;

    let queue_jobs = PyList::empty(py);
    for job in &snap.queue_jobs {
        queue_jobs.append(job_snapshot_to_py(py, job)?)?;
    }
    dict.set_item("queue_jobs", queue_jobs)?;

    let top_jobs = PyList::empty(py);
    for job in &snap.top_jobs {
        top_jobs.append(job_snapshot_to_py(py, job)?)?;
    }
    dict.set_item("top_jobs", top_jobs)?;

    let running_jobs = PyList::empty(py);
    for job in &snap.running_jobs {
        let j = PyDict::new(py);
        j.set_item("id", &job.id)?;
        j.set_item("name", &job.name)?;
        j.set_item("gpu_count", job.gpu_count)?;
        j.set_item("assigned_gpus", &job.assigned_gpus)?;
        j.set_item("priority", job.priority)?;
        j.set_item("tenant", &job.tenant)?;
        running_jobs.append(j)?;
    }
    dict.set_item("running_jobs", running_jobs)?;

    let nodes = PyList::empty(py);
    for node in &snap.nodes {
        let n = PyDict::new(py);
        n.set_item("id", &node.id)?;
        let gpus = PyList::empty(py);
        for gpu in &node.gpus {
            let g = PyDict::new(py);
            g.set_item("id", &gpu.id)?;
            g.set_item("node_id", &gpu.node_id)?;
            g.set_item("busy", gpu.busy)?;
            g.set_item("utilization", gpu.utilization)?;
            g.set_item("job_id", &gpu.job_id)?;
            g.set_item("job_name", &gpu.job_name)?;
            g.set_item("nvlink_group", gpu.nvlink_group)?;
            gpus.append(g)?;
        }
        n.set_item("gpus", gpus)?;
        nodes.append(n)?;
    }
    dict.set_item("nodes", nodes)?;

    Ok(dict.into())
}

fn step_result_to_py(
    py: Python<'_>,
    result: &zyvor_janus_simulator::StepResult,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("observation", snapshot_to_py(py, &result.observation)?)?;
    dict.set_item("reward", result.reward)?;
    dict.set_item("done", result.done)?;
    dict.set_item("placed", result.placed)?;
    dict.set_item("invalid_action", result.invalid_action)?;
    Ok(dict.into())
}

fn decisions_to_py(
    py: Python<'_>,
    decisions: &[zyvor_janus_core::SchedulerDecision],
) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for d in decisions {
        let dict = PyDict::new(py);
        dict.set_item("time", d.time)?;
        dict.set_item("kind", &d.kind)?;
        dict.set_item("job_id", &d.job_id)?;
        dict.set_item("job_name", &d.job_name)?;
        dict.set_item("gpu_ids", &d.gpu_ids)?;
        dict.set_item("message", &d.message)?;
        list.append(dict)?;
    }
    Ok(list.into())
}

#[pyfunction]
fn run_from_config(config_path: &str) -> PyResult<SimResult> {
    let metrics = run_simulation(std::path::Path::new(config_path))
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(SimResult::from(metrics))
}

#[pyfunction]
#[pyo3(signature = (config_path, scheduler=None))]
fn run_report_from_config(
    py: Python<'_>,
    config_path: &str,
    scheduler: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let report = run_simulation_report_with_scheduler(std::path::Path::new(config_path), scheduler)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let dict = PyDict::new(py);
    dict.set_item("metrics", SimResult::from(report.metrics))?;
    dict.set_item("timeline", report.timeline.to_json_pretty())?;
    dict.set_item("decisions", decisions_to_py(py, &report.decisions)?)?;
    let snapshots = PyList::empty(py);
    for snap in &report.snapshots {
        snapshots.append(snapshot_to_py(py, snap)?)?;
    }
    dict.set_item("snapshots", snapshots)?;
    dict.set_item("scheduler", &report.scheduler)?;
    dict.set_item("config_hash", &report.config_hash)?;
    if let Some(benchmark) = &report.benchmark {
        dict.set_item("benchmark", benchmark.to_json_pretty())?;
    }
    Ok(dict.into())
}

#[pymodule]
fn _zyvor_janus(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SimResult>()?;
    m.add_class::<SimSession>()?;
    m.add_function(wrap_pyfunction!(run_from_config, m)?)?;
    m.add_function(wrap_pyfunction!(run_report_from_config, m)?)?;
    Ok(())
}
