// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { create } from "zustand";
import type { ClusterSnapshot, SchedulerDecision } from "@/types/simulation";

interface ReplayState {
  snapshots: ClusterSnapshot[];
  decisions: SchedulerDecision[];
  index: number;
  playing: boolean;
  speed: number;
  setSnapshots: (s: ClusterSnapshot[]) => void;
  setDecisions: (d: SchedulerDecision[]) => void;
  setIndex: (i: number) => void;
  setPlaying: (p: boolean) => void;
  setSpeed: (s: number) => void;
  reset: () => void;
  next: () => void;
  prev: () => void;
}

export const useReplayStore = create<ReplayState>((set, get) => ({
  snapshots: [],
  decisions: [],
  index: 0,
  playing: false,
  speed: 1,
  setSnapshots: (snapshots) => set({ snapshots, index: 0, playing: false }),
  setDecisions: (decisions) => set({ decisions }),
  setIndex: (index) => {
    const max = Math.max(get().snapshots.length - 1, get().decisions.length - 1, 0);
    set({ index: Math.max(0, Math.min(index, max)) });
  },
  setPlaying: (playing) => set({ playing }),
  setSpeed: (speed) => set({ speed }),
  reset: () => set({ snapshots: [], decisions: [], index: 0, playing: false, speed: 1 }),
  next: () => get().setIndex(get().index + 1),
  prev: () => get().setIndex(get().index - 1),
}));
