// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import { requireAuth } from "@/lib/require-auth";
import { DashboardHome } from "./DashboardHome";

export default async function HomePage() {
  await requireAuth("/");
  return <DashboardHome />;
}
