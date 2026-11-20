// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { AppLink, Card, EmptyState, PageHero, Skeleton } from "@/components/ui";
import { fetchTwins } from "@/lib/api";
import { fadeInUp, staggerContainer } from "@/lib/motion";
import type { TwinEntry } from "@/types/simulation";

export default function TwinsPage() {
  const [twins, setTwins] = useState<TwinEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [gpuFilter, setGpuFilter] = useState("all");

  useEffect(() => {
    let cancelled = false;
    fetchTwins()
      .then((data) => {
        if (!cancelled) setTwins(data);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : "Failed to load twins");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const gpuTypes = useMemo(() => {
    if (!twins) return [];
    return Array.from(new Set(twins.map((t) => t.gpu_type))).sort();
  }, [twins]);

  const rows = useMemo(() => {
    if (!twins) return [];
    const filtered = gpuFilter === "all" ? twins : twins.filter((t) => t.gpu_type === gpuFilter);
    return filtered.slice().sort((a, b) => b.measured_at.localeCompare(a.measured_at));
  }, [twins, gpuFilter]);

  return (
    <div className="space-y-6">
      <PageHero
        kicker="TWIN LIBRARY"
        titleAccent="DIGITAL TWINS"
        title=""
        subtitle="AIPerf-calibrated GPU/model performance twins used to ground the simulator against measured hardware."
        status={twins === null ? "offline" : twins.length ? "go" : "idle"}
      />

      {error ? (
        <Card>
          <p className="text-sm text-red-400">{error}</p>
        </Card>
      ) : twins === null ? (
        <div className="space-y-3">
          <Skeleton className="h-10" />
          <Skeleton className="h-64" />
        </div>
      ) : twins.length === 0 ? (
        <EmptyState
          title="No twins calibrated yet"
          text="Twins are populated by offline AIPerf calibration runs (python/zyvor_janus/benchmarks/aiperf_adapter.py), not from the UI."
        />
      ) : (
        <Card
          title="Calibrated twins"
          description={`${rows.length} of ${twins.length} twin${twins.length === 1 ? "" : "s"} shown.`}
        >
          {gpuTypes.length > 1 ? (
            <div className="mb-4 flex flex-wrap gap-2">
              <button
                className={`run-btn ${gpuFilter === "all" ? "active" : ""}`}
                onClick={() => setGpuFilter("all")}
              >
                All GPUs
              </button>
              {gpuTypes.map((gpu) => (
                <button
                  key={gpu}
                  className={`run-btn ${gpuFilter === gpu ? "active" : ""}`}
                  onClick={() => setGpuFilter(gpu)}
                >
                  {gpu}
                </button>
              ))}
            </div>
          ) : null}
          <div className="overflow-auto">
            <table className="data-table">
              <thead>
                <tr>
                  <th>GPU</th>
                  <th>Model</th>
                  <th>TTFT (ms)</th>
                  <th>TPS</th>
                  <th>Throughput</th>
                  <th>Measured</th>
                  <th>AIPerf run</th>
                </tr>
              </thead>
              <motion.tbody variants={staggerContainer} initial="hidden" animate="visible">
                {rows.map((twin, idx) => (
                  <motion.tr key={`${twin.gpu_type}-${twin.model}-${twin.measured_at}-${idx}`} variants={fadeInUp}>
                    <td className="font-medium text-hs-heading">{twin.gpu_type}</td>
                    <td>{twin.model}</td>
                    <td>{twin.ttft_ms.toFixed(1)}</td>
                    <td>{twin.tps.toFixed(1)}</td>
                    <td>{twin.throughput.toFixed(1)}</td>
                    <td>{twin.measured_at}</td>
                    <td>
                      {twin.aiperf_run_id ? (
                        <AppLink href={`/runs/${twin.aiperf_run_id}`} showArrow={false}>
                          {twin.aiperf_run_id.slice(0, 8)}
                        </AppLink>
                      ) : (
                        <span className="text-hs-muted">—</span>
                      )}
                    </td>
                  </motion.tr>
                ))}
              </motion.tbody>
            </table>
          </div>
        </Card>
      )}
    </div>
  );
}
