// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import type { SimulationMetrics } from "@/types/simulation";
import { MetricTile } from "./ui";

function cumulativeWait(metrics: SimulationMetrics): number {
  return metrics.mean_cumulative_wait_time ?? metrics.mean_wait_time;
}

export const COMPARE_FIELDS: Array<{
  label: string;
  format: (m: SimulationMetrics) => string;
  lowerIsBetter?: boolean;
}> = [
  { label: "Makespan", format: (m) => `${m.makespan.toFixed(1)}s`, lowerIsBetter: true },
  { label: "GPU Utilization", format: (m) => `${(m.gpu_utilization * 100).toFixed(1)}%`, lowerIsBetter: false },
  {
    label: "Mean Cumulative Wait",
    format: (m) => `${cumulativeWait(m).toFixed(2)}s`,
    lowerIsBetter: true,
  },
  {
    label: "Jobs Completed",
    format: (m) => `${m.jobs_completed}/${m.jobs_total}`,
    lowerIsBetter: false,
  },
  { label: "Preemptions", format: (m) => String(m.preemptions), lowerIsBetter: true },
  { label: "Failed", format: (m) => String(m.jobs_failed), lowerIsBetter: true },
  { label: "Queue Max", format: (m) => String(m.queue_max_length), lowerIsBetter: true },
  {
    label: "Unschedulable",
    format: (m) => String(m.jobs_unschedulable ?? 0),
    lowerIsBetter: true,
  },
  { label: "Topo Penalties", format: (m) => String(m.topology_penalties), lowerIsBetter: true },
  { label: "TTFT p50", format: (m) => `${(m.ttft_p50 ?? 0).toFixed(3)}s`, lowerIsBetter: true },
  { label: "TTFT p99", format: (m) => `${(m.ttft_p99 ?? 0).toFixed(3)}s`, lowerIsBetter: true },
  { label: "TPS mean", format: (m) => (m.tps_mean ?? 0).toFixed(1), lowerIsBetter: false },
  { label: "Goodput", format: (m) => `${((m.goodput ?? 0) * 100).toFixed(1)}%`, lowerIsBetter: false },
  { label: "Queue delay p99", format: (m) => `${(m.queue_delay_p99 ?? 0).toFixed(3)}s`, lowerIsBetter: true },
];

export function metricValue(m: SimulationMetrics, label: string): number {
  switch (label) {
    case "Makespan":
      return m.makespan;
    case "GPU Utilization":
      return m.gpu_utilization;
    case "Mean Cumulative Wait":
      return cumulativeWait(m);
    case "Jobs Completed":
      return m.jobs_completed / Math.max(m.jobs_total, 1);
    case "Preemptions":
      return m.preemptions;
    case "Failed":
      return m.jobs_failed;
    case "Queue Max":
      return m.queue_max_length;
    case "Unschedulable":
      return m.jobs_unschedulable ?? 0;
    case "Topo Penalties":
      return m.topology_penalties;
    case "TTFT p50":
      return m.ttft_p50 ?? 0;
    case "TTFT p99":
      return m.ttft_p99 ?? 0;
    case "TPS mean":
      return m.tps_mean ?? 0;
    case "Goodput":
      return m.goodput ?? 0;
    case "Queue delay p99":
      return m.queue_delay_p99 ?? 0;
    default:
      return 0;
  }
}

export function formatDelta(fieldLabel: string, baseline: number, value: number, lowerIsBetter: boolean): string {
  const tie = Math.abs(baseline - value) < 1e-6;
  if (tie) return "Tie";
  const better = lowerIsBetter ? value < baseline : value > baseline;
  if (fieldLabel === "GPU Utilization" || fieldLabel === "Jobs Completed") {
    const pct = Math.abs((value - baseline) * 100);
    return better ? `Better (Δ ${pct.toFixed(1)}%)` : `Worse (Δ ${pct.toFixed(1)}%)`;
  }
  const delta = Math.abs(value - baseline);
  const unit = fieldLabel === "Makespan" || fieldLabel === "Mean Cumulative Wait" ? "s" : "";
  return better ? `Better (Δ ${delta.toFixed(delta < 10 ? 2 : 1)}${unit})` : `Worse (Δ ${delta.toFixed(delta < 10 ? 2 : 1)}${unit})`;
}

export interface CompareEntry {
  key: string;
  label: string;
  metrics: SimulationMetrics | null | undefined;
  header?: React.ReactNode;
  emptyText?: string;
}

/**
 * Renders `entries` side by side, one card per entry, with per-field
 * better/worse deltas against `entries[0]` as the baseline whenever there
 * are exactly two entries. Shared by `ComparePanel` (config A vs config B)
 * and `ShadowRace` (primary vs shadow scheduler).
 */
export function MetricsCompareGrid({ entries, className }: { entries: CompareEntry[]; className?: string }) {
  if (!entries.length) return null;
  const baseline = entries[0]?.metrics;

  return (
    <div className={className ? `compare-grid ${className}` : "compare-grid"}>
      {entries.map((entry, entryIndex) => (
        <div key={entry.key} className="compare-result-card">
          {entry.header}
          {entry.metrics ? (
            <div className="compare-metrics-grid">
              {COMPARE_FIELDS.map((field) => {
                const value = field.format(entry.metrics!);
                let highlight = "";
                let deltaText: string | null = null;
                if (baseline && entries.length === 2 && field.lowerIsBetter != null && entryIndex > 0) {
                  const a = metricValue(baseline, field.label);
                  const b = metricValue(entry.metrics!, field.label);
                  const better = field.lowerIsBetter ? b < a : b > a;
                  const tie = Math.abs(a - b) < 1e-6;
                  deltaText = formatDelta(field.label, a, b, field.lowerIsBetter);
                  if (!tie && better) highlight = "compare-metric-better";
                  if (!tie && !better) highlight = "compare-metric-worse";
                }
                return (
                  <div key={field.label} className={highlight}>
                    <MetricTile label={field.label} value={value} />
                    {deltaText ? <p className="compare-metric-delta">{deltaText}</p> : null}
                  </div>
                );
              })}
            </div>
          ) : (
            <p className="text-sm text-hs-muted">{entry.emptyText ?? "No metrics available for this run."}</p>
          )}
        </div>
      ))}
    </div>
  );
}
