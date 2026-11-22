// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

/** Zyvor / HyperSDK chart and topology colors (mirrors python/zyvor_janus/theme.py). */

export const theme = {
  accent: "#f0583a",
  accentOrange: "#f97316",
  accentDeep: "#d94b32",
  success: "#22c55e",
  teal: "#10b981",
  indigo: "#6366f1",
  info: "#06b6d4",
  error: "#ef4444",
  bg: "#050505",
  surface: "#0b0f14",
  surfaceCode: "#101722",
  textMuted: "#aeb9c8",
  textBody: "#cbd5e1",
  border: "rgba(255,255,255,0.06)",
  grid: "rgba(255,255,255,0.06)",
} as const;

export const gpuStateColors = {
  idle: theme.success,
  busy: theme.indigo,
  overloaded: theme.error,
} as const;

export const ganttColors = {
  wait: theme.accentOrange,
  run: theme.teal,
  failed: theme.error,
  unscheduled: theme.textMuted,
  track: theme.surfaceCode,
} as const;

export const chartColors = {
  bar: theme.accent,
  line: theme.info,
  tick: theme.textMuted,
  grid: theme.grid,
} as const;
