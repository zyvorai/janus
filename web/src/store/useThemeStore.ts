// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { create } from "zustand";

export type AppTheme =
  | "zyvor-janus"
  | "tahoe"
  | "aurora"
  | "dark-steel"
  | "deep-navy"
  | "emerald-noir"
  | "tahoe-light";

export const APP_THEMES: AppTheme[] = [
  "zyvor-janus",
  "tahoe",
  "aurora",
  "dark-steel",
  "deep-navy",
  "emerald-noir",
  "tahoe-light",
];

export const THEME_LABELS: Record<AppTheme, string> = {
  "zyvor-janus": "Zyvor Janus",
  tahoe: "Tahoe",
  aurora: "Aurora",
  "dark-steel": "Dark Steel",
  "deep-navy": "Deep Navy",
  "emerald-noir": "Emerald Noir",
  "tahoe-light": "Tahoe Light",
};

/** Mini preview colors for theme picker swatches */
export const THEME_SWATCHES: Record<AppTheme, { bg: string; a: string; b: string }> = {
  "zyvor-janus": { bg: "#070a10", a: "rgba(240,88,58,0.55)", b: "rgba(249,115,22,0.35)" },
  tahoe: { bg: "#070a10", a: "rgba(59,130,246,0.55)", b: "rgba(56,189,248,0.35)" },
  aurora: { bg: "#020408", a: "rgba(34,211,238,0.5)", b: "rgba(168,85,247,0.4)" },
  "dark-steel": { bg: "#080c12", a: "rgba(91,155,213,0.45)", b: "rgba(148,163,184,0.3)" },
  "deep-navy": { bg: "#050914", a: "rgba(79,140,255,0.5)", b: "rgba(37,99,235,0.35)" },
  "emerald-noir": { bg: "#050a08", a: "rgba(52,211,153,0.5)", b: "rgba(45,212,191,0.35)" },
  "tahoe-light": { bg: "#e8eef8", a: "rgba(37,99,235,0.35)", b: "rgba(14,165,233,0.28)" },
};

const STORAGE_KEY = "zyvor-janus-app-theme";

export function isAppTheme(v: string | null | undefined): v is AppTheme {
  return !!v && (APP_THEMES as string[]).includes(v);
}

export function applyThemeToDocument(theme: AppTheme) {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = theme;
}

export function readStoredTheme(): AppTheme {
  if (typeof window === "undefined") return "zyvor-janus";
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isAppTheme(stored)) return stored;
  } catch {
    /* ignore */
  }
  return "zyvor-janus";
}

interface ThemeState {
  theme: AppTheme;
  setTheme: (t: AppTheme) => void;
  cycleTheme: () => void;
  hydrate: () => void;
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  theme: "zyvor-janus",

  hydrate: () => {
    const t = readStoredTheme();
    applyThemeToDocument(t);
    set({ theme: t });
  },

  setTheme: (t) => {
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch {
      /* ignore */
    }
    applyThemeToDocument(t);
    set({ theme: t });
  },

  cycleTheme: () => {
    const i = APP_THEMES.indexOf(get().theme);
    const next = APP_THEMES[(i + 1) % APP_THEMES.length];
    get().setTheme(next);
  },
}));
