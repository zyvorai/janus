// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import clsx from "clsx";
import Image from "next/image";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { PRIMARY_NAV } from "@/components/GlassIconNav";
import { IconButton, Tooltip } from "@/components/ui";

export function Sidebar({
  collapsed,
  onToggle,
}: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  const pathname = usePathname();

  return (
    <aside className={clsx("app-sidebar", collapsed && "app-sidebar-collapsed")}>
      <Link href="/" className="app-sidebar-brand brand-mark">
        <div className="brand-logo-wrap">
          <Image
            src="/zyvor-logo.png"
            alt="Zyvor"
            width={120}
            height={40}
            className="h-6 w-auto"
            priority
          />
        </div>
        {!collapsed ? (
          <div>
            <div className="brand-copy-title">Zyvor Janus</div>
            <div className="brand-copy-sub">Console</div>
          </div>
        ) : null}
      </Link>

      <nav className="app-sidebar-nav" aria-label="Primary">
        {PRIMARY_NAV.map((item) => {
          const active = item.match(pathname);
          const Icon = item.icon;
          const href = item.href;
          const link = (
            <Link
              href={href}
              className={clsx(
                "mac-glass-icon mac-glass-icon-lg sidebar-glass-icon",
                active && "mac-glass-icon-active",
                !collapsed && "sidebar-glass-icon-expand"
              )}
              aria-current={active ? "page" : undefined}
              aria-label={item.label}
            >
              <Icon size={18} strokeWidth={1.75} />
              {!collapsed ? <span className="sidebar-item-label">{item.label}</span> : null}
            </Link>
          );
          return (
            <div key={`${item.label}-${item.href}`}>
              {collapsed ? <Tooltip label={item.label}>{link}</Tooltip> : link}
            </div>
          );
        })}
      </nav>

      <div className="app-sidebar-footer">
        <Tooltip label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
          <IconButton label={collapsed ? "Expand sidebar" : "Collapse sidebar"} onClick={onToggle}>
            {collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          </IconButton>
        </Tooltip>
      </div>
    </aside>
  );
}

export function BottomNav() {
  const pathname = usePathname();

  return (
    <nav className="bottom-nav" aria-label="Mobile primary">
      {PRIMARY_NAV.map((item) => {
        const active = item.match(pathname);
        const Icon = item.icon;
        return (
          <Link
            key={`${item.label}-${item.href}`}
            href={item.href}
            className={clsx("bottom-nav-item", active && "bottom-nav-item-active")}
            aria-current={active ? "page" : undefined}
          >
            <span className={clsx("mac-glass-icon", active && "mac-glass-icon-active")}>
              <Icon size={16} strokeWidth={1.75} />
            </span>
            <span>{item.label}</span>
          </Link>
        );
      })}
    </nav>
  );
}

function AmbientLayer() {
  return (
    <div className="app-ambient" aria-hidden="true">
      <div className="app-ambient-mesh" />
      <div className="app-ambient-orb app-ambient-orb-a" />
      <div className="app-ambient-orb app-ambient-orb-b" />
    </div>
  );
}

export function AppShell({
  children,
  header,
}: {
  children: React.ReactNode;
  header?: React.ReactNode;
}) {
  const pathname = usePathname();
  const [collapsed, setCollapsed] = useState(true);
  const isAuthPage = pathname.startsWith("/login");

  useEffect(() => {
    try {
      const stored = localStorage.getItem("zyvor-janus-sidebar-collapsed");
      if (stored === "0") setCollapsed(false);
      else setCollapsed(true);
    } catch {
      /* ignore */
    }
  }, []);

  function toggle() {
    setCollapsed((c) => {
      const next = !c;
      try {
        localStorage.setItem("zyvor-janus-sidebar-collapsed", next ? "1" : "0");
      } catch {
        /* ignore */
      }
      return next;
    });
  }

  if (isAuthPage) {
    return <>{children}</>;
  }

  return (
    <div className="app-body">
      <AmbientLayer />
      <Sidebar collapsed={collapsed} onToggle={toggle} />
      <div className="app-content">
        {header}
        {children}
      </div>
      <BottomNav />
    </div>
  );
}
