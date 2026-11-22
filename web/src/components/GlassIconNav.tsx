// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import clsx from "clsx";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  BarChart3,
  Database,
  FlaskConical,
  LayoutDashboard,
  PlayCircle,
  type LucideIcon,
} from "lucide-react";
import { Tooltip } from "@/components/ui";

export type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
  match: (pathname: string) => boolean;
};

export const PRIMARY_NAV: NavItem[] = [
  { href: "/", label: "Dashboard", icon: LayoutDashboard, match: (p) => p === "/" },
  {
    href: "/#runs",
    label: "Runs",
    icon: PlayCircle,
    match: (p) => p.startsWith("/runs"),
  },
  {
    href: "/benchmark",
    label: "Benchmark",
    icon: BarChart3,
    match: (p) => p.startsWith("/benchmark"),
  },
  {
    href: "/what-if",
    label: "What-if",
    icon: FlaskConical,
    match: (p) => p.startsWith("/what-if"),
  },
  {
    href: "/twins",
    label: "Twins",
    icon: Database,
    match: (p) => p.startsWith("/twins"),
  },
];

export function GlassIconNav({
  className,
  size = "md",
}: {
  className?: string;
  size?: "md" | "lg";
}) {
  const pathname = usePathname();

  return (
    <nav className={clsx("mac-glass-nav", className)} aria-label="Primary">
      {PRIMARY_NAV.map((item) => {
        const active = item.match(pathname);
        const Icon = item.icon;
        const link = (
          <Link
            href={item.href}
            className={clsx(
              "mac-glass-icon",
              size === "lg" && "mac-glass-icon-lg",
              active && "mac-glass-icon-active"
            )}
            aria-current={active ? "page" : undefined}
            aria-label={item.label}
          >
            <Icon size={size === "lg" ? 18 : 16} strokeWidth={1.75} />
          </Link>
        );
        return (
          <Tooltip key={`${item.label}-${item.href}`} label={item.label}>
            {link}
          </Tooltip>
        );
      })}
    </nav>
  );
}
