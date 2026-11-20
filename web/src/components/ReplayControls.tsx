// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { useEffect } from "react";
import type { ClusterSnapshot, SchedulerDecision } from "@/types/simulation";
import { useReplayStore } from "@/store/useReplayStore";
import { Button, Card, FormField, Select } from "./ui";
import { Pause, Play, SkipBack, SkipForward } from "lucide-react";
import clsx from "clsx";

const DECISION_STYLES: Record<string, string> = {
  job_scheduled: "decision-badge decision-badge-scheduled",
  job_arrival: "decision-badge decision-badge-arrival",
  job_complete: "decision-badge decision-badge-complete",
  job_preempted: "decision-badge decision-badge-preempted",
  gang_timeout: "decision-badge decision-badge-timeout",
};

function DecisionKindBadge({ kind }: { kind: string }) {
  return <span className={DECISION_STYLES[kind] ?? "decision-badge"}>{kind.replace(/_/g, " ")}</span>;
}

export function ReplayControls({
  snapshots,
  decisions,
  live = false,
}: {
  snapshots: ClusterSnapshot[];
  decisions: SchedulerDecision[];
  live?: boolean;
}) {
  const { index, playing, speed, setSnapshots, setDecisions, setPlaying, setSpeed, setIndex, next, prev } =
    useReplayStore();

  useEffect(() => {
    if (live) {
      // Keep index pinned to the newest frame instead of resetting to 0 on every append.
      useReplayStore.setState({
        snapshots,
        decisions,
        index: Math.max(snapshots.length, decisions.length, 1) - 1,
      });
      return;
    }
    setSnapshots(snapshots);
    setDecisions(decisions);
  }, [snapshots, decisions, live, setSnapshots, setDecisions]);

  useEffect(() => {
    if (!playing || live) return;
    const timer = setInterval(() => {
      const max = Math.max(snapshots.length, decisions.length, 1) - 1;
      if (index >= max) {
        setPlaying(false);
        return;
      }
      next();
    }, 500 / speed);
    return () => clearInterval(timer);
  }, [playing, live, speed, index, snapshots.length, decisions.length, next, setPlaying]);

  const decision = decisions[index] ?? decisions[decisions.length - 1];
  const snapshot = snapshots[index] ?? snapshots[snapshots.length - 1];
  const totalSteps = Math.max(snapshots.length, decisions.length, 1);
  const maxIndex = Math.max(totalSteps - 1, 0);

  const clockMax =
    snapshots[snapshots.length - 1]?.clock ??
    decisions[decisions.length - 1]?.time ??
    1;
  const markers = [0, 0.25, 0.5, 0.75, 1].map((p) => (clockMax * p).toFixed(1));

  return (
    <Card
      title="Scheduler Replay"
      description="Cluster, topology, and MIG views follow the replay step below."
      className="run-detail-span-2"
    >
      <div className="mb-3 flex flex-wrap items-center gap-2">
        {live ? (
          <span className="run-detail-live">Live · {snapshots.length} snapshots</span>
        ) : (
          <>
            <Button variant="secondary" onClick={() => setPlaying(!playing)}>
              {playing ? (
                <>
                  <Pause size={14} /> Pause
                </>
              ) : (
                <>
                  <Play size={14} /> Play
                </>
              )}
            </Button>
            <Button variant="secondary" onClick={prev}>
              <SkipBack size={14} /> Prev
            </Button>
            <Button variant="secondary" onClick={next}>
              Next <SkipForward size={14} />
            </Button>
            {[0.5, 1, 2, 10].map((s) => (
              <button
                key={s}
                type="button"
                aria-pressed={speed === s}
                className={clsx(
                  "rounded-hs px-2 py-1 text-xs transition",
                  speed === s
                    ? "bg-hs-indigo/30 text-hs-purple-light border border-hs-indigo/40"
                    : "bg-hs-bg border border-hs-border text-hs-muted hover:border-hs-border-accent"
                )}
                onClick={() => setSpeed(s)}
              >
                {s}x
              </button>
            ))}
          </>
        )}
        <span className="font-mono text-xs text-hs-muted">
          step {index + 1} / {totalSteps}
        </span>
      </div>

      {!live && totalSteps > 1 ? (
        <div className="replay-scrubber">
          <input
            type="range"
            className="replay-scrubber-track"
            min={0}
            max={maxIndex}
            value={Math.min(index, maxIndex)}
            onChange={(e) => {
              setPlaying(false);
              setIndex(Number(e.target.value));
            }}
            aria-label="Replay timeline scrubber"
          />
          <div className="replay-scrubber-labels">
            {markers.map((m, i) => (
              <span key={i}>{m}s</span>
            ))}
          </div>
        </div>
      ) : null}

      {!live && decisions.length > 0 ? (
        <div className="replay-jump">
          <FormField label="Jump to event">
            <Select
              value={String(Math.min(index, decisions.length - 1))}
              onChange={(e) => {
                setPlaying(false);
                setIndex(Number(e.target.value));
              }}
            >
              {decisions.map((d, i) => (
                <option key={`${d.time}-${d.kind}-${i}`} value={i}>
                  [{i + 1}] t={d.time.toFixed(2)}s · {d.kind.replace(/_/g, " ")}
                  {d.job_name ? ` · ${d.job_name}` : ""}
                </option>
              ))}
            </Select>
          </FormField>
        </div>
      ) : null}

      {decision ? (
        <div
          className={clsx(
            "decision-panel mt-3",
            decision.kind === "job_preempted" && "decision-panel-preempted"
          )}
        >
          <div className="flex flex-wrap items-center gap-2 font-mono text-xs text-hs-muted">
            <span>t={decision.time.toFixed(2)}s</span>
            <DecisionKindBadge kind={decision.kind} />
          </div>
          <div className="mt-2 text-hs-heading">{decision.message}</div>
          {decision.job_name ? (
            <div className="mt-1 text-sm text-hs-body">
              Job: <span className="font-medium text-hs-heading">{decision.job_name}</span>
            </div>
          ) : null}
          {decision.gpu_ids.length > 0 ? (
            <div className="mt-1 font-mono text-xs text-hs-muted">GPUs: {decision.gpu_ids.join(", ")}</div>
          ) : null}
        </div>
      ) : (
        <p className="text-sm text-hs-muted">No scheduler decisions recorded for this run.</p>
      )}
      {snapshot ? (
        <div className="mt-3 flex flex-wrap gap-3 font-mono text-xs text-hs-muted">
          <span>clock={snapshot.clock.toFixed(2)}</span>
          <span>running={snapshot.running}</span>
          <span>queued={snapshot.waiting}</span>
          <span>free GPUs={snapshot.free_gpus}</span>
        </div>
      ) : null}
    </Card>
  );
}
