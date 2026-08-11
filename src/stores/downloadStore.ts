import { create } from "zustand";
import type { DownloadTask, ProgressEvent } from "../types/download";
import { listDownloads } from "../lib/tauri";

interface DownloadState {
  tasks: DownloadTask[];
  loading: boolean;
  setTasks: (tasks: DownloadTask[]) => void;
  upsertTask: (task: DownloadTask) => void;
  applyProgress: (event: ProgressEvent) => void;
  appendLog: (id: string, line: string) => void;
  refresh: () => Promise<void>;
  removeFinishedLocally: () => void;
}

export const useDownloadStore = create<DownloadState>((set, get) => ({
  tasks: [],
  loading: false,
  setTasks: (tasks) => set({ tasks }),
  upsertTask: (task) =>
    set((state) => {
      const idx = state.tasks.findIndex((t) => t.id === task.id);
      if (idx === -1) {
        return { tasks: [task, ...state.tasks] };
      }
      const next = [...state.tasks];
      next[idx] = { ...next[idx], ...task };
      return { tasks: next };
    }),
  applyProgress: (event) =>
    set((state) => ({
      tasks: state.tasks.map((task) =>
        task.id === event.id
          ? {
              ...task,
              progress: event.percent,
              speed: event.speed ?? task.speed,
              eta: event.eta ?? task.eta,
              status: task.status === "queued" ? "running" : task.status,
            }
          : task,
      ),
    })),
  appendLog: (id, line) =>
    set((state) => ({
      tasks: state.tasks.map((task) =>
        task.id === id
          ? {
              ...task,
              logTail: [...(task.logTail ?? []).slice(-79), line],
            }
          : task,
      ),
    })),
  refresh: async () => {
    set({ loading: true });
    try {
      const tasks = await listDownloads();
      set({ tasks });
    } finally {
      set({ loading: false });
    }
  },
  removeFinishedLocally: () => {
    const { tasks } = get();
    set({
      tasks: tasks.filter(
        (t) => !["completed", "failed", "cancelled"].includes(t.status),
      ),
    });
  },
}));
