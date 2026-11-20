// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import type {
  ClusterSnapshot,
  CompareResult,
  ConfigEntry,
  JobsTimeline,
  RunDetail,
  RunSummary,
  SchedulerDecision,
  SimulationMetrics,
  TwinEntry,
} from "@/types/simulation";

const API = "/api";

function redirectToLogin() {
  if (typeof window !== "undefined") {
    const next = encodeURIComponent(window.location.pathname);
    window.location.href = `/login?next=${next}`;
  }
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API}${path}`, {
    ...init,
    credentials: "include",
  });
  if (res.status === 401) {
    redirectToLogin();
    throw new Error("unauthorized");
  }
  if (!res.ok) {
    let detail: string | undefined;
    try {
      const body = (await res.json()) as { detail?: string };
      detail = body.detail;
    } catch {
      /* non-JSON error body */
    }
    throw new Error(detail ?? `request failed: ${path} (${res.status})`);
  }
  return res.json() as Promise<T>;
}

export async function fetchConfigs(): Promise<ConfigEntry[]> {
  return apiFetch<ConfigEntry[]>("/configs");
}

export async function fetchRuns(): Promise<RunSummary[]> {
  return apiFetch<RunSummary[]>("/runs");
}

export async function startRun(
  config: string,
  opts?: { scheduler?: string; shadowScheduler?: string }
): Promise<{ id: string }> {
  return apiFetch<{ id: string }>("/runs", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      config,
      scheduler: opts?.scheduler,
      shadow_scheduler: opts?.shadowScheduler || undefined,
    }),
  });
}

export async function fetchRun(id: string): Promise<RunDetail> {
  return apiFetch<RunDetail>(`/runs/${id}`);
}

export async function fetchTimeline(id: string): Promise<JobsTimeline> {
  return apiFetch<JobsTimeline>(`/runs/${id}/timeline`);
}

export async function fetchEvents(id: string): Promise<SchedulerDecision[]> {
  return apiFetch<SchedulerDecision[]>(`/runs/${id}/events`);
}

export async function fetchShadowEvents(id: string): Promise<SchedulerDecision[]> {
  return apiFetch<SchedulerDecision[]>(`/runs/${id}/shadow-events`);
}

export async function fetchSnapshots(id: string): Promise<ClusterSnapshot[]> {
  return apiFetch<ClusterSnapshot[]>(`/runs/${id}/snapshots`);
}

/**
 * Mints a short-lived ticket for the `/ws/runs/{id}` upgrade. The Rust API
 * requires it as a query param because browsers cannot set an
 * `Authorization` header on a WebSocket handshake, so the bearer token
 * `web/src/middleware.ts` injects on ordinary `/api/*` calls can't reach
 * this route -- goes through apiFetch so it still carries the session
 * cookie and gets the same 401-redirect-to-login handling as everything
 * else.
 */
export async function fetchWsTicket(id: string): Promise<string> {
  const { ticket } = await apiFetch<{ ticket: string }>(`/runs/${id}/ws-ticket`, {
    method: "POST",
  });
  return ticket;
}

/**
 * Host-relative by default so it keeps working regardless of which fixed port the
 * servers are bound to, proxied through next.config.js's /ws/:path* rewrite.
 *
 * Next's rewrite cannot proxy WebSocket upgrade requests (next dev / next start both
 * throw internally on an upgrade to a rewritten route — confirmed via manual testing),
 * so NEXT_PUBLIC_ZYVOR_JANUS_WS_URL lets local dev / non-Ingress deployments point the
 * browser straight at the API instead. Unset by default: behind an Ingress (see
 * deploy/kubernetes/ingress.yaml), /ws/* is already routed directly to the API
 * service, bypassing the Next.js layer entirely, so the default same-origin path
 * works correctly there without this override.
 */
export function runWebSocketUrl(id: string, ticket: string): string {
  const override = process.env.NEXT_PUBLIC_ZYVOR_JANUS_WS_URL;
  const query = `?ticket=${encodeURIComponent(ticket)}`;
  if (override) return `${override.replace(/\/+$/, "")}/ws/runs/${id}${query}`;
  const proto = typeof window !== "undefined" && window.location.protocol === "https:" ? "wss" : "ws";
  const host = typeof window !== "undefined" ? window.location.host : "";
  return `${proto}://${host}/ws/runs/${id}${query}`;
}

export async function compareConfigs(configs: string[]): Promise<{ results: CompareResult[] }> {
  return apiFetch("/compare", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ configs }),
  });
}

export async function fetchBenchmarkPresets(): Promise<{
  configs: ConfigEntry[];
  workload_presets: Array<{ id: string; description: string }>;
}> {
  return apiFetch("/benchmark/presets");
}

export async function fetchBenchmarkReports(): Promise<
  Array<{
    run_id: string;
    config: string;
    scheduler: string | null;
    metrics: SimulationMetrics | null;
    benchmark: import("@/types/simulation").SchedulerBenchmarkReport | null;
  }>
> {
  return apiFetch("/benchmark/reports");
}

export async function runBenchmark(
  config: string,
  scheduler?: string,
): Promise<{
  run_id: string;
  metrics: SimulationMetrics;
  benchmark: import("@/types/simulation").SchedulerBenchmarkReport | null;
}> {
  return apiFetch("/benchmark/run", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ config, scheduler }),
  });
}

export async function runWhatIf(
  baseConfig: string,
  schedulers: string[],
): Promise<{ results: Array<Record<string, unknown>> }> {
  return apiFetch("/what-if", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ base_config: baseConfig, schedulers }),
  });
}

export async function fetchTwins(): Promise<TwinEntry[]> {
  return apiFetch<TwinEntry[]>("/twins");
}

export function pollRun(id: string, onUpdate: (run: RunDetail) => void, intervalMs = 1000): () => void {
  let active = true;
  const tick = async () => {
    while (active) {
      try {
        const run = await fetchRun(id);
        onUpdate(run);
        if (run.status === "completed" || run.status === "failed") break;
      } catch (e) {
        const message = e instanceof Error ? e.message : "";
        if (message === "unauthorized" || message.includes("(404)")) break;
      }
      await new Promise((r) => setTimeout(r, intervalMs));
    }
  };
  tick();
  return () => {
    active = false;
  };
}
