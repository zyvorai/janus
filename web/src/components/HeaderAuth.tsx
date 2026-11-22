// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

export function HeaderAuth() {
  const router = useRouter();
  const [username, setUsername] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/auth/me", { credentials: "include" })
      .then((res) => res.json())
      .then((data: { enabled?: boolean; authenticated?: boolean }) => {
        setUsername(data.authenticated ? "Admin" : null);
      })
      .catch(() => setUsername(null));
  }, []);

  async function logout() {
    await fetch("/api/auth/logout", { method: "POST", credentials: "include" });
    router.replace("/login");
    router.refresh();
  }

  if (!username) return null;

  return (
    <div className="topbar-session">
      <span>
        <b>{username}</b>
      </span>
      <button type="button" className="run-btn" onClick={logout}>
        Sign out
      </button>
    </div>
  );
}
