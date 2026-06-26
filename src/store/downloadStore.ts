import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  UrlMetadata,
  DownloadOptions,
  UrlDownloadProgress,
  DownloadedFile,
  YtdlpStatus,
  DownloadQuality,
} from "../types";

/** Progress of the yt-dlp binary install (not a media download). */
interface InstallProgress {
  progress: number;
  downloadedBytes: number;
  totalBytes: number;
}

interface DownloadState {
  /** yt-dlp binary status. */
  available: boolean;
  resolvedPath: string;
  downloadsDir: string;

  /** yt-dlp binary being downloaded. */
  installing: boolean;
  installProgress: InstallProgress | null;

  /** Metadata fetched from the URL. */
  metadata: UrlMetadata | null;
  fetching: boolean;

  /** Active download job. */
  downloading: boolean;
  downloadProgress: UrlDownloadProgress | null;
  downloadLog: string[];
  activeJobId: string | null;
  lastDownloadedPath: string | null;

  /** Recent downloads from the downloads folder. */
  recentDownloads: DownloadedFile[];
  loadingDownloads: boolean;

  error: string | null;

  // ── Actions ──
  init: () => Promise<void>;
  installYtdlp: () => Promise<void>;
  fetchMetadata: (url: string) => Promise<void>;
  startDownload: (options: DownloadOptions) => Promise<void>;
  cancelDownload: () => Promise<void>;
  loadRecentDownloads: () => Promise<void>;
  deleteDownload: (path: string) => Promise<void>;
  clearError: () => void;
  resetLog: () => void;
  /** Subscribe to download events. Idempotent. */
  bindEvents: () => Promise<() => void>;
}

export const useDownloadStore = create<DownloadState>((set) => ({
  available: false,
  resolvedPath: "",
  downloadsDir: "",

  installing: false,
  installProgress: null,

  metadata: null,
  fetching: false,

  downloading: false,
  downloadProgress: null,
  downloadLog: [],
  activeJobId: null,
  lastDownloadedPath: null,

  recentDownloads: [],
  loadingDownloads: false,

  error: null,

  init: async () => {
    try {
      const status = await invoke<YtdlpStatus>("ytdlp_status", {});
      set({
        available: status.available,
        resolvedPath: status.resolvedPath,
        downloadsDir: status.downloadsDir,
      });
    } catch {
      // leave defaults
    }
  },

  installYtdlp: async () => {
    set({ installing: true, installProgress: null, error: null });
    try {
      await invoke<string>("ensure_ytdlp", {});
      // Re-check status after install.
      const status = await invoke<YtdlpStatus>("ytdlp_status", {});
      set({
        installing: false,
        available: status.available,
        resolvedPath: status.resolvedPath,
      });
    } catch (e) {
      const msg = typeof e === "string" ? e : "failed to install yt-dlp";
      set({ installing: false, error: msg });
    }
  },

  fetchMetadata: async (url) => {
    set({ fetching: true, error: null, metadata: null });
    try {
      const meta = await invoke<UrlMetadata>("fetch_metadata", { url });
      set({ fetching: false, metadata: meta });
    } catch (e) {
      const msg = typeof e === "string" ? e : "failed to fetch metadata";
      set({ fetching: false, error: msg });
    }
  },

  startDownload: async (options) => {
    set({
      downloading: true,
      downloadProgress: null,
      downloadLog: ["> download start"],
      activeJobId: null,
      lastDownloadedPath: null,
      error: null,
    });
    try {
      const jobId = await invoke<string>("start_download", { options });
      set((s) => ({
        activeJobId: jobId,
        downloadLog: [...s.downloadLog, `> job ${jobId}`],
      }));
    } catch (e) {
      const msg = typeof e === "string" ? e : "download failed to start";
      set((s) => ({
        downloading: false,
        error: msg,
        downloadLog: [...s.downloadLog, `! ${msg}`],
        activeJobId: null,
      }));
    }
  },

  cancelDownload: async () => {
    const jobId = useDownloadStore.getState().activeJobId;
    try {
      await invoke("cancel_download", { jobId: jobId ?? "" });
    } catch {
      // the run will surface an error event
    }
    set({ activeJobId: null });
  },

  loadRecentDownloads: async () => {
    set({ loadingDownloads: true });
    try {
      const downloads = await invoke<DownloadedFile[]>("list_downloads", {});
      set({ recentDownloads: downloads, loadingDownloads: false });
    } catch {
      set({ loadingDownloads: false });
    }
  },

  deleteDownload: async (path) => {
    try {
      await invoke("delete_download", { path });
      set((s) => ({
        recentDownloads: s.recentDownloads.filter((f) => f.path !== path),
      }));
    } catch (e) {
      const msg = typeof e === "string" ? e : "failed to delete file";
      set({ error: msg });
    }
  },

  clearError: () => set({ error: null }),
  resetLog: () => set({ downloadLog: [] }),

  bindEvents: async () => {
    const unlisteners: UnlistenFn[] = [];

    // Install progress (yt-dlp binary download).
    unlisteners.push(
      await listen<InstallProgress>("ytdlp-install-progress", (e) => {
        set({ installProgress: e.payload });
      }),
    );

    // Download progress.
    unlisteners.push(
      await listen<UrlDownloadProgress>("download-progress", (e) => {
        set({ downloadProgress: e.payload });
      }),
    );

    // Download log lines.
    unlisteners.push(
      await listen<{ jobId: string; message: string }>("download-log", (e) => {
        set((s) => ({
          downloadLog: [...s.downloadLog, e.payload.message],
        }));
      }),
    );

    // Download complete — refresh the recent downloads list.
    unlisteners.push(
      await listen<{ jobId: string; outputPath?: string }>("download-complete", (e) => {
        set((s) => ({
          downloading: false,
          lastDownloadedPath: e.payload.outputPath ?? null,
          activeJobId: null,
          downloadLog: [...s.downloadLog, "> download complete"],
        }));
        useDownloadStore.getState().loadRecentDownloads();
      }),
    );

    return () => unlisteners.forEach((u) => u());
  },
}));

let bound = false;

/**
 * Bind the global download event listeners once for the lifetime of the app.
 * Returns a no-op if already bound (safe to call from multiple mounts).
 */
export async function bindDownloadEvents() {
  if (bound) return;
  bound = true;
  await useDownloadStore.getState().bindEvents();
}

// ── Convenience helpers ──

/** Human-readable quality options for the UI dropdown. */
export const QUALITY_OPTIONS: { value: DownloadQuality; label: string }[] = [
  { value: "best", label: "Best video + audio" },
  { value: "1080p", label: "1080p" },
  { value: "720p", label: "720p" },
  { value: "480p", label: "480p" },
  { value: "audio_only", label: "Audio only" },
];

/** Audio format options for extraction. */
export const AUDIO_FORMAT_OPTIONS = [
  { value: "mp3", label: "MP3" },
  { value: "opus", label: "Opus" },
  { value: "flac", label: "FLAC" },
  { value: "wav", label: "WAV" },
];

/** Output container format options for video downloads. */
export const OUTPUT_FORMAT_OPTIONS = [
  { value: "", label: "auto" },
  { value: "mp4", label: "MP4" },
  { value: "webm", label: "WebM" },
  { value: "mkv", label: "MKV" },
  { value: "mov", label: "MOV" },
];
