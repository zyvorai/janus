// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { Fragment } from "react";
import type { JobRunSegment, JobsTimeline } from "@/types/simulation";
import { ganttColors } from "@/lib/theme";
import { Card } from "./ui";

/** Row key: prefer node/gpu when segment carries node_ids. */
function rowKey(gpuId: string, nodeId?: string): string {
  return nodeId ? `${nodeId}/${gpuId}` : gpuId;
}

function jobGpuIds(job: JobsTimeline["jobs"][0]): string[] {
  const fromSegs = (job.segments ?? []).flatMap((s) => s.gpu_ids);
  if (fromSegs.length) return Array.from(new Set(fromSegs));
  return job.assigned_gpus;
}

function gpuNodePairs(timeline: JobsTimeline): { key: string; gpuId: string; nodeId?: string }[] {
  const map = new Map<string, { key: string; gpuId: string; nodeId?: string }>();
  for (const job of timeline.jobs) {
    for (const seg of job.segments ?? []) {
      seg.gpu_ids.forEach((gpuId, i) => {
        const nodeId = seg.node_ids?.[i];
        const key = rowKey(gpuId, nodeId);
        if (!map.has(key)) map.set(key, { key, gpuId, nodeId });
      });
    }
    for (const gpuId of job.assigned_gpus) {
      const key = rowKey(gpuId);
      if (!map.has(key)) map.set(key, { key, gpuId });
    }
  }
  const pairs = Array.from(map.values()).sort((a, b) => a.key.localeCompare(b.key));
  if (pairs.length >= timeline.gpu_count) return pairs;
  for (let i = 0; pairs.length < timeline.gpu_count; i++) {
    const gpuId = `gpu-${i}`;
    const key = rowKey(gpuId);
    if (!pairs.some((p) => p.key === key)) pairs.push({ key, gpuId });
  }
  return pairs.sort((a, b) => a.key.localeCompare(b.key));
}

export function GanttChart({ timeline }: { timeline: JobsTimeline | null }) {
  if (!timeline?.jobs?.length) {
    return <Card title="Job Timeline (final summary)">No jobs in timeline.</Card>;
  }

  const makespan = timeline.makespan || 1;
  const rows = gpuNodePairs(timeline);
  const unassigned = timeline.jobs.filter(
    (j) => jobGpuIds(j).length === 0 && j.state !== "failed",
  );
  const displayRows = rows.length
    ? rows
    : unassigned.length
      ? [{ key: "unassigned", gpuId: "unassigned" as string }]
      : [{ key: "gpu-0", gpuId: "gpu-0" }];

  function renderSegmentBar(job: JobsTimeline["jobs"][0], seg: JobRunSegment, idx: number) {
    const left = (seg.start / makespan) * 100;
    const width = ((seg.end - seg.start) / makespan) * 100;
    const where =
      seg.node_ids?.length
        ? seg.gpu_ids.map((g, i) => `${seg.node_ids![i] ?? "?"}/${g}`).join(",")
        : seg.gpu_ids.join(",");
    return (
      <div
        key={`${job.job_id}-seg-${idx}`}
        className="absolute top-1 h-4 rounded gantt-bar-grow"
        style={{
          left: `${left}%`,
          width: `${Math.max(width, 0.5)}%`,
          backgroundColor: `${ganttColors.run}CC`,
        }}
        title={`run: ${job.name} [${where}] ${seg.start.toFixed(1)}–${seg.end.toFixed(1)}`}
        aria-label={`${job.name} running on ${where}`}
      />
    );
  }

  function renderJobBar(
    job: JobsTimeline["jobs"][0],
    gpuId: string,
    nodeId?: string,
  ) {
    if (job.state === "failed") {
      const left = (job.arrival_time / makespan) * 100;
      const width = (((job.finish_time ?? job.arrival_time) - job.arrival_time) / makespan) * 100;
      return (
        <div
          key={`${job.job_id}-failed`}
          className="absolute top-1 h-4 rounded border border-dashed gantt-bar-grow"
          style={{
            left: `${left}%`,
            width: `${Math.max(width, 1)}%`,
            borderColor: ganttColors.failed,
            backgroundColor: `${ganttColors.failed}80`,
          }}
          title={`${job.name} (failed)`}
          aria-label={`${job.name} failed`}
        />
      );
    }

    const segmentsOnGpu = (job.segments ?? []).filter((s) =>
      s.gpu_ids.some((g, i) => g === gpuId && (!nodeId || s.node_ids?.[i] === nodeId || !s.node_ids?.length)),
    );
    if (segmentsOnGpu.length > 0) {
      const firstStart = Math.min(...(job.segments ?? []).map((s) => s.start));
      const waitLeft = (job.arrival_time / makespan) * 100;
      const waitWidth = ((firstStart - job.arrival_time) / makespan) * 100;
      const firstGpu = job.segments?.[0]?.gpu_ids[0] ?? "";
      const firstNode = job.segments?.[0]?.node_ids?.[0];
      const showWait =
        gpuId === firstGpu &&
        (!nodeId || !firstNode || nodeId === firstNode) &&
        waitWidth > 0;
      return (
        <Fragment key={job.job_id}>
          {showWait ? (
            <div
              className="absolute top-1 h-4 rounded gantt-bar-grow"
              style={{
                left: `${waitLeft}%`,
                width: `${waitWidth}%`,
                backgroundColor: `${ganttColors.wait}B3`,
              }}
              title={`wait: ${job.name}`}
              aria-label={`${job.name} waiting`}
            />
          ) : null}
          {segmentsOnGpu.map((seg, idx) => renderSegmentBar(job, seg, idx))}
        </Fragment>
      );
    }

    if (job.start_time == null) {
      const left = (job.arrival_time / makespan) * 100;
      const end = job.finish_time ?? makespan;
      const width = ((end - job.arrival_time) / makespan) * 100;
      return (
        <div
          key={`${job.job_id}-unschedulable`}
          className="absolute top-1 h-4 rounded border border-dashed gantt-bar-grow"
          style={{
            left: `${left}%`,
            width: `${Math.max(width, 1)}%`,
            borderColor: ganttColors.wait,
            backgroundColor: `${ganttColors.wait}66`,
          }}
          title={`${job.name} (${job.state}, never started on ${gpuId})`}
          aria-label={`${job.name} ${job.state}`}
        />
      );
    }

    if (!job.assigned_gpus.includes(gpuId)) return null;
    const waitLeft = (job.arrival_time / makespan) * 100;
    const waitWidth = ((job.start_time - job.arrival_time) / makespan) * 100;
    const runLeft = (job.start_time / makespan) * 100;
    const runEnd = job.finish_time ?? job.start_time + job.runtime;
    const runWidth = ((runEnd - job.start_time) / makespan) * 100;

    return (
      <Fragment key={job.job_id}>
        {waitWidth > 0 ? (
          <div
            className="absolute top-1 h-4 rounded gantt-bar-grow"
            style={{
              left: `${waitLeft}%`,
              width: `${waitWidth}%`,
              backgroundColor: `${ganttColors.wait}B3`,
            }}
            title={`wait: ${job.name}`}
            aria-label={`${job.name} waiting`}
          />
        ) : null}
        <div
          className="absolute top-1 h-4 rounded gantt-bar-grow"
          style={{
            left: `${runLeft}%`,
            width: `${Math.max(runWidth, 0.5)}%`,
            backgroundColor: `${ganttColors.run}CC`,
          }}
          title={`run: ${job.name}`}
          aria-label={`${job.name} running`}
        />
      </Fragment>
    );
  }

  return (
    <Card
      title="Job Timeline (final summary)"
      description="Full-run Gantt — rows are machine/GPU. Preempt resume = placement migrate, not live CUDA."
    >
      <div className="space-y-2">
        {displayRows.map((row) => {
          const jobsOnGpu =
            row.gpuId === "unassigned"
              ? unassigned
              : timeline.jobs.filter((j) => {
                  if (
                    (j.segments ?? []).some((s) =>
                      s.gpu_ids.some(
                        (g, i) =>
                          g === row.gpuId &&
                          (!row.nodeId ||
                            !s.node_ids?.length ||
                            s.node_ids[i] === row.nodeId),
                      ),
                    )
                  ) {
                    return true;
                  }
                  return j.assigned_gpus.includes(row.gpuId);
                });
          return (
            <div key={row.key} className="flex items-center gap-2 text-xs">
              <div className="w-28 shrink-0 font-mono text-hs-muted" title={row.key}>
                {row.key}
              </div>
              <div
                className="relative h-6 flex-1 rounded"
                style={{ backgroundColor: ganttColors.track }}
              >
                {jobsOnGpu.map((job) => renderJobBar(job, row.gpuId, row.nodeId))}
              </div>
            </div>
          );
        })}
      </div>
      <div className="mt-3 flex flex-wrap gap-4 text-xs text-hs-muted">
        <span className="inline-flex items-center gap-1">
          <span className="h-2 w-4 rounded" style={{ backgroundColor: `${ganttColors.wait}B3` }} /> wait
        </span>
        <span className="inline-flex items-center gap-1">
          <span className="h-2 w-4 rounded" style={{ backgroundColor: `${ganttColors.run}CC` }} /> run
        </span>
        <span className="inline-flex items-center gap-1">
          <span
            className="h-2 w-4 rounded border border-dashed"
            style={{ borderColor: ganttColors.wait, backgroundColor: `${ganttColors.wait}66` }}
          />{" "}
          unschedulable
        </span>
        <span className="inline-flex items-center gap-1">
          <span
            className="h-2 w-4 rounded border border-dashed"
            style={{ borderColor: ganttColors.failed, backgroundColor: `${ganttColors.failed}80` }}
          />{" "}
          failed
        </span>
      </div>
    </Card>
  );
}
