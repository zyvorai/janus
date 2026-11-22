// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import { useEffect, useMemo, useState, Suspense } from "react";
import { useSearchParams } from "next/navigation";
import { Gpu } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { SimulateStage } from "@/components/SimulateStage";
import { ReplayControls } from "@/components/ReplayControls";
import {
  AppLink,
  Button,
  Card,
  EmptyState,
  FormField,
  MetricTile,
  PageHero,
  Select,
} from "@/components/ui";
import { fetchEvents, fetchRuns, fetchSnapshots } from "@/lib/api";
import { easeOut } from "@/lib/motion";
import { useReplayStore } from "@/store/useReplayStore";
import type { ClusterSnapshot, RunSummary, SchedulerDecision } from "@/types/simulation";

function usePrefersReducedMotion() {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    setReduced(mq.matches);
    const onChange = () => setReduced(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return reduced;
}

export default function SimulatePage() {
  return (
    <Suspense fallback={<div className="glass card sim-empty">Loading simulate…</div>}>
      <SimulatePageInner />
    </Suspense>
  );
}

function SimulatePageInner() {
  const searchParams = useSearchParams();
  const queryRun = searchParams.get("run") ?? "";
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [runId, setRunId] = useState("");
  const [snapshots, setSnapshots] = useState<ClusterSnapshot[]>([]);
  const [decisions, setDecisions] = useState<SchedulerDecision[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reducedMotion = usePrefersReducedMotion();
  const index = useReplayStore((s) => s.index);

  const watchable = useMemo(
    () => runs.filter((r) => r.status === "completed" || r.status === "running" || r.status === "failed"),
    [runs]
  );

  const snapshot = snapshots[index] ?? snapshots[snapshots.length - 1] ?? null;
  const decision = decisions[index] ?? null;

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    fetchRuns()
      .then((list) => {
        if (cancelled) return;
        setRuns(list);
        const fromQuery = queryRun && list.some((r) => r.id === queryRun) ? queryRun : "";
        const preferred =
          fromQuery ||
          list.find((r) => r.status === "completed")?.id ||
          list.find((r) => r.status === "running")?.id ||
          list[0]?.id ||
          "";
        if (preferred) setRunId(preferred);
      })
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load runs"))
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [queryRun]);

  async function loadRun(id: string) {
    if (!id) return;
    setBusy(true);
    setError(null);
    try {
      const [snaps, evts] = await Promise.all([
        fetchSnapshots(id).catch(() => [] as ClusterSnapshot[]),
        fetchEvents(id).catch(() => [] as SchedulerDecision[]),
      ]);
      setSnapshots(snaps);
      setDecisions(evts);
      useReplayStore.getState().setSnapshots(snaps);
      useReplayStore.getState().setDecisions(evts);
      useReplayStore.getState().setPlaying(snaps.length > 1 && !reducedMotion);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load run data");
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    if (runId) void loadRun(runId);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- load when runId / reducedMotion change
  }, [runId, reducedMotion]);

  const status = snapshot
    ? snapshot.running > 0
      ? "go"
      : snapshot.waiting > 0
        ? "degraded"
        : "idle"
    : "offline";

  return (
    <>
      <PageHero
        kicker="ZYVOR JANUS · SIMULATE"
        titleAccent="SIMULATE"
        title=""
        subtitle="Requests → queue → GPU assign → complete"
        status={status}
        icon={<Gpu size={26} strokeWidth={1.75} color="#38bdf8" />}
        actions={
          <AppLink href="/" className="run-btn" showArrow={false}>
            Launch run
          </AppLink>
        }
      >
        <div className="stat-grid">
          <MetricTile label="Queued" value={String(snapshot?.waiting ?? 0)} />
          <MetricTile label="Running" value={String(snapshot?.running ?? 0)} />
          <MetricTile label="Finished" value={String(snapshot?.finished ?? 0)} />
          <MetricTile label="Free GPUs" value={String(snapshot?.free_gpus ?? 0)} />
        </div>
      </PageHero>

      <h2>Stage</h2>
      <div className="sim-layout">
        <div className="sim-layout-main">
          <AnimatePresence mode="wait" initial={false}>
            {loading ? (
              <motion.div
                key="loading"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2 }}
                className="glass card sim-empty"
              >
                Loading runs…
              </motion.div>
            ) : watchable.length === 0 ? (
              <motion.div key="empty" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                <EmptyState
                  title="No runs to watch"
                  text="Launch a simulation from the Dashboard, then return here to see request → GPU flow."
                >
                  <AppLink href="/" className="run-btn primary" showArrow={false}>
                    Open Dashboard
                  </AppLink>
                </EmptyState>
              </motion.div>
            ) : (
              <motion.div key="stage" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                <SimulateStage snapshot={snapshot} decision={decision} reducedMotion={reducedMotion} />
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <aside className="sim-layout-side">
          <Card title="Watch" variant="action">
            <FormField label="Run">
              <Select value={runId} onChange={(e) => setRunId(e.target.value)} disabled={busy}>
                {watchable.map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.config} · {r.status} · {r.id.slice(0, 8)}
                  </option>
                ))}
              </Select>
            </FormField>
            <div className="mt-3 flex flex-wrap gap-2">
              <Button variant="secondary" disabled={busy || !runId} onClick={() => loadRun(runId)}>
                Reload
              </Button>
              {runId ? (
                <AppLink href={`/runs/${runId}`} className="run-btn" showArrow={false}>
                  Run detail
                </AppLink>
              ) : null}
            </div>
            {error ? <p className="form-error mt-2">{error}</p> : null}
          </Card>

          <Card title="Legend">
            <ul className="sim-legend">
              <li>
                <span className="sim-legend-swatch idle" /> Idle GPU
              </li>
              <li>
                <span className="sim-legend-swatch busy" /> Assigned / busy
              </li>
              <li>
                <span className="sim-legend-swatch hot" /> High utilization
              </li>
              <li>
                <span className="sim-legend-swatch move" /> Request / assign pulse
              </li>
            </ul>
          </Card>
        </aside>
      </div>

      <AnimatePresence>
        {snapshots.length > 0 || decisions.length > 0 ? (
          <motion.div
            key="replay"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.3, ease: easeOut }}
          >
            <h2>Replay</h2>
            <ReplayControls snapshots={snapshots} decisions={decisions} />
          </motion.div>
        ) : null}
      </AnimatePresence>
    </>
  );
}
