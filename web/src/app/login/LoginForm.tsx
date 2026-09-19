// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

"use client";

import { useSearchParams } from "next/navigation";
import { FormEvent, useEffect, useState, type CSSProperties } from "react";
import { DEFAULT_PASSWORD, DEFAULT_USERNAME } from "@/lib/auth";
import { safeRelativePath } from "@/lib/navigation";

function EyeIcon({ open }: { open: boolean }) {
  if (open) {
    return (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" className="h-5 w-5">
        <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" className="h-5 w-5">
      <path d="M3 3l18 18" />
      <path d="M10.58 10.58A3 3 0 0 0 12 15a3 3 0 0 0 2.42-4.42" />
      <path d="M9.88 5.1A10.8 10.8 0 0 1 12 5c6.5 0 10 7 10 7a17.4 17.4 0 0 1-2.12 3.17" />
      <path d="M6.12 6.12A17.4 17.4 0 0 0 4 12s3.5 7 10 7a10.8 10.8 0 0 0 3.9-.72" />
    </svg>
  );
}

const ORBS = [
  { size: 340, top: "4%", left: "6%", del: "0s", dur: "11s", hue: "blue" },
  { size: 220, top: "55%", left: "12%", del: "2.2s", dur: "13s", hue: "violet" },
  { size: 180, top: "18%", left: "58%", del: "0.8s", dur: "9s", hue: "cyan" },
  { size: 400, top: "58%", left: "68%", del: "3.2s", dur: "15s", hue: "red" },
] as const;

export function LoginForm() {
  const searchParams = useSearchParams();
  const nextPath = safeRelativePath(searchParams.get("next"));
  const [username, setUsername] = useState(DEFAULT_USERNAME);
  const [password, setPassword] = useState(DEFAULT_PASSWORD);
  const [showPassword, setShowPassword] = useState(false);
  const [remember, setRemember] = useState(true);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    try {
      const saved = localStorage.getItem("zyvor-janus-login-user");
      if (saved) setUsername(saved);
    } catch {
      /* ignore */
    }
  }, []);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password }),
        credentials: "include",
      });
      if (!res.ok) {
        const data = (await res.json().catch(() => null)) as { detail?: string } | null;
        setError(data?.detail ?? `Login failed (${res.status})`);
        return;
      }
      try {
        if (remember) localStorage.setItem("zyvor-janus-login-user", username.trim());
        else localStorage.removeItem("zyvor-janus-login-user");
      } catch {
        /* ignore */
      }
      window.location.assign(nextPath);
    } catch {
      setError("Unable to reach the server");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="login-page">
      <div className="login-aurora" aria-hidden="true" />
      <div className="login-scanline" aria-hidden="true" />

      <aside className="login-hero">
        <div className="login-spotlight" aria-hidden="true" />
        <div id="orbs" aria-hidden="true">
          {ORBS.map((o, i) => (
            <div
              key={i}
              className={`login-orb orb-${o.hue}`}
              style={
                {
                  width: o.size,
                  height: o.size,
                  top: o.top,
                  left: o.left,
                  ["--del" as string]: o.del,
                  ["--dur" as string]: o.dur,
                } as CSSProperties
              }
            />
          ))}
        </div>
        <div id="particles" aria-hidden="true">
          {Array.from({ length: 28 }, (_, i) => {
            const size = 2 + (i % 3);
            return (
              <span
                key={i}
                className="login-particle"
                style={{
                  left: `${(i * 17 + 7) % 100}%`,
                  top: `${(i * 23 + 11) % 100}%`,
                  width: size,
                  height: size,
                  animationDelay: `${(i % 7) * 0.45}s`,
                }}
              />
            );
          })}
        </div>

        <div style={{ position: "relative", zIndex: 1 }}>
          <div className="login-fade-in" style={{ marginBottom: 32 }}>
            <div className="brand-name">Zyvor Janus</div>
            <div className="brand-sub">ZYVOR</div>
          </div>
          <h2 className="hero-headline login-fade-in d1">
            GPU scheduler simulation for the <span className="login-text-gradient">Zynera platform</span>
          </h2>
          <p className="hero-sub login-fade-in d2">
            Launch clusters, replay scheduler decisions, benchmark policies, and run what-if sweeps —
            without needing physical NVIDIA GPUs.
          </p>
          <div className="pills login-fade-in d3">
            <span className="login-stat-pill">Discrete-event</span>
            <span className="login-stat-pill">MIG · Topology</span>
            <span className="login-stat-pill">Replay</span>
            <span className="login-stat-pill login-stat-pill-glow">What-if</span>
          </div>
        </div>

        <div className="features">
          <div className="login-feature-card login-fade-in" style={{ animationDelay: "0.35s", opacity: 0 }}>
            <div className="feature-icon">▶</div>
            <div>
              <div className="feature-title">Run &amp; compare</div>
              <p className="feature-desc">FIFO, priority, preemptive, Zynera, and best-fit side by side.</p>
            </div>
          </div>
          <div className="login-feature-card login-feature-card-highlight login-fade-in" style={{ animationDelay: "0.42s", opacity: 0 }}>
            <div className="feature-icon">⏱</div>
            <div>
              <div className="feature-title">Live replay scrubber</div>
              <p className="feature-desc">Step through scheduler decisions with cluster, topology, and MIG views.</p>
            </div>
          </div>
          <div className="login-feature-card login-fade-in" style={{ animationDelay: "0.49s", opacity: 0 }}>
            <div className="feature-icon">∑</div>
            <div>
              <div className="feature-title">Inference metrics</div>
              <p className="feature-desc">TTFT, TPS, goodput, fairness, and cost vectors for every benchmark.</p>
            </div>
          </div>
        </div>
      </aside>

      <div className="login-beam" aria-hidden="true" />

      <main className="login-panel">
        <div className="login-panel-grid" aria-hidden="true" />
        <div className="login-panel-glow" aria-hidden="true" />
        <div className="panel-inner">
          <div className="mobile-brand">
            <h1 className="panel-title">Zyvor Janus</h1>
            <p className="panel-sub" style={{ marginBottom: 0 }}>
              Mission Control
            </p>
          </div>

          <div className="login-fade-in">
            <h2 className="panel-title">Welcome back</h2>
            <p className="panel-sub">Sign in to Mission Control</p>
          </div>

          <div className="login-glass login-fade-in d1">
            {error ? (
              <div className="login-error show" role="alert">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#f87171" strokeWidth="2">
                  <circle cx="12" cy="12" r="10" />
                  <line x1="12" y1="8" x2="12" y2="12" />
                  <line x1="12" y1="16" x2="12.01" y2="16" />
                </svg>
                <span>{error}</span>
              </div>
            ) : null}

            <form onSubmit={onSubmit} autoComplete="on">
              <label className="field-label" htmlFor="username">
                Username
              </label>
              <div className="field">
                <svg className="login-field-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                  <circle cx="12" cy="7" r="4" />
                </svg>
                <input
                  id="username"
                  className="login-input"
                  name="username"
                  type="text"
                  autoComplete="username"
                  placeholder="Username"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  required
                  autoFocus
                />
              </div>

              <label className="field-label" htmlFor="password">
                Password
              </label>
              <div className="field" style={{ marginBottom: 0, position: "relative" }}>
                <svg className="login-field-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="3" y="11" width="18" height="11" rx="2" />
                  <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                </svg>
                <input
                  id="password"
                  className="login-input"
                  name="password"
                  type={showPassword ? "text" : "password"}
                  autoComplete="current-password"
                  placeholder="Password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                  style={{ paddingRight: "2.75rem" }}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword((v) => !v)}
                  aria-label={showPassword ? "Hide password" : "Show password"}
                  style={{
                    position: "absolute",
                    right: 10,
                    top: "50%",
                    transform: "translateY(-50%)",
                    background: "none",
                    border: 0,
                    color: "var(--text-muted, #94a3b8)",
                    cursor: "pointer",
                  }}
                >
                  <EyeIcon open={showPassword} />
                </button>
              </div>

              <label className="remember">
                <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
                <span>Remember me on this device</span>
              </label>

              <button className="login-btn-primary" type="submit" disabled={busy || !username || !password}>
                {busy ? "Signing in…" : "Sign in to Mission Control"}
              </button>
            </form>
          </div>

          <p className="panel-hint login-fade-in d2">
            Lab credentials: <code>{DEFAULT_USERNAME}</code> / <code>{DEFAULT_PASSWORD}</code>
          </p>
        </div>
      </main>
    </div>
  );
}
