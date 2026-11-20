// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { useEffect, useState } from "react";
import {
  Activity,
  Boxes,
  Layers,
  ListOrdered,
  Network,
  Swords,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { ClusterView } from "@/components/ClusterView";
import { GanttChart } from "@/components/GanttChart";
import { MetricsDashboard } from "@/components/MetricsCharts";
import { MigView } from "@/components/MigView";
import { QueueTable } from "@/components/QueueTable";
import { ReplayControls } from "@/components/ReplayControls";
import { ShadowRace } from "@/components/ShadowRace";
import { TopologyView } from "@/components/TopologyView";
import { AppLink, PageHero, Skeleton, StatusBadge, TabPanel, Tabs } from "@/components/ui";
import { fetchEvents, fetchRun, fetchShadowEvents, fetchSnapshots, fetchTimeline, fetchWsTicket, pollRun, runWebSocketUrl } from "@/lib/api";
import { easeOut } from "@/lib/motion";
import { useReplayStore } from "@/store/useReplayStore";
import type { ClusterSnapshot, JobsTimeline, RunDetail, SchedulerDecision, ShadowStepEvent } from "@/types/simulation";

type Phase = "loading" | "live" | "completed" | "failed";

export function RunDetailView({ runId }: { runId: string }) {
  const [run, setRun] = useState<RunDetail | null>(null);
  const [phase, setPhase] = useState<Phase>("loading");
  const [snapshots, setSnapshots] = useState<ClusterSnapshot[]>([]);
  const [decisions, setDecisions] = useState<SchedulerDecision[]>([]);
  const [timeline, setTimeline] = useState<JobsTimeline | null>(null);
  const [shadowSteps, setShadowSteps] = useState<ShadowStepEvent[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let ws: WebSocket | null = null;
    let stopPoll: (() => void) | null = null;
    let finalized = false;

    useReplayStore.getState().reset();

    async function loadCompletedExtras(detail: RunDetail) {
      const [snaps, events, tl, shadowEvents] = await Promise.all([
        fetchSnapshots(runId).catch(() => []),
        fetchEvents(runId).catch(() => []),
        fetchTimeline(runId).catch(() => null),
        detail.shadow_scheduler ? fetchShadowEvents(runId).catch(() => []) : Promise.resolve([]),
      ]);
      if (cancelled) return;
      setSnapshots(snaps);
      setDecisions(events);
      setTimeline(tl);
      if (detail.shadow_scheduler) {
        // Reconstructs the full step-by-step race from stored decisions --
        // needed when this run is loaded after the fact (no live WS steps
        // were ever seen), and also corrects any undercount from a WS
        // connection that joined mid-run.
        const steps: ShadowStepEvent[] = [
          ...events.map((d) => ({ side: "primary" as const, time: d.time, kind: d.kind, decisions: [d] })),
          ...shadowEvents.map((d) => ({ side: "shadow" as const, time: d.time, kind: d.kind, decisions: [d] })),
        ].sort((a, b) => a.time - b.time);
        setShadowSteps(steps);
      }
    }

    async function settle(detail: RunDetail) {
      if (finalized || cancelled) return;
      finalized = true;
      stopPoll?.();
      ws?.close();
      setRun(detail);
      if (detail.status === "completed") await loadCompletedExtras(detail);
      if (!cancelled) setPhase(detail.status === "failed" ? "failed" : "completed");
    }

    function fallbackToPolling() {
      if (finalized || stopPoll) return;
      stopPoll = pollRun(runId, (updated) => {
        if (cancelled) return;
        setRun(updated);
        if (updated.status === "completed" || updated.status === "failed") {
          settle(updated);
        }
      });
    }

    async function init() {
      let initial: RunDetail;
      try {
        initial = await fetchRun(runId);
      } catch (e) {
        if (!cancelled) setLoadError(e instanceof Error ? e.message : "Run not found");
        return;
      }
      if (cancelled) return;
      setRun(initial);

      if (initial.status === "completed" || initial.status === "failed") {
        await settle(initial);
        return;
      }

      setPhase("live");
      try {
        const ticket = await fetchWsTicket(runId);
        if (cancelled) return;
        ws = new WebSocket(runWebSocketUrl(runId, ticket));
      } catch {
        fallbackToPolling();
        return;
      }

      ws.onmessage = (event) => {
        if (cancelled) return;
        let msg: {
          type: string;
          data?: ClusterSnapshot | { side: "primary" | "shadow"; outcome: { event_time: number; kind: string; new_decisions: SchedulerDecision[] } };
        };
        try {
          msg = JSON.parse(event.data);
        } catch {
          return;
        }
        if (msg.type === "snapshot" && msg.data) {
          setSnapshots((prev) => [...prev, msg.data as ClusterSnapshot]);
        } else if (msg.type === "step" && msg.data && "outcome" in msg.data) {
          const raw = msg.data;
          setShadowSteps((prev) => [
            ...prev,
            {
              side: raw.side,
              time: raw.outcome.event_time,
              kind: raw.outcome.kind,
              decisions: raw.outcome.new_decisions,
            },
          ]);
        } else if (msg.type === "complete") {
          fetchRun(runId)
            .then((detail) => settle(detail))
            .catch(() => fallbackToPolling());
        }
      };
      ws.onerror = () => {
        if (!cancelled) fallbackToPolling();
      };
      ws.onclose = () => {
        if (!cancelled && !finalized) fallbackToPolling();
      };
    }

    init();

    return () => {
      cancelled = true;
      ws?.close();
      stopPoll?.();
    };
  }, [runId]);

  let bodyKey: string;
  let body: React.ReactNode;

  if (loadError) {
    bodyKey = "error";
    body = (
      <PageHero
        kicker="RUN"
        titleAccent="NOT FOUND"
        title=""
        subtitle={loadError ?? undefined}
        status="down"
      />
    );
  } else if (!run) {
    bodyKey = "loading";
    body = (
      <div className="space-y-4">
        <PageHero kicker="RUN" titleAccent="LOADING" title="" status="offline" subtitle="Fetching run…" />
        <div className="grid gap-3 md:grid-cols-4">
          <Skeleton className="h-20" />
          <Skeleton className="h-20" />
          <Skeleton className="h-20" />
          <Skeleton className="h-20" />
        </div>
        <Skeleton className="h-64" />
      </div>
    );
  } else {
    bodyKey = "loaded";
    body = (
      <RunDetailContent
        run={run}
        phase={phase}
        snapshots={snapshots}
        decisions={decisions}
        timeline={timeline}
        shadowSteps={shadowSteps}
        runId={runId}
      />
    );
  }

  return (
    <AnimatePresence initial={false}>
      <motion.div
        key={bodyKey}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.25, ease: easeOut }}
      >
        {body}
      </motion.div>
    </AnimatePresence>
  );
}

function RunDetailContent({
  run,
  phase,
  snapshots,
  decisions,
  timeline,
  shadowSteps,
  runId,
}: {
  run: RunDetail;
  phase: Phase;
  snapshots: ClusterSnapshot[];
  decisions: SchedulerDecision[];
  timeline: JobsTimeline | null;
  shadowSteps: ShadowStepEvent[];
  runId: string;
}) {
  const index = useReplayStore((s) => s.index);
  const live = phase === "live";
  const isShadow = Boolean(run.shadow_scheduler);
  const currentSnapshot = snapshots[index] ?? snapshots[snapshots.length - 1] ?? null;
  const heroStatus =
    phase === "failed" ? "down" : phase === "live" ? "go" : phase === "completed" ? "go" : "offline";
  const banner =
    phase === "failed" ? "FAILED" : phase === "live" ? "LIVE" : phase === "completed" ? "DONE" : "STANDBY";

  const tabs = [
    { id: "overview", label: "Overview", icon: <Activity size={14} /> },
    ...(isShadow ? [{ id: "shadow", label: "Shadow", icon: <Swords size={14} /> }] : []),
    { id: "cluster", label: "Cluster", icon: <Network size={14} /> },
    { id: "queue", label: "Queue", icon: <ListOrdered size={14} /> },
    { id: "mig", label: "MIG", icon: <Layers size={14} /> },
    { id: "replay", label: "Replay", icon: <Boxes size={14} /> },
  ];

  return (
    <>
      <div className="run-detail-header">
        <PageHero
          kicker={run.config}
          titleAccent={banner}
          title={run.id.slice(0, 8)}
          subtitle={
            run.scheduler
              ? isShadow
                ? `${run.scheduler} vs ${run.shadow_scheduler} (shadow)`
                : `Scheduler: ${run.scheduler}`
              : undefined
          }
          status={heroStatus}
          statusLabel={run.status}
          actions={<AppLink href="/">Back to dashboard</AppLink>}
        />
        <div className="run-detail-meta">
          <StatusBadge status={run.status} />
          {live ? <span className="run-detail-live">Live · {snapshots.length} frames</span> : null}
        </div>
      </div>

      {phase === "failed" && run.error ? (
        <div className="run-error-banner">
          <strong className="block mb-1">Simulation failed</strong>
          {run.error}
        </div>
      ) : null}
      {live ? (
        <p className="run-warning-banner">
          Simulation is running — results will appear here as they arrive, then finalize once the run
          completes.
        </p>
      ) : null}

      <Tabs tabs={tabs} defaultTab={isShadow ? "shadow" : live ? "replay" : "overview"}>
        <TabPanel id="overview">
          <div className="run-detail-grid">
            {!live ? (
              <div className="run-detail-span-2">
                <MetricsDashboard metrics={run.metrics} />
              </div>
            ) : (
              <div className="run-detail-span-2">
                <p className="text-sm text-hs-muted">
                  Metrics finalize when the run completes. Streaming {snapshots.length} cluster snapshots…
                </p>
              </div>
            )}
            {!live ? (
              <div className="run-detail-span-2">
                <GanttChart timeline={timeline} />
              </div>
            ) : null}
          </div>
        </TabPanel>

        {isShadow ? (
          <TabPanel id="shadow">
            <ShadowRace run={run} steps={shadowSteps} live={live} />
          </TabPanel>
        ) : null}

        <TabPanel id="cluster">
          <div className="run-detail-grid">
            <ClusterView snapshot={currentSnapshot} />
            <TopologyView snapshot={currentSnapshot} />
          </div>
        </TabPanel>

        <TabPanel id="queue">
          <div className="run-detail-grid">
            {!live ? <QueueTable timeline={timeline} /> : null}
            <div className="run-detail-span-2">
              {decisions.length === 0 ? (
                <p className="text-sm text-hs-muted">No scheduler decisions yet.</p>
              ) : (
                <div className="activity-feed" style={{ maxHeight: "32rem" }}>
                  <AnimatePresence initial={false}>
                    {decisions
                      .slice()
                      .reverse()
                      .map((ev) => (
                        <motion.div
                          key={`${ev.time}-${ev.kind}-${ev.job_id ?? ev.message}`}
                          layout
                          initial={{ opacity: 0, x: -10 }}
                          animate={{ opacity: 1, x: 0 }}
                          exit={{ opacity: 0 }}
                          transition={{ duration: 0.25, ease: easeOut }}
                          className="activity-item"
                        >
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="activity-item-time">t={ev.time.toFixed(2)}s</span>
                            <span className={`decision-badge decision-badge-${ev.kind.replace("job_", "").replace("_", "-")}`}>
                              {ev.kind.replace(/_/g, " ")}
                            </span>
                          </div>
                          <p className="activity-item-msg">{ev.message}</p>
                        </motion.div>
                      ))}
                  </AnimatePresence>
                </div>
              )}
            </div>
          </div>
        </TabPanel>

        <TabPanel id="mig">
          <div className="run-detail-grid">
            <div className="run-detail-span-2">
              <MigView snapshot={currentSnapshot} />
            </div>
          </div>
        </TabPanel>

        <TabPanel id="replay">
          <div className="run-detail-grid">
            <div className="run-detail-span-2 flex flex-wrap gap-2">
              <AppLink href={`/simulate?run=${runId}`} className="run-btn primary" showArrow={false}>
                Open in Simulate
              </AppLink>
            </div>
            <ReplayControls snapshots={snapshots} decisions={decisions} live={live} />
            <ClusterView snapshot={currentSnapshot} />
            <TopologyView snapshot={currentSnapshot} />
          </div>
        </TabPanel>
      </Tabs>
    </>
  );
}
