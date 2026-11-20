// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

/// Read-only port of Python's `TwinStore` (`python/zyvor_janus/benchmarks/twin_store.py`),
/// scoped to what `GET /api/twins` needs. Nothing in this codebase writes twins
/// over HTTP -- the `twins` table is populated by offline AIPerf calibration
/// tooling, so only the read path is ported here.
#[derive(Debug, Clone, Serialize)]
pub struct TwinEntry {
    pub gpu_type: String,
    pub model: String,
    pub ttft_ms: f64,
    pub tps: f64,
    pub throughput: f64,
    pub measured_at: String,
    pub aiperf_run_id: Option<String>,
}

/// Ensures `twins.sqlite` (and its `twins` table) exist at `db_path`, then
/// returns every row ordered by `id`, matching Python's `export_json`.
pub fn list_twins(db_path: &Path) -> rusqlite::Result<Vec<TwinEntry>> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS twins (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            gpu_type TEXT NOT NULL,
            model TEXT NOT NULL,
            ttft_ms REAL NOT NULL,
            tps REAL NOT NULL,
            throughput REAL NOT NULL,
            measured_at TEXT NOT NULL,
            aiperf_run_id TEXT,
            version INTEGER NOT NULL DEFAULT 1
        )",
        [],
    )?;

    let mut stmt = conn.prepare(
        "SELECT gpu_type, model, ttft_ms, tps, throughput, measured_at, aiperf_run_id
         FROM twins ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TwinEntry {
            gpu_type: row.get(0)?,
            model: row.get(1)?,
            ttft_ms: row.get(2)?,
            tps: row.get(3)?,
            throughput: row.get(4)?,
            measured_at: row.get(5)?,
            aiperf_run_id: row.get(6)?,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_returns_empty_list() {
        let dir = tempfile_dir();
        let db_path = dir.join("twins.sqlite");
        let twins = list_twins(&db_path).unwrap();
        assert!(twins.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trips_inserted_rows() {
        let dir = tempfile_dir();
        let db_path = dir.join("twins.sqlite");
        list_twins(&db_path).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO twins (gpu_type, model, ttft_ms, tps, throughput, measured_at, aiperf_run_id, version)
             VALUES ('h100', 'llama-70b', 42.0, 100.0, 5000.0, '2026-01-01T00:00:00Z', 'run-1', 1)",
            [],
        )
        .unwrap();

        let twins = list_twins(&db_path).unwrap();
        assert_eq!(twins.len(), 1);
        assert_eq!(twins[0].gpu_type, "h100");
        assert_eq!(twins[0].aiperf_run_id.as_deref(), Some("run-1"));
        std::fs::remove_dir_all(&dir).ok();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zyvor-janus-api-twin-store-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
