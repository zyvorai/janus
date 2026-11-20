// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { motion } from "framer-motion";
import type { JobsTimeline } from "@/types/simulation";
import { fadeInUp, staggerContainer } from "@/lib/motion";
import { Card } from "./ui";

export function QueueTable({ timeline }: { timeline: JobsTimeline | null }) {
  const jobs = timeline?.jobs ?? [];
  return (
    <Card
      title="Queue / Jobs (final summary)"
      description="Final job states for the completed run. Does not update during scheduler replay."
    >
      <div className="data-table-wrap">
        <table className="data-table">
          <thead>
            <tr>
              <th>Job</th>
              <th>Priority</th>
              <th>Tenant</th>
              <th>GPUs</th>
              <th>Status</th>
            </tr>
          </thead>
          <motion.tbody variants={staggerContainer} initial="hidden" animate="visible">
            {jobs.length === 0 ? (
              <tr>
                <td colSpan={5} className="text-center text-hs-muted">
                  No jobs recorded.
                </td>
              </tr>
            ) : (
              jobs.map((job) => (
                <motion.tr key={job.job_id} variants={fadeInUp}>
                  <td className="font-medium text-hs-heading">{job.name}</td>
                  <td className="font-mono">{job.priority}</td>
                  <td className="text-hs-muted">{job.tenant ?? "—"}</td>
                  <td className="font-mono">{job.gpu_count}</td>
                  <td className="capitalize text-hs-body">{job.state}</td>
                </motion.tr>
              ))
            )}
          </motion.tbody>
        </table>
      </div>
    </Card>
  );
}
