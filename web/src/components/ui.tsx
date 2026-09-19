// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: Apache-2.0

"use client";

import clsx from "clsx";
import Link from "next/link";
import { Children, isValidElement, useEffect, useId, useState } from "react";
import type { ButtonHTMLAttributes, InputHTMLAttributes, SelectHTMLAttributes } from "react";
import { Line, LineChart, ResponsiveContainer } from "recharts";
import { AnimatePresence, animate, motion, useMotionValue, useReducedMotion } from "framer-motion";
import { chartColors } from "@/lib/theme";
import { easeOut, fadeInUp, staggerContainer } from "@/lib/motion";

export function Card({
  title,
  description,
  children,
  className,
  variant = "default",
  accent: _accent,
}: {
  title?: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
  variant?: "default" | "action";
  accent?: "orange" | "teal" | "violet" | "green";
}) {
  // Action-variant cards already get an entrance + hover treatment from the `.act` CSS
  // class (rise keyframe + translateY hover) — animating them again here would fight
  // that CSS via a duplicate inline `transform`. Only default cards get a motion entrance.
  const isAction = variant === "action";
  return (
    <motion.section
      className={clsx("glass card", isAction && "act", className)}
      initial={isAction ? false : { opacity: 0, y: 10 }}
      animate={isAction ? false : { opacity: 1, y: 0 }}
      transition={{ duration: 0.4, ease: easeOut }}
    >
      {title ? (
        <>
          <div className={variant === "action" ? "act-title" : "card-title"}>{title}</div>
          {description ? <p className="card-description">{description}</p> : null}
        </>
      ) : null}
      {children}
    </motion.section>
  );
}

export type HeroStatus = "go" | "degraded" | "idle" | "down" | "offline";

export function PageHero({
  kicker,
  title,
  titleAccent,
  subtitle,
  actions,
  status,
  icon,
  children,
}: {
  kicker?: string;
  title: string;
  titleAccent?: string;
  subtitle?: string;
  actions?: React.ReactNode;
  status?: HeroStatus;
  icon?: React.ReactNode;
  statusLabel?: string;
  children?: React.ReactNode;
}) {
  const dataStatus = status === "idle" ? "offline" : status;
  return (
    <div className="hero" data-status={dataStatus ?? "offline"}>
      <div className="hero-shine" aria-hidden="true" />
      <motion.div
        className="hero-head"
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.45, ease: easeOut, delay: 0.08 }}
      >
        {icon ? <div className="icon-badge">{icon}</div> : null}
        <div>
          {kicker ? <div className="brand-eyebrow">{kicker}</div> : null}
          <h1>
            {titleAccent ? <span>{titleAccent}</span> : null}
            {titleAccent && title ? " " : null}
            {title || null}
          </h1>
          {subtitle ? (
            <div className="subtitle status-line">
              <span className="lamp" aria-hidden="true" />
              <span>{subtitle}</span>
            </div>
          ) : null}
        </div>
        {actions ? <div className="sync">{actions}</div> : null}
      </motion.div>
      {children ? (
        <motion.div variants={staggerContainer} initial="hidden" animate="visible">
          {children}
        </motion.div>
      ) : null}
    </div>
  );
}

export function FormField({
  label,
  children,
  className,
}: {
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <label className={clsx("form-field", className)}>
      <span className="form-label">{label}</span>
      {children}
    </label>
  );
}

export function EmptyState({
  title,
  text,
  children,
}: {
  title: string;
  text: string;
  children?: React.ReactNode;
}) {
  return (
    <motion.div
      className="empty-state"
      initial={{ opacity: 0, scale: 0.97 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.3, ease: easeOut }}
    >
      <p className="empty-state-title">{title}</p>
      <p className="empty-state-text">{text}</p>
      {children}
    </motion.div>
  );
}

const NUMERIC_VALUE_RE = /^(-?[\d,]+(?:\.\d+)?)(.*)$/;

/** Tweens the numeric portion of a metric string (e.g. "42.5%", "12/20") on change. */
function AnimatedValue({ value }: { value: string }) {
  const match = value.match(NUMERIC_VALUE_RE);
  const numeric = match ? parseFloat(match[1].replace(/,/g, "")) : null;
  const suffix = match ? match[2] : "";
  const decimals = match && match[1].includes(".") ? match[1].split(".")[1].length : 0;
  const motionValue = useMotionValue(numeric ?? 0);
  const reducedMotion = useReducedMotion();
  const [display, setDisplay] = useState(value);

  useEffect(() => {
    if (numeric === null) {
      setDisplay(value);
      return;
    }
    if (reducedMotion) {
      motionValue.set(numeric);
      setDisplay(value);
      return;
    }
    const controls = animate(motionValue, numeric, {
      duration: 0.6,
      ease: easeOut,
      onUpdate: (latest) => setDisplay(`${latest.toFixed(decimals)}${suffix}`),
    });
    return () => controls.stop();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [numeric, suffix, decimals, reducedMotion]);

  return <>{display}</>;
}

export function MetricTile({
  label,
  value,
  sparkData,
  sub,
}: {
  label: string;
  value: string;
  sparkData?: number[];
  sub?: string;
}) {
  return (
    <motion.div className="stat-tile" variants={fadeInUp}>
      <span className="stat-label">{label}</span>
      <span className="stat-value">
        <AnimatedValue value={value} />
      </span>
      {sub ? <span className="stat-sub">{sub}</span> : null}
      {sparkData && sparkData.length > 1 ? <Sparkline data={sparkData} /> : null}
    </motion.div>
  );
}

const statusStyles: Record<string, string> = {
  completed: "bg-hs-success/15 text-hs-success-light border border-hs-success/25",
  running: "bg-hs-indigo/15 text-hs-purple-light border border-hs-indigo/25",
  failed: "bg-hs-error/15 text-hs-error-light border border-hs-error/25",
  pending: "bg-hs-surface text-hs-muted border border-hs-border",
};

export function StatusBadge({ status }: { status: string }) {
  const reducedMotion = useReducedMotion();
  const isLive = status === "running" && !reducedMotion;
  return (
    <motion.span
      className={clsx("status-badge", statusStyles[status] ?? statusStyles.pending)}
      animate={
        isLive
          ? { boxShadow: ["0 0 0 0 rgba(99,102,241,0.35)", "0 0 0 6px rgba(99,102,241,0)"] }
          : undefined
      }
      transition={isLive ? { duration: 1.6, repeat: Infinity, ease: "easeOut" } : undefined}
    >
      {status}
    </motion.span>
  );
}

export function Badge({
  children,
  variant = "default",
}: {
  children: React.ReactNode;
  variant?: "default" | "accent" | "success" | "error";
}) {
  return (
    <span
      className={clsx(
        "zyvor-badge",
        variant === "accent" && "zyvor-badge-accent",
        variant === "success" && "zyvor-badge-success",
        variant === "error" && "zyvor-badge-error"
      )}
    >
      {children}
    </span>
  );
}

export function Button({
  variant = "primary",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" }) {
  return (
    <button
      className={clsx("run-btn", variant === "primary" && "primary", className)}
      {...props}
    />
  );
}

export function IconButton({
  className,
  label,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return <button type="button" aria-label={label} className={clsx("icon-btn", className)} {...props} />;
}

export function AppLink({
  href,
  className,
  children,
  showArrow = true,
}: {
  href: string;
  className?: string;
  children: React.ReactNode;
  showArrow?: boolean;
}) {
  return (
    <Link href={href} className={clsx("zyvor-link", className)}>
      {children}
      {showArrow ? <span aria-hidden="true">→</span> : null}
    </Link>
  );
}

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={clsx("inp", className)} {...props} />;
}

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={clsx("inp grow", className)} {...props} />;
}

/* ── Tabs ── */

export function Tabs({
  tabs,
  defaultTab,
  children,
}: {
  tabs: Array<{ id: string; label: string; icon?: React.ReactNode }>;
  defaultTab?: string;
  children: React.ReactNode;
}) {
  const [active, setActive] = useState(defaultTab ?? tabs[0]?.id ?? "");

  // Select the active panel directly (instead of each TabPanel reading shared context)
  // so the outgoing panel still has its own content to fade out during its AnimatePresence
  // exit — context would already report the new `active` id by the time it exits.
  const activeChild = Children.toArray(children).find(
    (child) => isValidElement<{ id?: string }>(child) && child.props.id === active
  );

  return (
    <>
      <div className="tab-bar" role="tablist">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active === tab.id}
            className={clsx("tab-item", active === tab.id && "tab-item-active")}
            onClick={() => setActive(tab.id)}
          >
            {active === tab.id ? (
              <motion.span
                layoutId="tab-active-pill"
                className="tab-item-active-bg"
                transition={{ type: "spring", stiffness: 500, damping: 40 }}
              />
            ) : null}
            <span className="tab-item-content">
              {tab.icon}
              {tab.label}
            </span>
          </button>
        ))}
      </div>
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={active}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
        >
          {activeChild}
        </motion.div>
      </AnimatePresence>
    </>
  );
}

export function TabPanel({ children }: { id: string; children: React.ReactNode }) {
  return (
    <div className="tab-panel" role="tabpanel">
      {children}
    </div>
  );
}

/* ── Skeleton ── */

export function Skeleton({ className, style }: { className?: string; style?: React.CSSProperties }) {
  return <div className={clsx("skeleton", className)} style={style} aria-hidden="true" />;
}

/* ── Tooltip ── */

export function Tooltip({ label, children }: { label: string; children: React.ReactNode }) {
  const id = useId();
  return (
    <span className="tooltip-wrap" aria-describedby={id}>
      {children}
      <span id={id} role="tooltip" className="tooltip-bubble">
        {label}
      </span>
    </span>
  );
}

/* ── Sparkline ── */

export function Sparkline({ data, color = chartColors.bar }: { data: number[]; color?: string }) {
  const chartData = data.map((v, i) => ({ i, v }));
  return (
    <div className="sparkline-wrap">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={chartData}>
          <Line
          type="monotone"
          dataKey="v"
          stroke={color}
          strokeWidth={1.5}
          dot={false}
          isAnimationActive
          animationDuration={500}
          animationEasing="ease-out"
        />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

/* ── ProgressBar ── */

export function ProgressBar({
  value,
  max = 100,
  variant = "default",
}: {
  value: number;
  max?: number;
  variant?: "default" | "success" | "warn";
}) {
  const pct = max <= 0 ? 0 : Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <div className="progress-bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
      <motion.div
        className={clsx(
          "progress-bar-fill",
          variant === "success" && "progress-bar-fill-success",
          variant === "warn" && "progress-bar-fill-warn"
        )}
        initial={false}
        animate={{ width: `${pct}%` }}
        transition={{ duration: 0.5, ease: easeOut }}
      />
    </div>
  );
}

/* ── Scheduler pills ── */

export const SCHEDULERS = ["fifo", "priority", "preemptive", "zynera", "bestfit"] as const;
export type SchedulerId = (typeof SCHEDULERS)[number];

export function SchedulerPillGroup({
  value,
  onChange,
  multi = false,
  selected,
  onMultiChange,
}: {
  value?: string;
  onChange?: (v: string) => void;
  multi?: boolean;
  selected?: string[];
  onMultiChange?: (v: string[]) => void;
}) {
  if (multi) {
    const set = new Set(selected ?? []);
    return (
      <div className="scheduler-pill-group" role="group" aria-label="Schedulers">
        {SCHEDULERS.map((s) => {
          const active = set.has(s);
          return (
            <button
              key={s}
              type="button"
              aria-pressed={active}
              className={clsx("scheduler-pill", active && "scheduler-pill-active")}
              onClick={() => {
                const next = new Set(set);
                if (next.has(s)) next.delete(s);
                else next.add(s);
                onMultiChange?.(Array.from(next));
              }}
            >
              {s}
            </button>
          );
        })}
      </div>
    );
  }

  return (
    <div className="scheduler-pill-group" role="radiogroup" aria-label="Scheduler">
      {SCHEDULERS.map((s) => (
        <button
          key={s}
          type="button"
          role="radio"
          aria-checked={value === s}
          className={clsx("scheduler-pill", value === s && "scheduler-pill-active")}
          onClick={() => onChange?.(s)}
        >
          {s}
        </button>
      ))}
    </div>
  );
}
