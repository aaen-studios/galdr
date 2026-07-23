import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { QueueJob, QueueUpdatePayload } from "../types";

interface QueueState {
  /** All jobs (active + completed), sorted with running first. */
  jobs: QueueJob[];
  /** True until the initial `get_queue` snapshot has been fetched. */
  loaded: boolean;
  /** Frontend-local ordering preference (not yet backend-enforced). */
  paused: boolean;
  /** Desired parallel worker count (frontend hint). */
  parallelWorkers: number;

  /** Refresh the queue from the backend snapshot. */
  refresh: () => Promise<void>;
  /** Cancel a specific job by id. */
  cancelJob: (id: string) => Promise<void>;
  /** Remove all completed/failed/cancelled jobs. */
  clearCompleted: () => Promise<void>;
  /** Reorder a job within the local view (drag-and-drop priority). */
  reorder: (fromIdx: number, toIdx: number) => void;
  /** Pause/resume the queue (frontend hint — stops dispatching new ops). */
  setPaused: (p: boolean) => void;
  /** Set the desired parallel worker count. */
  setParallelWorkers: (n: number) => void;
  /** Subscribe to `queue-update` events. Idempotent. */
  bindEvents: () => Promise<() => void>;
}

let bound = false;

export const useQueueStore = create<QueueState>((set, get) => ({
  jobs: [],
  loaded: false,
  paused: false,
  parallelWorkers: 1,

  refresh: async () => {
    try {
      const jobs = await invoke<QueueJob[]>("get_queue");
      set({ jobs, loaded: true });
    } catch {
      set({ loaded: true });
    }
  },

  cancelJob: async (id) => {
    try {
      await invoke("cancel_job", { id });
    } catch {
      // best-effort
    }
    await get().refresh();
  },

  clearCompleted: async () => {
    try {
      await invoke("clear_completed_jobs");
    } catch {
      // best-effort
    }
    await get().refresh();
  },

  reorder: (fromIdx, toIdx) => {
    const jobs = [...get().jobs];
    if (fromIdx < 0 || fromIdx >= jobs.length || toIdx < 0 || toIdx >= jobs.length) return;
    const [moved] = jobs.splice(fromIdx, 1);
    jobs.splice(toIdx, 0, moved);
    set({ jobs });
    // Note: backend priority isn't changed — this is a frontend view reorder.
    // A future backend `set_job_priority` command would persist this.
  },

  setPaused: (p) => set({ paused: p }),

  setParallelWorkers: (n) => set({ parallelWorkers: Math.max(1, Math.min(4, n)) }),

  bindEvents: async () => {
    const unlisten: UnlistenFn = await listen<QueueUpdatePayload>(
      "queue-update",
      (e) => {
        const jobs = e.payload?.jobs ?? [];
        set({ jobs });
      },
    );

    // Seed with the current backend state.
    await get().refresh();

    return unlisten;
  },
}));

/**
 * Bind the global queue event listener once for the lifetime of the app.
 * Returns a no-op if already bound (safe to call from multiple mounts).
 */
export async function bindQueueEvents() {
  if (bound) return;
  bound = true;
  await useQueueStore.getState().bindEvents();
}

/** Derived selector: jobs that are queued or running. */
export function selectActive(jobs: QueueJob[]): QueueJob[] {
  return jobs.filter((j) => j.status === "queued" || j.status === "running");
}

/** Derived selector: jobs that are completed, failed, or cancelled. */
export function selectFinished(jobs: QueueJob[]): QueueJob[] {
  return jobs.filter(
    (j) => j.status === "completed" || j.status === "failed" || j.status === "cancelled",
  );
}

/** Count of jobs that are queued or running. */
export function selectActiveCount(jobs: QueueJob[]): number {
  return selectActive(jobs).length;
}

/**
 * Overall progress across all active jobs as a 0..1 fraction.
 * Returns `null` when nothing is active (so callers can hide the strip).
 */
export function selectOverallProgress(jobs: QueueJob[]): number | null {
  const active = selectActive(jobs);
  if (active.length === 0) return null;
  const sum = active.reduce((s, j) => s + (j.progress || 0), 0);
  return sum / active.length;
}
