// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import type { SimulationMetrics } from "@/types/simulation";
import { hasInferenceMetrics } from "@/types/simulation";
import { chartColors } from "@/lib/theme";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { motion } from "framer-motion";
import { staggerContainer } from "@/lib/motion";
import { Card, MetricTile } from "./ui";

function cumulativeWait(metrics: SimulationMetrics): number {
  return metrics.mean_cumulative_wait_time ?? metrics.mean_wait_time;
}

export function MetricsDashboard({ metrics }: { metrics: SimulationMetrics | null }) {
  if (!metrics) return <Card title="Metrics">Run not complete.</Card>;

  const meanWait = cumulativeWait(metrics);
  const unschedulable = metrics.jobs_unschedulable ?? 0;
  const showInference = hasInferenceMetrics(metrics);

  const percentBars = [
    { name: "GPU util", value: metrics.gpu_utilization * 100 },
    { name: "Jobs done", value: (metrics.jobs_completed / Math.max(metrics.jobs_total, 1)) * 100 },
  ];

  const countBars = [
    { name: "Preemptions", value: metrics.preemptions },
    { name: "Failed", value: metrics.jobs_failed },
    { name: "Topo penalties", value: metrics.topology_penalties },
    { name: "Queue max", value: metrics.queue_max_length },
    { name: "Unschedulable", value: unschedulable },
  ];

  return (
    <div className="space-y-4">
      <motion.div className="metrics-grid" variants={staggerContainer} initial="hidden" animate="visible">
        <MetricTile label="Makespan" value={`${metrics.makespan.toFixed(1)}s`} />
        <MetricTile label="GPU Utilization" value={`${(metrics.gpu_utilization * 100).toFixed(1)}%`} />
        <MetricTile label="Mean Cumulative Wait" value={`${meanWait.toFixed(2)}s`} />
        <MetricTile label="Jobs" value={`${metrics.jobs_completed}/${metrics.jobs_total}`} />
        <MetricTile label="Preemptions" value={String(metrics.preemptions)} />
        <MetricTile label="Failed Jobs" value={String(metrics.jobs_failed)} />
        <MetricTile label="Queue Max" value={String(metrics.queue_max_length)} />
        <MetricTile label="Unschedulable" value={String(unschedulable)} />
        {showInference ? (
          <>
            <MetricTile label="TTFT p50" value={`${(metrics.ttft_p50 ?? 0).toFixed(3)}s`} />
            <MetricTile label="TTFT p99" value={`${(metrics.ttft_p99 ?? 0).toFixed(3)}s`} />
            <MetricTile label="TPS mean" value={(metrics.tps_mean ?? 0).toFixed(1)} />
            <MetricTile label="Goodput" value={`${((metrics.goodput ?? 0) * 100).toFixed(1)}%`} />
            <MetricTile label="Queue delay p99" value={`${(metrics.queue_delay_p99 ?? 0).toFixed(3)}s`} />
          </>
        ) : null}
      </motion.div>
      <Card title="Metrics Charts">
        <div className="grid gap-4 md:grid-cols-2">
          <div>
            <p className="mb-2 text-xs font-medium uppercase tracking-wide text-hs-muted">Percent metrics</p>
            <ResponsiveContainer width="100%" height={180}>
              <BarChart data={percentBars}>
                <CartesianGrid strokeDasharray="3 3" stroke={chartColors.grid} />
                <XAxis dataKey="name" tick={{ fill: chartColors.tick, fontSize: 11 }} />
                <YAxis domain={[0, 100]} tick={{ fill: chartColors.tick, fontSize: 11 }} unit="%" />
                <Tooltip
                  formatter={(value: number) => [`${value.toFixed(1)}%`, "Value"]}
                  contentStyle={{
                    backgroundColor: "#0b0f14",
                    border: "1px solid rgba(255,255,255,0.06)",
                    borderRadius: "8px",
                  }}
                />
                <Bar dataKey="value" fill={chartColors.bar} />
              </BarChart>
            </ResponsiveContainer>
          </div>
          <div>
            <p className="mb-2 text-xs font-medium uppercase tracking-wide text-hs-muted">Count metrics</p>
            <ResponsiveContainer width="100%" height={180}>
              <BarChart data={countBars}>
                <CartesianGrid strokeDasharray="3 3" stroke={chartColors.grid} />
                <XAxis dataKey="name" tick={{ fill: chartColors.tick, fontSize: 11 }} />
                <YAxis tick={{ fill: chartColors.tick, fontSize: 11 }} allowDecimals={false} />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "#0b0f14",
                    border: "1px solid rgba(255,255,255,0.06)",
                    borderRadius: "8px",
                  }}
                />
                <Bar dataKey="value" fill={chartColors.line} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>
      </Card>
    </div>
  );
}
