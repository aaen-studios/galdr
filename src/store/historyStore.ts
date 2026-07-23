import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { HistoryEntry, UsageStats } from "../types";

interface HistoryState {
  entries: HistoryEntry[];
  loaded: boolean;

  /** Fetch the full history from the backend. */
  load: () => Promise<void>;
  /** Append an entry (called after an op completes). */
  add: (entry: HistoryEntry) => Promise<void>;
  /** Remove a single entry by id. */
  remove: (id: string) => Promise<void>;
  /** Clear all history. */
  clear: () => Promise<void>;
  /** Compute aggregate stats. */
  stats: () => Promise<UsageStats>;
}

export const useHistoryStore = create<HistoryState>((set, get) => ({
  entries: [],
  loaded: false,

  load: async () => {
    try {
      const entries = await invoke<HistoryEntry[]>("list_history");
      set({ entries, loaded: true });
    } catch {
      set({ loaded: true });
    }
  },

  add: async (entry) => {
    try {
      await invoke("add_history_entry", { entry });
      set({ entries: [entry, ...get().entries].slice(0, 200) });
    } catch {
      // best-effort — history is non-critical
    }
  },

  remove: async (id) => {
    try {
      await invoke("delete_history_entry", { id });
      set({ entries: get().entries.filter((e) => e.id !== id) });
    } catch {
      // best-effort
    }
  },

  clear: async () => {
    try {
      await invoke("clear_history");
      set({ entries: [] });
    } catch {
      // best-effort
    }
  },

  stats: async () => {
    return await invoke<UsageStats>("get_usage_stats");
  },
}));

/** Convenience helper: generate a history entry id. */
export function historyId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

/** Format a relative time string from an ISO timestamp. */
export function relativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  const diff = Date.now() - then;
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 7) return `${day}d ago`;
  return new Date(iso).toLocaleDateString();
}

/** Format a byte count as a human-readable size. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
