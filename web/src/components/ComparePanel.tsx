// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import type { CompareResult } from "@/types/simulation";
import { AppLink, StatusBadge } from "./ui";
import { MetricsCompareGrid } from "./MetricsCompareGrid";

export function ComparePanel({ results }: { results: CompareResult[] }) {
  if (!results.length) return null;

  return (
    <MetricsCompareGrid
      className="mt-4"
      entries={results.map((r) => ({
        key: `${r.config}-${r.run_id}`,
        label: r.config,
        metrics: r.metrics,
        emptyText:
          r.status === "failed"
            ? "Simulation failed — no metrics available for this config."
            : "No metrics available for this run.",
        header: (
          <div className="compare-result-header">
            <div>
              <div className="text-sm font-semibold text-hs-heading">{r.config}</div>
              <div className="mt-1 flex flex-wrap items-center gap-2">
                <StatusBadge status={r.status} />
                <span className="font-mono text-xs text-hs-muted">{r.run_id.slice(0, 8)}</span>
              </div>
            </div>
            {r.status === "completed" ? <AppLink href={`/runs/${r.run_id}`}>View run</AppLink> : null}
          </div>
        ),
      }))}
    />
  );
}
