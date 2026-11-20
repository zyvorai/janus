// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { useMemo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Swords, Trophy } from "lucide-react";
import { easeOut } from "@/lib/motion";
import type { RunDetail, ShadowStepEvent } from "@/types/simulation";
import { StatusBadge } from "./ui";
import { COMPARE_FIELDS, MetricsCompareGrid, metricValue } from "./MetricsCompareGrid";

interface LaneTally {
  scheduled: number;
  completed: number;
  running: number;
  arrivals: Set<string>;
  lastTime: number;
  feed: Array<{ key: string; time: number; kind: string; message: string }>;
}

function emptyTally(): LaneTally {
  return { scheduled: 0, completed: 0, running: 0, arrivals: new Set(), lastTime: 0, feed: [] };
}

function badgeClass(kind: string): string {
  return `decision-badge decision-badge-${kind.replace("job_", "").replace("_", "-")}`;
}

function tallySteps(steps: ShadowStepEvent[], side: "primary" | "shadow"): LaneTally {
  const tally = emptyTally();
  for (const step of steps) {
    if (step.side !== side) continue;
    tally.lastTime = step.time;
    for (const d of step.decisions) {
      if (d.kind === "job_arrival" && d.job_id) tally.arrivals.add(d.job_id);
      if (d.kind === "job_scheduled") {
        tally.scheduled += 1;
        tally.running += 1;
      }
      if (d.kind === "job_complete") {
        tally.completed += 1;
        tally.running = Math.max(0, tally.running - 1);
      }
      tally.feed.push({
        key: `${step.time}-${d.kind}-${d.job_id ?? d.message}`,
        time: d.time,
        kind: d.kind,
        message: d.message,
      });
    }
  }
  tally.feed = tally.feed.slice(-6).reverse();
  return tally;
}

function Lane({
  title,
  scheduler,
  accent,
  tally,
  totalEstimate,
}: {
  title: string;
  scheduler: string;
  accent: "primary" | "shadow";
  tally: LaneTally;
  totalEstimate: number;
}) {
  const pct = totalEstimate > 0 ? Math.min(100, (tally.completed / totalEstimate) * 100) : 0;
  return (
    <div className={`shadow-lane shadow-lane-${accent}`}>
      <div className="shadow-lane-head">
        <span className="shadow-lane-kicker">{title}</span>
        <span className="shadow-scheduler-chip">{scheduler}</span>
      </div>
      <div className="shadow-track" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
        <motion.div
          className="shadow-track-fill"
          initial={false}
          animate={{ width: `${pct}%` }}
          transition={{ duration: 0.4, ease: easeOut }}
        />
      </div>
      <div className="shadow-tally">
        <span>
          <strong>{tally.completed}</strong> done
        </span>
        <span>
          <strong>{tally.running}</strong> running
        </span>
        <span className="font-mono">t={tally.lastTime.toFixed(1)}s</span>
      </div>
      <div className="shadow-feed">
        {tally.feed.length === 0 ? (
          <p className="text-xs text-hs-muted">Waiting for the first decision…</p>
        ) : (
          <AnimatePresence initial={false}>
            {tally.feed.map((ev) => (
              <motion.div
                key={ev.key}
                layout
                initial={{ opacity: 0, x: accent === "primary" ? -8 : 8 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2, ease: easeOut }}
                className="shadow-feed-item"
              >
                <span className="shadow-feed-time">t={ev.time.toFixed(1)}s</span>
                <span className={badgeClass(ev.kind)}>{ev.kind.replace(/_/g, " ")}</span>
                <p className="shadow-feed-msg">{ev.message}</p>
              </motion.div>
            ))}
          </AnimatePresence>
        )}
      </div>
    </div>
  );
}

export function ShadowRace({
  run,
  steps,
  live,
}: {
  run: RunDetail;
  steps: ShadowStepEvent[];
  live: boolean;
}) {
  const primaryTally = useMemo(() => tallySteps(steps, "primary"), [steps]);
  const shadowTally = useMemo(() => tallySteps(steps, "shadow"), [steps]);
  const totalEstimate = Math.max(
    run.metrics?.jobs_total ?? 0,
    run.shadow_metrics?.jobs_total ?? 0,
    primaryTally.arrivals.size,
    shadowTally.arrivals.size,
    1
  );

  const winner = useMemo(() => {
    if (!run.metrics || !run.shadow_metrics) return null;
    let primaryWins = 0;
    let shadowWins = 0;
    for (const field of COMPARE_FIELDS) {
      if (field.lowerIsBetter == null) continue;
      const a = metricValue(run.metrics, field.label);
      const b = metricValue(run.shadow_metrics, field.label);
      if (Math.abs(a - b) < 1e-9) continue;
      const shadowBetter = field.lowerIsBetter ? b < a : b > a;
      if (shadowBetter) shadowWins += 1;
      else primaryWins += 1;
    }
    if (primaryWins === shadowWins) return { side: "tie" as const, primaryWins, shadowWins };
    return { side: primaryWins > shadowWins ? ("primary" as const) : ("shadow" as const), primaryWins, shadowWins };
  }, [run.metrics, run.shadow_metrics]);

  return (
    <div className="shadow-race">
      <div className="shadow-race-header">
        <Swords size={16} strokeWidth={1.75} />
        <span>Shadow race</span>
        {live ? <span className="shadow-live-pill">LIVE</span> : null}
      </div>

      <div className="shadow-lanes">
        <Lane
          title="Primary"
          scheduler={run.scheduler ?? "default"}
          accent="primary"
          tally={primaryTally}
          totalEstimate={totalEstimate}
        />
        <div className="shadow-vs" aria-hidden="true">
          VS
        </div>
        <Lane
          title="Shadow"
          scheduler={run.shadow_scheduler ?? "unknown"}
          accent="shadow"
          tally={shadowTally}
          totalEstimate={totalEstimate}
        />
      </div>

      {run.status === "completed" && run.metrics && run.shadow_metrics ? (
        <div className="shadow-verdict">
          {winner ? (
            <div className={`shadow-winner-banner shadow-winner-${winner.side}`}>
              <Trophy size={16} strokeWidth={1.75} />
              {winner.side === "tie" ? (
                <span>Evenly matched — {winner.primaryWins} metrics each.</span>
              ) : (
                <span>
                  <strong>{winner.side === "primary" ? run.scheduler : run.shadow_scheduler}</strong> wins on{" "}
                  {Math.max(winner.primaryWins, winner.shadowWins)} of{" "}
                  {winner.primaryWins + winner.shadowWins} compared metrics.
                </span>
              )}
            </div>
          ) : null}
          <MetricsCompareGrid
            className="shadow-compare-grid"
            entries={[
              {
                key: "primary",
                label: run.scheduler ?? "primary",
                metrics: run.metrics,
                header: (
                  <div className="compare-result-header">
                    <div>
                      <div className="text-sm font-semibold text-hs-heading">Primary · {run.scheduler}</div>
                      <StatusBadge status={run.status} />
                    </div>
                  </div>
                ),
              },
              {
                key: "shadow",
                label: run.shadow_scheduler ?? "shadow",
                metrics: run.shadow_metrics,
                header: (
                  <div className="compare-result-header">
                    <div>
                      <div className="text-sm font-semibold text-hs-heading">
                        Shadow · {run.shadow_scheduler}
                      </div>
                      <StatusBadge status={run.status} />
                    </div>
                  </div>
                ),
              },
            ]}
          />
        </div>
      ) : (
        <p className="text-sm text-hs-muted mt-3">
          {live
            ? "Metrics finalize once both schedulers finish stepping through the workload."
            : "Waiting for the shadow run to start…"}
        </p>
      )}
    </div>
  );
}
