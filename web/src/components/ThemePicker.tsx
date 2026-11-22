// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import { useEffect, useRef, useState } from "react";
import { Palette } from "lucide-react";
import clsx from "clsx";
import {
  APP_THEMES,
  THEME_LABELS,
  THEME_SWATCHES,
  useThemeStore,
  type AppTheme,
} from "@/store/useThemeStore";

export function ThemePicker() {
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="theme-picker" ref={rootRef}>
      <button
        type="button"
        className="theme-picker-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label="Appearance theme"
        onClick={() => setOpen((o) => !o)}
      >
        <Palette size={15} strokeWidth={1.75} />
        <span className="hidden sm:inline">{THEME_LABELS[theme]}</span>
      </button>
      {open ? (
        <div className="theme-picker-panel" role="listbox" aria-label="Themes">
          <div className="theme-picker-title">Appearance</div>
          <div className="theme-picker-grid">
            {APP_THEMES.map((t: AppTheme) => {
              const sw = THEME_SWATCHES[t];
              return (
                <button
                  key={t}
                  type="button"
                  role="option"
                  aria-selected={theme === t}
                  className={clsx("theme-swatch", theme === t && "theme-swatch-active")}
                  onClick={() => {
                    setTheme(t);
                    setOpen(false);
                  }}
                >
                  <span
                    className="theme-swatch-preview"
                    style={
                      {
                        "--swatch-bg": sw.bg,
                        "--swatch-a": sw.a,
                        "--swatch-b": sw.b,
                      } as React.CSSProperties
                    }
                  />
                  <span className="theme-swatch-label">{THEME_LABELS[t]}</span>
                </button>
              );
            })}
          </div>
        </div>
      ) : null}
    </div>
  );
}
