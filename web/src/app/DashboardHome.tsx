// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Swords } from "lucide-react";
import { ComparePanel } from "@/components/ComparePanel";
import {
  AppLink,
  Button,
  Card,
  EmptyState,
  FormField,
  MetricTile,
  PageHero,
  SCHEDULERS,
  SchedulerPillGroup,
  Select,
  StatusBadge,
} from "@/components/ui";
import { compareConfigs, fetchConfigs, fetchEvents, fetchRuns, startRun } from "@/lib/api";
import { easeOut, fadeInUp, staggerContainer } from "@/lib/motion";
import type { CompareResult, ConfigEntry, RunSummary, SchedulerDecision } from "@/types/simulation";

type SortKey = "config" | "status" | "created_at";
type SortDir = "asc" | "desc";

function pushHistory(prev: number[], next: number, max = 10): number[] {
  const out = [...prev, next];
  return out.length > max ? out.slice(out.length - max) : out;
}

function PulseIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="#38bdf8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
    </svg>
  );
}

export function DashboardHome() {
  const [configs, setConfigs] = useState<ConfigEntry[]>([]);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [selected, setSelected] = useState("");
  const [schedulerHint, setSchedulerHint] = useState("fifo");
  const [shadowScheduler, setShadowScheduler] = useState("");
  const [compareA, setCompareA] = useState("");
  const [compareB, setCompareB] = useState("");
  const [compareResults, setCompareResults] = useState<CompareResult[]>([]);
  const [runBusy, setRunBusy] = useState(false);
  const [compareBusy, setCompareBusy] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);
  const [compareError, setCompareError] = useState<string | null>(null);
  const [sortKey, setSortKey] = useState<SortKey>("created_at");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [activity, setActivity] = useState<SchedulerDecision[]>([]);
  const [kpiHistory, setKpiHistory] = useState({
    total: [] as number[],
    running: [] as number[],
    completed: [] as number[],
    failed: [] as number[],
  });

  const canCompare = Boolean(compareA && compareB && compareA !== compareB);

  const runningCount = runs.filter((r) => r.status === "running" || r.status === "pending").length;
  const completedCount = runs.filter((r) => r.status === "completed").length;
  const failedCount = runs.filter((r) => r.status === "failed").length;
  const health = failedCount > 0 ? "degraded" : runningCount > 0 ? "go" : "offline";
  const bannerTitle = health === "go" ? "LIVE" : health === "degraded" ? "DEGRADED" : "STANDBY";
  const healthLabel =
    health === "go" ? "Simulations running" : health === "degraded" ? "Recent failures" : "Idle — ready to run";

  const sortedRuns = useMemo(() => {
    const copy = [...runs];
    copy.sort((a, b) => {
      const av = a[sortKey];
      const bv = b[sortKey];
      const cmp = String(av).localeCompare(String(bv));
      return sortDir === "asc" ? cmp : -cmp;
    });
    return copy;
  }, [runs, sortKey, sortDir]);

  const refresh = useCallback(async () => {
    const [cfgs, runList] = await Promise.all([fetchConfigs(), fetchRuns()]);
    setConfigs(cfgs);
    setRuns(runList);
    if (!selected && cfgs.length) setSelected(cfgs[0].id);

    const running = runList.filter((r) => r.status === "running" || r.status === "pending").length;
    const completed = runList.filter((r) => r.status === "completed").length;
    const failed = runList.filter((r) => r.status === "failed").length;
    setKpiHistory((prev) => ({
      total: pushHistory(prev.total, runList.length),
      running: pushHistory(prev.running, running),
      completed: pushHistory(prev.completed, completed),
      failed: pushHistory(prev.failed, failed),
    }));

    const latest = runList[0];
    if (latest) {
      try {
        const events = await fetchEvents(latest.id);
        setActivity(events.slice(-12).reverse());
      } catch {
        setActivity([]);
      }
    } else {
      setActivity([]);
    }
  }, [selected]);

  useEffect(() => {
    refresh().catch(console.error);
    const t = setInterval(() => refresh().catch(console.error), 3000);
    return () => clearInterval(t);
  }, [refresh]);

  function toggleSort(key: SortKey) {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else {
      setSortKey(key);
      setSortDir(key === "created_at" ? "desc" : "asc");
    }
  }

  async function handleRun() {
    if (!selected || runBusy || compareBusy) return;
    setRunBusy(true);
    setRunError(null);
    try {
      const { id } = await startRun(selected, { shadowScheduler: shadowScheduler || undefined });
      await refresh();
      window.location.href = `/runs/${id}`;
    } catch (e) {
      setRunError(e instanceof Error ? e.message : "Failed to start simulation");
    } finally {
      setRunBusy(false);
    }
  }

  async function handleCompare() {
    if (!canCompare || runBusy || compareBusy) return;
    setCompareBusy(true);
    setCompareError(null);
    try {
      const { results } = await compareConfigs([compareA, compareB]);
      setCompareResults(results);
    } catch (e) {
      setCompareError(e instanceof Error ? e.message : "Compare failed");
    } finally {
      setCompareBusy(false);
    }
  }

  function pickScheduler(s: string) {
    setSchedulerHint(s);
    const match = configs.find((c) => c.id.toLowerCase().includes(s));
    if (match) setSelected(match.id);
  }

  return (
    <>
      <PageHero
        kicker="ZYVOR JANUS · MISSION CONTROL"
        titleAccent={bannerTitle}
        title=""
        subtitle={healthLabel}
        status={health}
        icon={<PulseIcon />}
        actions={
          <>
            <AppLink href="/benchmark" showArrow={false}>
              Benchmark
            </AppLink>
            <AppLink href="/what-if" showArrow={false}>
              What-if
            </AppLink>
          </>
        }
      >
        <div className="stat-grid">
          <MetricTile label="Total Runs" value={String(runs.length)} sparkData={kpiHistory.total} />
          <MetricTile label="Running Now" value={String(runningCount)} sparkData={kpiHistory.running} />
          <MetricTile label="Completed" value={String(completedCount)} sparkData={kpiHistory.completed} />
          <MetricTile label="Failed" value={String(failedCount)} sparkData={kpiHistory.failed} />
        </div>
      </PageHero>

      <h2>Actions</h2>
      <div className="acts">
        <Card variant="action" title="▶ Launch simulation" description="Pick a cluster config and start a run.">
          <div className="mb-3">
            <p className="form-label mb-2">Scheduler quick pick</p>
            <SchedulerPillGroup value={schedulerHint} onChange={pickScheduler} />
          </div>
          <div className="act-row">
            <FormField label="Configuration" className="grow">
              <Select value={selected} onChange={(e) => setSelected(e.target.value)} disabled={runBusy || compareBusy}>
                {configs.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.id}
                  </option>
                ))}
              </Select>
            </FormField>
            <FormField label="Shadow vs (optional)">
              <Select
                value={shadowScheduler}
                onChange={(e) => setShadowScheduler(e.target.value)}
                disabled={runBusy || compareBusy}
              >
                <option value="">None</option>
                {SCHEDULERS.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </Select>
            </FormField>
            <Button disabled={runBusy || compareBusy || !selected} onClick={handleRun}>
              {runBusy ? "Starting…" : shadowScheduler ? "⚔ Run + shadow" : "▶ Run"}
            </Button>
          </div>
          {shadowScheduler ? (
            <p className="shadow-hint">
              <Swords size={12} strokeWidth={2} /> Steps <strong>{shadowScheduler}</strong> live alongside the
              primary scheduler over the same job arrivals.
            </p>
          ) : null}
          {runError ? <p className="inline-error-banner">{runError}</p> : null}
        </Card>

        <Card variant="action" title="⚙ Compare configs" description="Run two configs and compare metrics.">
          <div className="act-row">
            <FormField label="Config A">
              <Select value={compareA} onChange={(e) => setCompareA(e.target.value)} disabled={compareBusy}>
                <option value="">Select…</option>
                {configs.map((c) => (
                  <option key={c.id} value={c.id} disabled={c.id === compareB}>
                    {c.id}
                  </option>
                ))}
              </Select>
            </FormField>
            <FormField label="Config B">
              <Select value={compareB} onChange={(e) => setCompareB(e.target.value)} disabled={compareBusy}>
                <option value="">Select…</option>
                {configs.map((c) => (
                  <option key={c.id} value={c.id} disabled={c.id === compareA}>
                    {c.id}
                  </option>
                ))}
              </Select>
            </FormField>
            <Button variant="secondary" disabled={compareBusy || runBusy || !canCompare} onClick={handleCompare}>
              {compareBusy ? "Comparing…" : "Compare"}
            </Button>
          </div>
          {configs.length < 2 ? (
            <p className="compare-hint">Add at least two cluster configs to compare.</p>
          ) : null}
          {compareBusy ? <p className="compare-progress">Running both simulations…</p> : null}
          {compareError ? <p className="inline-error-banner">{compareError}</p> : null}
          <ComparePanel results={compareResults} />
        </Card>
      </div>

      <h2>Console</h2>
      <div className="qa">
        <div>
          <Card title="Quick links" description="Jump to analysis tools.">
            <div className="act-row">
              <AppLink href="/benchmark">Open Benchmark</AppLink>
            </div>
            <div className="act-row">
              <AppLink href="/what-if">Open What-if</AppLink>
            </div>
          </Card>
        </div>

        <div id="runs">
          <Card title="Recent runs" description="Click a column header to sort.">
            {runs.length === 0 ? (
              <EmptyState title="No runs yet" text="Start a simulation above to populate this table." />
            ) : (
              <div className="data-table-wrap">
                <table className="data-table runs-table">
                  <thead>
                    <tr>
                      <th className="sortable-th" onClick={() => toggleSort("config")}>
                        Config {sortKey === "config" ? (sortDir === "asc" ? "↑" : "↓") : ""}
                      </th>
                      <th>Run</th>
                      <th className="sortable-th" onClick={() => toggleSort("status")}>
                        Status {sortKey === "status" ? (sortDir === "asc" ? "↑" : "↓") : ""}
                      </th>
                      <th className="sortable-th" onClick={() => toggleSort("created_at")}>
                        Created {sortKey === "created_at" ? (sortDir === "asc" ? "↑" : "↓") : ""}
                      </th>
                      <th />
                    </tr>
                  </thead>
                  <motion.tbody variants={staggerContainer} initial="hidden" animate="visible">
                    {sortedRuns.map((run) => (
                      <Fragment key={run.id}>
                        <motion.tr
                          layout
                          variants={fadeInUp}
                          onMouseEnter={() => setPreviewId(run.id)}
                          onMouseLeave={() => setPreviewId(null)}
                        >
                          <td>{run.config}</td>
                          <td className="font-mono text-xs">{run.id.slice(0, 8)}</td>
                          <td>
                            <StatusBadge status={run.status} />
                          </td>
                          <td className="font-mono text-xs">{run.created_at.slice(0, 19)}</td>
                          <td>
                            <AppLink href={`/runs/${run.id}`}>Open</AppLink>
                          </td>
                        </motion.tr>
                        <AnimatePresence initial={false}>
                          {previewId === run.id ? (
                            <motion.tr
                              initial={{ opacity: 0 }}
                              animate={{ opacity: 1 }}
                              exit={{ opacity: 0 }}
                            >
                              <td colSpan={5} style={{ padding: 0 }}>
                                <motion.div
                                  initial={{ height: 0 }}
                                  animate={{ height: "auto" }}
                                  exit={{ height: 0 }}
                                  transition={{ duration: 0.2, ease: easeOut }}
                                  style={{ overflow: "hidden" }}
                                >
                                  <div style={{ padding: "10px 4px" }}>
                                    <span className="chip">scheduler: {run.scheduler ?? "default"}</span>{" "}
                                    {run.shadow_scheduler ? (
                                      <span className="chip">
                                        <Swords size={10} strokeWidth={2} style={{ display: "inline", marginRight: 4 }} />
                                        shadow: {run.shadow_scheduler}
                                      </span>
                                    ) : null}{" "}
                                    <span className="chip">
                                      finished: {run.finished_at ? run.finished_at.slice(0, 19) : "—"}
                                    </span>
                                  </div>
                                </motion.div>
                              </td>
                            </motion.tr>
                          ) : null}
                        </AnimatePresence>
                      </Fragment>
                    ))}
                  </motion.tbody>
                </table>
              </div>
            )}
          </Card>
        </div>

        <div>
          <Card title="Activity" description="Scheduler decisions from the latest run.">
            {activity.length === 0 ? (
              <EmptyState title="No activity yet" text="Run a simulation to see live events." />
            ) : (
              <div className="activity-feed feed">
                <AnimatePresence initial={false}>
                  {activity.map((ev) => (
                    <motion.div
                      key={`${ev.time}-${ev.kind}-${ev.job_id ?? ev.message}`}
                      layout
                      initial={{ opacity: 0, x: -10 }}
                      animate={{ opacity: 1, x: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.25, ease: easeOut }}
                      className="activity-item feed-item"
                    >
                      <span className="activity-item-time f-when">t={ev.time.toFixed(2)}s</span>
                      <StatusBadge status={ev.kind.replace(/_/g, " ")} />
                      <p className="activity-item-msg">{ev.message}</p>
                    </motion.div>
                  ))}
                </AnimatePresence>
              </div>
            )}
          </Card>
        </div>
      </div>
    </>
  );
}
