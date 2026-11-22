// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import { useMemo, useState } from "react";
import { motion } from "framer-motion";
import {
  AppLink,
  Button,
  Card,
  FormField,
  PageHero,
  ProgressBar,
  SchedulerPillGroup,
  Select,
} from "@/components/ui";
import { runWhatIf } from "@/lib/api";
import { fadeInUp, staggerContainer } from "@/lib/motion";

type Row = Record<string, unknown>;

function metricsOf(row: Row) {
  return row.metrics as
    | { makespan?: number; ttft_p50?: number; gpu_utilization?: number }
    | null
    | undefined;
}

function costOf(row: Row) {
  return (row.benchmark as { cost_usd?: number } | null | undefined)?.cost_usd;
}

function downloadCsv(rows: Row[]) {
  const headers = ["config", "scheduler", "makespan", "ttft_p50", "gpu_utilization", "cost_usd"];
  const lines = [headers.join(",")];
  for (const row of rows) {
    const m = metricsOf(row);
    lines.push(
      [
        JSON.stringify(String(row.config ?? "")),
        JSON.stringify(String(row.scheduler ?? "")),
        m?.makespan ?? "",
        m?.ttft_p50 ?? "",
        m?.gpu_utilization ?? "",
        costOf(row) ?? "",
      ].join(",")
    );
  }
  const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `zyvor-janus-whatif-${Date.now()}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}

export default function WhatIfPage() {
  const [baseConfig, setBaseConfig] = useState("inference_llama.yaml");
  const [schedulers, setSchedulers] = useState<string[]>(["fifo", "preemptive"]);
  const [rows, setRows] = useState<Row[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sweepNonce, setSweepNonce] = useState(0);

  async function handleSweep() {
    if (schedulers.length < 1) {
      setError("Select at least one scheduler");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const data = await runWhatIf(baseConfig, schedulers);
      setRows(data.results);
      setSweepNonce((n) => n + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Sweep failed");
    } finally {
      setBusy(false);
    }
  }

  const ranges = useMemo(() => {
    const makespans = rows.map((r) => metricsOf(r)?.makespan ?? 0);
    const ttfts = rows.map((r) => metricsOf(r)?.ttft_p50 ?? 0);
    const gpus = rows.map((r) => metricsOf(r)?.gpu_utilization ?? 0);
    const costs = rows.map((r) => costOf(r) ?? 0);
    return {
      makespanMax: Math.max(...makespans, 1),
      ttftMax: Math.max(...ttfts, 0.001),
      gpuMax: Math.max(...gpus, 0.001),
      costMax: Math.max(...costs, 0.001),
      bestMakespan: Math.min(...makespans.filter((n) => n > 0), Infinity),
      bestTtft: Math.min(...ttfts.filter((n) => n > 0), Infinity),
      bestGpu: Math.max(...gpus, 0),
      bestCost: Math.min(...costs.filter((n) => n > 0), Infinity),
    };
  }, [rows]);

  const winners = useMemo(() => {
    if (!rows.length) return null;
    let makespan = "—";
    let ttft = "—";
    let gpu = "—";
    let cost = "—";
    for (const row of rows) {
      const m = metricsOf(row);
      const sch = String(row.scheduler);
      if (m?.makespan != null && m.makespan === ranges.bestMakespan) makespan = sch;
      if (m?.ttft_p50 != null && m.ttft_p50 === ranges.bestTtft) ttft = sch;
      if (m?.gpu_utilization != null && m.gpu_utilization === ranges.bestGpu) gpu = sch;
      if (costOf(row) === ranges.bestCost) cost = sch;
    }
    return { makespan, ttft, gpu, cost };
  }, [rows, ranges]);

  return (
    <div className="space-y-6">
      <PageHero
        kicker="WHAT-IF"
        titleAccent="WHAT-IF"
        title=""
        subtitle="Compare schedulers on the same inference workload."
        status="offline"
        actions={<AppLink href="/benchmark">Benchmark</AppLink>}
      />
      <h2>Actions</h2>
      <Card variant="action" title="▶ Sweep" description="Pick any subset of schedulers to compare on the base config.">
        <FormField label="Base config">
          <Select value={baseConfig} onChange={(e) => setBaseConfig(e.target.value)}>
            <option value="inference_llama.yaml">inference_llama.yaml</option>
            <option value="small_h100.yaml">small_h100.yaml</option>
          </Select>
        </FormField>
        <div className="mt-4">
          <p className="form-label mb-2">Schedulers</p>
          <SchedulerPillGroup multi selected={schedulers} onMultiChange={setSchedulers} />
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button onClick={handleSweep} disabled={busy || schedulers.length === 0}>
            {busy ? "Running sweep…" : `Run sweep (${schedulers.length})`}
          </Button>
          {rows.length > 0 ? (
            <Button variant="secondary" onClick={() => downloadCsv(rows)}>
              Export CSV
            </Button>
          ) : null}
        </div>
        {error ? <p className="mt-3 text-sm text-red-400">{error}</p> : null}
      </Card>

      <Card title="Results matrix" description="Inline bars show relative magnitude within this sweep.">
        <div className="overflow-auto">
          <table className="data-table">
            <thead>
              <tr>
                <th>Config</th>
                <th>Scheduler</th>
                <th>Makespan</th>
                <th>TTFT p50</th>
                <th>GPU util</th>
                <th>Cost USD</th>
              </tr>
            </thead>
            <motion.tbody key={sweepNonce} variants={staggerContainer} initial="hidden" animate="visible">
              {rows.length === 0 ? (
                <tr>
                  <td colSpan={6} className="text-hs-muted">
                    Run a sweep to populate results.
                  </td>
                </tr>
              ) : (
                rows.map((row, idx) => {
                  const metrics = metricsOf(row);
                  const cost = costOf(row);
                  const makespan = metrics?.makespan ?? 0;
                  const ttft = metrics?.ttft_p50 ?? 0;
                  const gpu = metrics?.gpu_utilization ?? 0;
                  return (
                    <motion.tr key={idx} variants={fadeInUp}>
                      <td>{String(row.config)}</td>
                      <td className="font-medium text-hs-heading">{String(row.scheduler)}</td>
                      <td>
                        <div className="compare-cell">
                          <span
                            className={
                              makespan === ranges.bestMakespan ? "compare-cell-value compare-cell-best" : "compare-cell-value"
                            }
                          >
                            {metrics?.makespan?.toFixed(2) ?? "—"}
                          </span>
                          <ProgressBar value={makespan} max={ranges.makespanMax} variant="warn" />
                        </div>
                      </td>
                      <td>
                        <div className="compare-cell">
                          <span
                            className={
                              ttft > 0 && ttft === ranges.bestTtft
                                ? "compare-cell-value compare-cell-best"
                                : "compare-cell-value"
                            }
                          >
                            {metrics?.ttft_p50?.toFixed(3) ?? "—"}
                          </span>
                          <ProgressBar value={ttft} max={ranges.ttftMax} />
                        </div>
                      </td>
                      <td>
                        <div className="compare-cell">
                          <span
                            className={
                              gpu === ranges.bestGpu ? "compare-cell-value compare-cell-best" : "compare-cell-value"
                            }
                          >
                            {metrics ? `${(gpu * 100).toFixed(1)}%` : "—"}
                          </span>
                          <ProgressBar value={gpu} max={ranges.gpuMax} variant="success" />
                        </div>
                      </td>
                      <td>
                        <div className="compare-cell">
                          <span
                            className={
                              cost != null && cost === ranges.bestCost
                                ? "compare-cell-value compare-cell-best"
                                : "compare-cell-value"
                            }
                          >
                            {cost?.toFixed(2) ?? "—"}
                          </span>
                          <ProgressBar value={cost ?? 0} max={ranges.costMax} />
                        </div>
                      </td>
                    </motion.tr>
                  );
                })
              )}
              {winners ? (
                <motion.tr className="winner-row" variants={fadeInUp}>
                  <td colSpan={2}>Winner</td>
                  <td>{winners.makespan}</td>
                  <td>{winners.ttft}</td>
                  <td>{winners.gpu}</td>
                  <td>{winners.cost}</td>
                </motion.tr>
              ) : null}
            </motion.tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}
