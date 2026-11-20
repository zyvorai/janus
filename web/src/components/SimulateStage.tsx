// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type { ClusterSnapshot, SchedulerDecision } from "@/types/simulation";

type Sprite = {
  id: string;
  label: string;
  kind: string;
  x: number;
  y: number;
  targetX: number;
  targetY: number;
  born: number;
  ttl: number;
};

function hashHue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return h % 360;
}

function gpuClass(busy: boolean, util: number): string {
  if (!busy) return "sim-gpu idle";
  if (util >= 0.95) return "sim-gpu hot";
  return "sim-gpu busy";
}

export function SimulateStage({
  snapshot,
  decision,
  reducedMotion = false,
}: {
  snapshot: ClusterSnapshot | null;
  decision: SchedulerDecision | null;
  reducedMotion?: boolean;
}) {
  const stageRef = useRef<HTMLDivElement>(null);
  const gpuRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const spritesRef = useRef<Sprite[]>([]);
  const [sprites, setSprites] = useState<Sprite[]>([]);
  const lastDecisionKey = useRef<string>("");
  /** Last GPU a job occupied (from schedule/preempt) — used for GPU→GPU migrate sprites. */
  const lastGpuByJob = useRef<Map<string, string>>(new Map());

  const gpus = useMemo(() => {
    if (!snapshot) return [];
    return snapshot.nodes.flatMap((n) => n.gpus.map((g) => ({ ...g, nodeId: n.id })));
  }, [snapshot]);

  const queueJobs = snapshot?.queue_jobs ?? [];

  useEffect(() => {
    if (!decision || !stageRef.current) return;
    const key = `${decision.time}-${decision.kind}-${decision.job_id ?? ""}-${decision.message}`;
    if (key === lastDecisionKey.current) return;
    lastDecisionKey.current = key;

    const stage = stageRef.current.getBoundingClientRect();
    let startX = 48;
    let startY = Math.min(stage.height * 0.35, 160);

    let targetX = stage.width * 0.55;
    let targetY = stage.height * 0.45;
    const gpuId = decision.gpu_ids[0];
    const jobKey = decision.job_id ?? decision.job_name ?? "";

    if (decision.kind.includes("preempt") && gpuId && jobKey) {
      lastGpuByJob.current.set(jobKey, gpuId);
    }

    // After preemption, reschedule: fly from previous GPU tile → new GPU.
    if (
      decision.kind.includes("scheduled") &&
      jobKey &&
      lastGpuByJob.current.has(jobKey) &&
      gpuId &&
      lastGpuByJob.current.get(jobKey) !== gpuId
    ) {
      const prevId = lastGpuByJob.current.get(jobKey)!;
      const prevEl = gpuRefs.current.get(prevId);
      if (prevEl) {
        const r = prevEl.getBoundingClientRect();
        startX = r.left - stage.left + r.width / 2;
        startY = r.top - stage.top + r.height / 2;
      }
    }

    if (gpuId) {
      const el = gpuRefs.current.get(gpuId);
      if (el) {
        const r = el.getBoundingClientRect();
        targetX = r.left - stage.left + r.width / 2;
        targetY = r.top - stage.top + r.height / 2;
      }
      if (decision.kind.includes("scheduled") && jobKey) {
        lastGpuByJob.current.set(jobKey, gpuId);
      }
    } else if (decision.kind.includes("arrival")) {
      targetX = 120;
      targetY = stage.height * 0.55;
    } else if (decision.kind.includes("complete")) {
      targetX = stage.width - 64;
      targetY = 48;
      if (jobKey) lastGpuByJob.current.delete(jobKey);
    }

    const sprite: Sprite = {
      id: `${key}-${Date.now()}`,
      label: decision.job_name ?? decision.job_id ?? decision.kind.replace(/_/g, " "),
      kind: decision.kind,
      x: startX,
      y: startY,
      targetX,
      targetY,
      born: performance.now(),
      ttl: reducedMotion ? 80 : 1400,
    };
    spritesRef.current = [...spritesRef.current.slice(-24), sprite];
    setSprites(spritesRef.current);
  }, [decision, reducedMotion]);

  useEffect(() => {
    if (reducedMotion || sprites.length === 0) return;
    let raf = 0;
    let alive = true;
    const tick = (now: number) => {
      if (!alive) return;
      const next = spritesRef.current
        .map((s) => {
          const t = Math.min(1, (now - s.born) / Math.max(s.ttl, 1));
          const ease = 1 - Math.pow(1 - t, 3);
          return {
            ...s,
            x: s.x + (s.targetX - s.x) * (0.08 + ease * 0.12),
            y: s.y + (s.targetY - s.y) * (0.08 + ease * 0.12),
          };
        })
        .filter((s) => now - s.born < s.ttl + 180);
      spritesRef.current = next;
      setSprites(next);
      if (next.length > 0) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
    };
  }, [reducedMotion, sprites.length]);

  return (
    <div className="sim-stage glass card" ref={stageRef}>
      <div className="sim-stage-bg" aria-hidden="true" />
      <div className="sim-flow-label sim-flow-ingress">Requests</div>
      <div className="sim-flow-label sim-flow-queue">Queue</div>
      <div className="sim-flow-label sim-flow-gpus">GPUs</div>
      <div className="sim-flow-label sim-flow-done">Done</div>

      <div className="sim-queue-rail" aria-label="Request queue">
        {queueJobs.length === 0 ? (
          <span className="sim-queue-empty">empty</span>
        ) : (
          queueJobs.slice(0, 12).map((j) => (
            <div
              key={j.id}
              className="sim-queue-chip"
              style={{ ["--chip-hue" as string]: hashHue(j.id) }}
              title={`${j.name} · ${j.gpu_count} GPU · p${j.priority}`}
            >
              {j.name}
            </div>
          ))
        )}
      </div>

      <div className="sim-gpu-field">
        {gpus.length === 0 ? (
          <div className="sim-empty">No cluster snapshot yet — launch a run to feed the stage.</div>
        ) : (
          snapshot!.nodes.map((node) => (
            <div key={node.id} className="sim-node">
              <div className="sim-node-label">{node.id}</div>
              <div className="sim-gpu-grid">
                {node.gpus.map((gpu) => (
                  <div
                    key={gpu.id}
                    ref={(el) => {
                      if (el) gpuRefs.current.set(gpu.id, el);
                      else gpuRefs.current.delete(gpu.id);
                    }}
                    className={gpuClass(gpu.busy, gpu.utilization)}
                    title={gpu.job_name ?? "idle"}
                  >
                    <div className="sim-gpu-id">{gpu.id}</div>
                    <div className="sim-gpu-job">{gpu.busy ? gpu.job_name ?? "busy" : "idle"}</div>
                    <div
                      className="sim-gpu-bar"
                      style={{ width: `${Math.round(Math.min(1, gpu.utilization) * 100)}%` }}
                    />
                    {gpu.busy ? <span className="sim-gpu-pulse" aria-hidden="true" /> : null}
                  </div>
                ))}
              </div>
            </div>
          ))
        )}
      </div>

      {sprites.map((s) => (
        <div
          key={s.id}
          className={`sim-sprite sim-sprite-${s.kind.includes("preempt") ? "preempt" : s.kind.includes("complete") ? "done" : "move"}`}
          style={{
            left: s.x,
            top: s.y,
            ["--sprite-hue" as string]: hashHue(s.label),
          }}
        >
          {s.label}
        </div>
      ))}

      {decision ? (
        <div className="sim-event-toast">
          <span className="sim-event-t">t={decision.time.toFixed(2)}s</span>
          <span className="sim-event-kind">{decision.kind.replace(/_/g, " ")}</span>
          <span className="sim-event-msg">{decision.message}</span>
        </div>
      ) : null}
    </div>
  );
}
