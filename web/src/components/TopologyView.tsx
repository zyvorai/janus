// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"use client";

import { useEffect, useMemo } from "react";
import ReactFlow, {
  Background,
  Controls,
  type Edge,
  type Node,
  ReactFlowProvider,
  useReactFlow,
} from "reactflow";
import "reactflow/dist/style.css";
import type { ClusterSnapshot } from "@/types/simulation";
import { gpuStateColors, theme } from "@/lib/theme";
import { Card } from "./ui";

const FIT_PADDING = 0.35;

function buildGraph(snapshot: ClusterSnapshot): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  let x = 0;

  snapshot.nodes.forEach((node, nodeIdx) => {
    node.gpus.forEach((gpu, gpuIdx) => {
      const id = gpu.id;
      nodes.push({
        id,
        position: { x: x + gpuIdx * 110, y: nodeIdx * 90 },
        data: { label: `${gpu.id}${gpu.busy ? `\n${gpu.job_name ?? "busy"}` : "\nidle"}` },
        style: {
          background: gpu.busy ? gpuStateColors.busy : gpuStateColors.idle,
          color: theme.textBody,
          border: `1px solid ${theme.border}`,
          fontSize: 11,
          whiteSpace: "pre",
          width: 88,
          padding: 6,
        },
      });
      if (gpuIdx > 0) {
        edges.push({
          id: `${node.gpus[gpuIdx - 1].id}-${id}`,
          source: node.gpus[gpuIdx - 1].id,
          target: id,
          label: gpu.nvlink_group != null ? "NVLink" : "PCIe",
          style: { stroke: theme.textMuted },
          labelStyle: { fill: theme.textMuted, fontSize: 10 },
        });
      }
    });
    x += node.gpus.length * 110 + 32;
  });

  return { nodes, edges };
}

function FitViewOnChange({ signature }: { signature: string }) {
  const { fitView } = useReactFlow();

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      fitView({ padding: FIT_PADDING, duration: 150, maxZoom: 1 });
    });
    return () => cancelAnimationFrame(frame);
  }, [signature, fitView]);

  return null;
}

function TopologyFlow({ snapshot }: { snapshot: ClusterSnapshot }) {
  const { nodes, edges } = useMemo(() => buildGraph(snapshot), [snapshot]);
  const signature = `${snapshot.clock}-${nodes.map((n) => n.id).join(",")}`;

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      fitView
      fitViewOptions={{ padding: FIT_PADDING, maxZoom: 1 }}
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable={false}
      panOnScroll
      zoomOnScroll={false}
      proOptions={{ hideAttribution: true }}
      onInit={(instance) => instance.fitView({ padding: FIT_PADDING, maxZoom: 1 })}
    >
      <FitViewOnChange signature={signature} />
      <Background color={theme.border} gap={16} />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}

export function TopologyView({ snapshot }: { snapshot: ClusterSnapshot | null }) {
  if (!snapshot?.nodes?.length) {
    return <Card title="GPU Topology">No topology data.</Card>;
  }

  const gpuCount = snapshot.nodes.reduce((sum, node) => sum + node.gpus.length, 0);
  const minHeight = Math.max(280, snapshot.nodes.length * 100 + 80);

  return (
    <Card title="GPU Topology" description={`${gpuCount} GPUs across ${snapshot.nodes.length} node(s)`}>
      <div
        className="topology-flow rounded-hs border border-hs-border"
        style={{ backgroundColor: theme.surfaceCode, minHeight }}
      >
        <ReactFlowProvider>
          <TopologyFlow snapshot={snapshot} />
        </ReactFlowProvider>
      </div>
    </Card>
  );
}
