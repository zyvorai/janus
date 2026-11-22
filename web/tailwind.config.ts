// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./src/**/*.{js,ts,jsx,tsx,mdx}"],
  theme: {
    extend: {
      colors: {
        hs: {
          bg: "var(--hs-bg)",
          "bg-alt": "var(--hs-bg-alt)",
          surface: "var(--hs-surface)",
          "surface-code": "var(--hs-surface-code)",
          accent: "var(--hs-accent)",
          "accent-deep": "var(--hs-accent-deep)",
          "accent-light": "var(--hs-accent-light)",
          "accent-orange": "var(--hs-accent-orange)",
          indigo: "var(--hs-indigo)",
          purple: "var(--hs-purple)",
          info: "var(--hs-info)",
          teal: "var(--hs-teal)",
          success: "var(--hs-success)",
          error: "var(--hs-error)",
          warning: "var(--hs-warning)",
          white: "var(--hs-text-white)",
          heading: "var(--hs-text-heading)",
          body: "var(--hs-text-body)",
          muted: "var(--hs-text-muted)",
          subtle: "var(--hs-text-subtle)",
          dim: "var(--hs-text-dim)",
          border: "var(--hs-border)",
          "border-accent": "var(--hs-border-accent)",
          "border-accent-strong": "var(--hs-border-accent-strong)",
        },
        idle: "#22c55e",
        training: "#6366f1",
        inference: "#f97316",
        overloaded: "#ef4444",
      },
      borderRadius: {
        hs: "var(--hs-radius)",
        "hs-lg": "var(--hs-radius-lg)",
        "hs-xl": "var(--hs-radius-xl)",
      },
      boxShadow: {
        "hs-elev-1": "var(--hs-elev-1)",
        "hs-elev-2": "var(--hs-elev-2)",
        "hs-elev-3": "var(--hs-elev-3)",
        "hs-card": "var(--hs-shadow-card)",
        "hs-accent": "var(--hs-shadow-accent)",
      },
      fontFamily: {
        sans: ["var(--font-inter)", "system-ui", "sans-serif"],
        mono: ["var(--font-jetbrains)", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
export default config;
