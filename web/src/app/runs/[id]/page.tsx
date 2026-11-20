// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { requireAuth } from "@/lib/require-auth";
import { RunDetailView } from "./RunDetailView";

export default async function RunPage({ params }: { params: { id: string } }) {
  await requireAuth(`/runs/${params.id}`);
  return <RunDetailView runId={params.id} />;
}
