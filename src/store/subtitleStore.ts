import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DownloadProgress,
  DownloadStatus,
  TranscribeParams,
  TranscribeResult,
  WhisperModel,
  WhisperStatus,
} from "../types";

interface SubtitleState {
  /** Catalog of available whisper models, each annotated with `installed`. */
  models: WhisperModel[];
  /** `false` until the first `load()` completes. */
  loaded: boolean;
  /** `true` if the bundled `whisper-cli` binary can be invoked. */
  whisperAvailable: boolean;
  /** Resolved binary path (for diagnostics when availability fails). */
  whisperResolvedPath: string;

  /** Download progress keyed by model id (0.0–1.0). */
  downloads: Record<string, DownloadProgress>;
  /** Download status keyed by model id — tracks failed state for retry UI. */
  downloadStatuses: Record<string, DownloadStatus>;

  /** Active transcription run, or `null` when idle. */
  transcribing: boolean;
  transcriptionProgress: number;
  transcriptionLog: string[];
  lastResult: TranscribeResult | null;
  error: string | null;
  /** Queue job id for the active transcription — passed to cancel_transcription. */
  activeJobId: string | null;

  load: () => Promise<void>;
  refreshModels: () => Promise<void>;
  installModel: (id: string) => Promise<void>;
  cancelDownload: (id: string) => Promise<void>;
  deleteModel: (id: string) => Promise<void>;
  importCustomModel: (srcPath: string) => Promise<void>;
  pickModelFile: () => Promise<string | null>;
  transcribe: (params: TranscribeParams) => Promise<TranscribeResult | null>;
  cancelTranscription: () => Promise<void>;
  clearError: () => void;
  resetLog: () => void;
  /** Subscribe to whisper events. Idempotent. */
  bindEvents: () => Promise<() => void>;
}

export const useSubtitleStore = create<SubtitleState>((set) => ({
  models: [],
  loaded: false,
  whisperAvailable: false,
  whisperResolvedPath: "",
  downloads: {},
  downloadStatuses: {},
  transcribing: false,
  transcriptionProgress: 0,
  transcriptionLog: [],
  lastResult: null,
  error: null,
  activeJobId: null,

  load: async () => {
    try {
      const status = await invoke<WhisperStatus>("whisper_status");
      set({
        models: status.models,
        whisperAvailable: status.available,
        whisperResolvedPath: status.resolvedPath,
        loaded: true,
      });
    } catch {
      set({ loaded: true });
    }
  },

  refreshModels: async () => {
    try {
      const models = await invoke<WhisperModel[]>("list_whisper_models");
      set({ models });
    } catch {
      // leave catalog as-is
    }
  },

  installModel: async (id) => {
    try {
      set((s) => ({
        downloads: { ...s.downloads, [id]: { modelId: id, progress: 0, downloadedBytes: 0, totalBytes: 0 } },
        downloadStatuses: {
          ...s.downloadStatuses,
          [id]: { modelId: id, state: "downloading", progress: 0, downloadedBytes: 0, totalBytes: 0 },
        },
      }));
      const updated = await invoke<WhisperModel>("install_whisper_model", { modelId: id });
      set((s) => ({
        models: s.models.map((m) => (m.id === updated.id ? updated : m)),
        downloads: Object.fromEntries(Object.entries(s.downloads).filter(([k]) => k !== id)),
        downloadStatuses: Object.fromEntries(
          Object.entries(s.downloadStatuses).filter(([k]) => k !== id),
        ),
      }));
    } catch (e) {
      const msg = String(e);
      set((s) => ({
        error: msg,
        downloads: Object.fromEntries(Object.entries(s.downloads).filter(([k]) => k !== id)),
        // Keep the download entry in downloads so the UI can show the error,
        // but mark the status as failed for retry.
        downloadStatuses: {
          ...s.downloadStatuses,
          [id]: {
            modelId: id,
            state: "failed",
            error: msg,
            progress: s.downloads[id]?.progress ?? 0,
            downloadedBytes: s.downloads[id]?.downloadedBytes ?? 0,
            totalBytes: s.downloadStatuses[id]?.totalBytes ?? 0,
          },
        },
      }));
    }
  },

  cancelDownload: async (id) => {
    try {
      await invoke("cancel_whisper_download", { modelId: id });
    } catch {
      // ignore — the install command will surface the cancellation
    }
    set((s) => ({
      downloads: Object.fromEntries(Object.entries(s.downloads).filter(([k]) => k !== id)),
      downloadStatuses: Object.fromEntries(
        Object.entries(s.downloadStatuses).filter(([k]) => k !== id),
      ),
    }));
  },

  deleteModel: async (id) => {
    try {
      await invoke("delete_whisper_model", { modelId: id });
      set((s) => ({
        models: s.models.map((m) => (m.id === id ? { ...m, installed: false } : m)),
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  importCustomModel: async (srcPath) => {
    try {
      const model = await invoke<WhisperModel>("import_custom_model", { srcPath });
      set((s) => ({
        models: [...s.models.filter((m) => m.id !== model.id), model],
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  pickModelFile: async () => {
    try {
      const path = await invoke<string | null>("pick_model_file");
      return path;
    } catch {
      return null;
    }
  },

  transcribe: async (params) => {
    set({
      transcribing: true,
      transcriptionProgress: 0,
      transcriptionLog: ["> transcribe start"],
      error: null,
      lastResult: null,
      activeJobId: null,
    });
    try {
      const result = await invoke<TranscribeResult>("transcribe_audio", { ...params });
      set({ activeJobId: result.jobId });
      set((s) => ({
        transcribing: false,
        transcriptionProgress: 1,
        transcriptionLog: [...s.transcriptionLog, "> done"],
        lastResult: result,
      }));
      return result;
    } catch (e) {
      const msg = typeof e === "string" ? e : "transcription failed";
      set((s) => ({
        transcribing: false,
        error: msg,
        transcriptionLog: [...s.transcriptionLog, `! ${msg}`],
        activeJobId: null,
      }));
      return null;
    }
  },

  cancelTranscription: async () => {
    const jobId = useSubtitleStore.getState().activeJobId;
    try {
      await invoke("cancel_transcription", { jobId: jobId ?? "" });
    } catch {
      // ignore — the run will surface an error event
    }
    set({ activeJobId: null });
  },

  clearError: () => set({ error: null }),
  resetLog: () => set({ transcriptionLog: [] }),

  bindEvents: async () => {
    const unlisteners: UnlistenFn[] = [];

    unlisteners.push(
      await listen<{ progress: number }>("transcribe-progress", (e) => {
        set({ transcriptionProgress: e.payload.progress });
      }),
    );

    unlisteners.push(
      await listen<{ message: string }>("transcribe-log", (e) => {
        set((s) => ({ transcriptionLog: [...s.transcriptionLog, e.payload.message] }));
      }),
    );

    unlisteners.push(
      await listen<DownloadProgress>("whisper-download-progress", (e) => {
        set((s) => ({
          downloads: { ...s.downloads, [e.payload.modelId]: e.payload },
          // Also update the status entry so retry UI can show progress.
          downloadStatuses: {
            ...s.downloadStatuses,
            [e.payload.modelId]: {
              modelId: e.payload.modelId,
              state: "downloading",
              progress: e.payload.progress,
              downloadedBytes: e.payload.downloadedBytes,
              totalBytes: e.payload.totalBytes,
            },
          },
        }));
      }),
    );

    return () => unlisteners.forEach((u) => u());
  },
}));

let bound = false;

/**
 * Bind the global whisper event listeners once for the lifetime of the app.
 * Returns a no-op if already bound (safe to call from multiple mounts).
 */
export async function bindSubtitleEvents() {
  if (bound) return;
  bound = true;
  await useSubtitleStore.getState().bindEvents();
}
