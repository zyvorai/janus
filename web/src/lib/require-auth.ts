// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { COOKIE_NAME, verifySessionToken } from "@/lib/auth";

/** Server-side guard for dashboard pages (backup when middleware is bypassed). */
export async function requireAuth(nextPath = "/"): Promise<void> {
  const token = cookies().get(COOKIE_NAME)?.value;
  if (await verifySessionToken(token)) {
    return;
  }
  redirect(`/login?next=${encodeURIComponent(nextPath)}`);
}
