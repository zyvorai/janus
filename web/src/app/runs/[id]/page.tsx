// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import { requireAuth } from "@/lib/require-auth";
import { RunDetailView } from "./RunDetailView";

export default async function RunPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  await requireAuth(`/runs/${id}`);
  return <RunDetailView runId={id} />;
}
