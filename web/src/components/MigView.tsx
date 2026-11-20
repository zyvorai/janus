// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import type { ClusterSnapshot } from "@/types/simulation";
import { Card } from "./ui";

export function MigView({ snapshot }: { snapshot: ClusterSnapshot | null }) {
  if (!snapshot) return null;

  return (
    <Card
      title="MIG Layout"
      description="Experimental preview — real MIG slice data is not yet wired into snapshots."
      className="mig-view-experimental"
    >
      <p className="mb-3 text-xs text-hs-muted">
        Placeholder layout for future MIG visualization. Whole-GPU jobs are shown as a single occupied slot.
      </p>
      <div className="grid gap-2 md:grid-cols-2">
        {snapshot.nodes.flatMap((node) =>
          node.gpus.map((gpu) => (
            <div key={gpu.id} className="rounded-hs border border-hs-border p-2 text-xs opacity-80">
              <div className="font-medium text-hs-heading">{gpu.id}</div>
              <div className="mt-1 grid grid-cols-4 gap-1">
                {[0, 1, 2, 3].map((slot) => (
                  <div
                    key={slot}
                    className={`rounded p-1 text-center ${
                      gpu.busy && slot === 0
                        ? "bg-hs-indigo/30 text-hs-purple-light border border-hs-indigo/40"
                        : "bg-hs-bg text-hs-dim border border-hs-border"
                    }`}
                  >
                    {gpu.busy && slot === 0 ? (gpu.job_name?.slice(0, 6) ?? "Job") : "Idle"}
                  </div>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </Card>
  );
}
