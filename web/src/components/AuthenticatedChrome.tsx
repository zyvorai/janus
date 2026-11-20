// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import clsx from "clsx";
import { HeaderAuth } from "@/components/HeaderAuth";

const NAV = [
  { href: "/", label: "Dashboard", match: (p: string) => p === "/" },
  { href: "/simulate", label: "Simulate", match: (p: string) => p.startsWith("/simulate") },
  { href: "/benchmark", label: "Benchmark", match: (p: string) => p.startsWith("/benchmark") },
  { href: "/what-if", label: "What-if", match: (p: string) => p.startsWith("/what-if") },
  { href: "/twins", label: "Twins", match: (p: string) => p.startsWith("/twins") },
] as const;

export function AuthenticatedChrome({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const isAuthPage = pathname.startsWith("/login");

  if (isAuthPage) {
    return <>{children}</>;
  }

  return (
    <>
      <main className="fs-main">
        <div className="topbar">
          <Link href="/" className="topbar-brand">
            <span className="topbar-dot" aria-hidden="true" />
            Zyvor Janus · Mission Control
          </Link>
          <nav className="topbar-nav" aria-label="Primary">
            {NAV.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className={clsx("run-btn", item.match(pathname) && "active")}
              >
                {item.label}
              </Link>
            ))}
          </nav>
          <div className="topbar-spacer" />
          <HeaderAuth />
        </div>
        {children}
      </main>
    </>
  );
}
