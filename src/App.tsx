import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, ProgressBarStatus, UserAttentionType } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { isEnabled as isAutostartEnabled } from "@tauri-apps/plugin-autostart";
import HomePage from "./pages/HomePage";
import ConvertPage from "./pages/ConvertPage";

import CompressPage from "./pages/CompressPage";
import SettingsPage from "./pages/SettingsPage";
import RunesPage from "./pages/RunesPage";
import ForgePage from "./pages/ForgePage";
import WatchFoldersPage from "./pages/WatchFoldersPage";
import SubtitlesPage from "./pages/SubtitlesPage";
import ImportPage from "./pages/ImportPage";
import HistoryPage from "./pages/HistoryPage";
import StatsPage from "./pages/StatsPage";
import ScrambleText from "./components/ScrambleText";
import UpdateBanner from "./components/UpdateBanner";
import HelpOverlay from "./components/HelpOverlay";
import CommandPalette from "./components/CommandPalette";
import QueueDropdown from "./components/QueueDropdown";
import NotificationCenter from "./components/NotificationCenter";
import DropZone from "./components/DropZone";
import Toaster from "./components/Toaster";
import OnboardingTour from "./components/OnboardingTour";
import PageTransition from "./transitions";
import { useGaldrStore } from "./store";
import { useForgeStore } from "./store/forgeStore";
import { bindQueueEvents, useQueueStore, selectOverallProgress } from "./store/queueStore";
import { bindDownloadEvents } from "./store/downloadStore";
import { useToastStore } from "./store/toastStore";
import { useHistoryStore } from "./store/historyStore";
import { ContextMenuProvider, useContextMenu } from "./components/ContextMenu";
import type { GaldrProjectFile, JobStatus } from "./types";
import "./App.css";

interface AppSettings {
  outputDir: string;
  transitionStyle: string;
  showRuneInTitlebar: boolean;
  discordEnabled: boolean;
  preferredVideoEncoder: string | null;
  autoFallbackHw: boolean;
  downloadDir: string;
  autoDownloadSubtitles: boolean;
  autoEmbedSubtitles: boolean;
  theme: string;
  crtEnabled: boolean;
  onboardingSeen: boolean;
}

const PERSIST_FIELDS: (keyof AppSettings)[] = [
  "outputDir", "transitionStyle", "showRuneInTitlebar", "discordEnabled",
  "preferredVideoEncoder", "autoFallbackHw", "downloadDir", "autoDownloadSubtitles",
  "autoEmbedSubtitles", "theme", "crtEnabled", "onboardingSeen",
];

type Page = "home" | "convert" | "compress" | "settings" | "runes" | "forge" | "watch" | "subtitles" | "import" | "history" | "stats";

function AppShell() {
  const [page, setPage] = useState<Page>("home");
  const [prevPage, setPrevPage] = useState<Page>("home");
  const [appVersion, setAppVersion] = useState("");
  const [helpOverlayOpen, setHelpOverlayOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const transitionStyle = useGaldrStore((s) => s.transitionStyle);
  const taskbarAction = useGaldrStore((s) => s.taskbarAction);
  const taskbarProgress = useGaldrStore((s) => s.taskbarProgress);
  const taskbarFlash = useGaldrStore((s) => s.taskbarFlash);
  const setTaskbarFlash = useGaldrStore((s) => s.setTaskbarFlash);
  const showRuneInTitlebar = useGaldrStore((s) => s.showRuneInTitlebar);
  const theme = useGaldrStore((s) => s.theme);
  const crtEnabled = useGaldrStore((s) => s.crtEnabled);
  const onboardingSeen = useGaldrStore((s) => s.onboardingSeen);
  const setOnboardingSeen = useGaldrStore((s) => s.setOnboardingSeen);
  const queueJobs = useQueueStore((s) => s.jobs);
  const queueProgress = selectOverallProgress(queueJobs);
  const win = getCurrentWindow();
  const prevFlash = useRef(false);
  const { show } = useContextMenu();

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion("0.1.0"));
  }, []);

  // Routes an externally-opened .galdr file: inspect its content and open the
  // right page. The `type` field discriminates forge projects from rune
  // collections — both share the .galdr extension.
  const handleOpenFile = useCallback(async (path: string) => {
    try {
      const raw = await invoke<string>("load_project_file", { path });
      const file = JSON.parse(raw) as GaldrProjectFile;
      if (file.type === "galdr-project") {
        if (file.app === "forge") {
          setPage("forge");
          await useForgeStore.getState().loadProjectFromPath(path, { fromExternal: true });
        }
        return;
      }
      if (file.type === "galdr-runes" && Array.isArray((file as any).runes)) {
        const sourceName = path.split(/[/\\]/).pop() || path;
        useGaldrStore.getState().setPendingRunesImport({
          runes: (file as any).runes,
          sourceName,
        });
        setPage("runes");
      }
    } catch {
      // unreadable / invalid file — ignore silently
    }
  }, []);

  // Centralised open-file routing. Three sources feed handleOpenFile:
  //  1. first-launch CLI arg (Windows/Linux double-click) via consume_pending_file
  //  2. macOS file-association event
  //  3. single-instance forwarding (second launch hands args to first window)
  useEffect(() => {
    invoke<string | null>("consume_pending_file").then((path) => {
      if (path) handleOpenFile(path);
    }).catch(() => {});

    const unlisteners: Array<() => void> = [];
    (async () => {
      const u1 = await listen<string>("tauri://open-file", (e) => handleOpenFile(e.payload));
      const u2 = await listen<string>("galdr://open-file", (e) => handleOpenFile(e.payload));
      unlisteners.push(u1, u2);
    })();
    return () => unlisteners.forEach((u) => u());
  }, [handleOpenFile]);

  // Load persisted settings on mount
  useEffect(() => {
    const store = useGaldrStore.getState();
    invoke<AppSettings>("load_settings").then((s) => {
      store.setOutputDir(s.outputDir);
      store.setTransitionStyle(s.transitionStyle as any);
      store.setShowRuneInTitlebar(s.showRuneInTitlebar);
      store.setDiscordEnabled(s.discordEnabled);
      if (s.preferredVideoEncoder != null) {
        store.setPreferredVideoEncoder(s.preferredVideoEncoder);
      }
      store.setAutoFallbackHw(s.autoFallbackHw);
      store.setDownloadDir(s.downloadDir);
      store.setAutoDownloadSubtitles(s.autoDownloadSubtitles);
      store.setAutoEmbedSubtitles(s.autoEmbedSubtitles);
      if (s.theme) store.setTheme(s.theme);
      store.setCrtEnabled(s.crtEnabled);
      store.setOnboardingSeen(s.onboardingSeen);
    }).catch(() => {
      useToastStore.getState().push({
        kind: "warn",
        title: "couldn't load saved settings",
        message: "using defaults — your preferences may not have persisted last session",
      });
    });
  }, []);

  // Apply the active theme + CRT overlay to the document root whenever they change.
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);
  useEffect(() => {
    document.documentElement.setAttribute("data-crt", crtEnabled ? "on" : "off");
  }, [crtEnabled]);

  // Hydrate rune tags once so every page can offer preset pick / save-as-rune.
  useEffect(() => {
    useGaldrStore.getState().loadRuneTags();
  }, []);

  // Bind the background queue event listener on mount.
  useEffect(() => {
    bindQueueEvents();
  }, []);

  // Toast on background job completion/failure. The queue only surfaces
  // status through the QueueDropdown (which the user may not have open), so
  // we diff the jobs array here and fire a toast whenever a job transitions
  // out of running into a terminal state. Covers convert/compress/concat/
  // extract/transcription/subtitle/forge/batch/watch conversions in one place.
  const jobs = useQueueStore((s) => s.jobs);
  const prevStatusRef = useRef<Record<string, JobStatus>>({});
  useEffect(() => {
    const prev = prevStatusRef.current;
    const next: Record<string, JobStatus> = {};
    const toast = useToastStore.getState().push;

    for (const job of jobs) {
      next[job.id] = job.status;
      const before = prev[job.id];
      // Only toast on a transition FROM running INTO a terminal state —
      // this avoids firing on the initial snapshot load (where jobs may
      // already be completed) and on re-renders.
      if (before === "running" && job.status === "completed") {
        toast({
          kind: "success",
          title: job.label,
          action: job.outputPath
            ? { label: "reveal", onClick: () => invoke("reveal_in_folder", { path: job.outputPath }).catch(() => {}) }
            : undefined,
        });
      } else if (before === "running" && job.status === "failed") {
        toast({
          kind: "error",
          title: `${job.label} failed`,
          message: job.error,
        });
      }

      // Record completed/failed jobs into the persistent history (for re-run + stats).
      if (before === "running" && (job.status === "completed" || job.status === "failed")) {
        useHistoryStore.getState().add({
          id: job.id,
          op: job.jobType,
          label: job.label,
          inputPath: job.inputPath,
          outputPath: job.outputPath,
          status: job.status === "completed" ? "completed" : "failed",
          createdAt: job.completedAt ?? new Date().toISOString(),
        });
      }
    }
    prevStatusRef.current = next;
  }, [jobs]);

  // Bind the download event listener on mount.
  useEffect(() => {
    bindDownloadEvents();
  }, []);

  // Read OS autostart state on mount (autostart is OS-managed, not in settings.json)
  useEffect(() => {
    isAutostartEnabled()
      .then((enabled) => useGaldrStore.getState().setAutostartEnabled(enabled))
      .catch(() => {});
  }, []);

  // Detect hardware encoders on startup
  useEffect(() => {
    useGaldrStore.getState().loadHardwareEncoders();
  }, []);

  // Check for forge recovery on mount
  useEffect(() => {
    invoke<string | null>("load_forge_recovery").then((raw) => {
      if (!raw) return;
      try {
        const recovery = JSON.parse(raw);
        const forgeStore = useForgeStore.getState();
        forgeStore.restoreFromRecovery(recovery.project, recovery.mediaLibrary, recovery.filePath);
      } catch {}
    }).catch(() => {});
  }, []);

  // Auto-save settings when they change (debounced)
  useEffect(() => {
    const unsub = useGaldrStore.subscribe((state, prev) => {
      const changed = PERSIST_FIELDS.some((f) => (state as any)[f] !== (prev as any)[f]);
      if (!changed) return;
      clearTimeout((window as any)._settingsSaveTimer);
      (window as any)._settingsSaveTimer = setTimeout(() => {
        const s = useGaldrStore.getState();
        invoke("save_app_preferences", {
          outputDir: s.outputDir,
          transitionStyle: s.transitionStyle,
          showRuneInTitlebar: s.showRuneInTitlebar,
          discordEnabled: s.discordEnabled,
          preferredVideoEncoder: s.preferredVideoEncoder === "software" ? null : s.preferredVideoEncoder,
          autoFallbackHw: s.autoFallbackHw,
          downloadDir: s.downloadDir,
          autoDownloadSubtitles: s.autoDownloadSubtitles,
          autoEmbedSubtitles: s.autoEmbedSubtitles,
          theme: s.theme,
          crtEnabled: s.crtEnabled,
          onboardingSeen: s.onboardingSeen,
        }).catch(() => {
          useToastStore.getState().push({
            kind: "error",
            title: "couldn't save settings",
            message: "your changes may not have persisted to disk",
          });
        });
      }, 300);
    });
    return () => {
      unsub();
      clearTimeout((window as any)._settingsSaveTimer);
    };
  }, []);

  // Auto-save forge recovery debounced on any forge store change
  useEffect(() => {
    const unsub = useForgeStore.subscribe(() => {
      clearTimeout((window as any)._forgeRecoveryTimer);
      (window as any)._forgeRecoveryTimer = setTimeout(() => {
        const f = useForgeStore.getState();
        if (!f.isModified) return;
        const data = JSON.stringify({
          project: f.project,
          mediaLibrary: f.mediaLibrary,
          filePath: f.currentFilePath,
        });
        invoke("save_forge_recovery", { data }).catch(() => {});
      }, 2000);
    });
    return () => {
      unsub();
      clearTimeout((window as any)._forgeRecoveryTimer);
    };
  }, []);

  useEffect(() => {
    if (page === "forge") {
      const forgeState = useForgeStore.getState();
      const vclips = forgeState.project.videoTrack.clips.length;
      const aclips = forgeState.project.audioTrack.clips.length;
      const totalDur = [...forgeState.project.videoTrack.clips, ...forgeState.project.audioTrack.clips]
        .reduce((s, c) => s + c.duration, 0);
      invoke("update_discord_presence", { page, forgeClips: vclips + aclips, forgeDuration: totalDur }).catch((e) => console.error("discord rpc:", e));
    } else {
      invoke("update_discord_presence", { page, forgeClips: null, forgeDuration: null }).catch((e) => console.error("discord rpc:", e));
    }
  }, [page]);

  useEffect(() => {
    const title = taskbarAction ? `GALDR - ${taskbarAction}` : "GALDR";
    win.setTitle(title).catch(() => {});
  }, [taskbarAction, win]);

  useEffect(() => {
    if (taskbarProgress === null) {
      win.setProgressBar({ progress: 0, status: ProgressBarStatus.None }).catch(() => {});
    } else {
      win.setProgressBar({ progress: Math.round(taskbarProgress * 100), status: ProgressBarStatus.Normal }).catch(() => {});
    }
  }, [taskbarProgress, win]);

  useEffect(() => {
    if (taskbarFlash && !prevFlash.current) {
      prevFlash.current = true;
      win.requestUserAttention(UserAttentionType.Critical).catch(() => {});
      setTimeout(() => {
        prevFlash.current = false;
        setTaskbarFlash(false);
      }, 100);
    }
  }, [taskbarFlash, win, setTaskbarFlash]);

  const handleSettings = () => {
    if (page === "settings") {
      setPage(prevPage);
    } else {
      setPrevPage(page);
      setPage("settings");
    }
  };

  const handleRunes = () => {
    if (page === "runes") {
      setPage(prevPage);
    } else {
      setPrevPage(page);
      setPage("runes");
    }
  };

  // ── Global keyboard shortcuts ──
  // Single window-level listener dispatches navigation, palette, and help.
  // Input-focus guard prevents firing while typing in inputs/textareas.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      // Ctrl+K — command palette
      if (e.ctrlKey && e.key === "k") {
        e.preventDefault();
        setCommandPaletteOpen((o) => !o);
        return;
      }

      // Ctrl+, — settings
      if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        handleSettings();
        return;
      }

      // ? — help overlay
      if (e.key === "?" && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        setHelpOverlayOpen((o) => !o);
        return;
      }

      // Ctrl+1..Ctrl+7 — page navigation
      if (e.ctrlKey && /^[1-7]$/.test(e.key) && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        const targets: Page[] = ["home", "convert", "compress", "forge", "subtitles", "watch", "runes"];
        const idx = parseInt(e.key, 10) - 1;
        if (idx < targets.length) setPage(targets[idx]);
        return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleSettings]);

  const rootSegs: { label: string; target: Page }[] = [
    { label: "~", target: "home" },
    { label: "galdr", target: "home" },
  ];

  const pageSegs: { label: string; target: Page }[] = (() => {
    if (page === "convert") {
      return [
        { label: "convert", target: "convert" },
      ];
    }
    if (page === "compress") {
      return [
        { label: "compress", target: "compress" },
      ];
    }
    if (page === "runes") {
      return [
        { label: "runes", target: "runes" },
      ];
    }
    if (page === "forge") {
      return [
        { label: "forge", target: "forge" },
      ];
    }
    if (page === "watch") {
      return [
        { label: "watch", target: "watch" },
      ];
    }
    if (page === "subtitles") {
      return [
        { label: "subtitles", target: "subtitles" },
      ];
    }
    return [{ label: page, target: page }];
  })();

  const pathSegs = [...rootSegs, ...pageSegs];

  const handleGlobalContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    show(e, [
      { label: "quick convert", rune: "ᛏ", action: () => setPage("convert") },
      { label: "compress", rune: "ᛉ", action: () => setPage("compress") },
      { label: "forge editor", rune: "ᚲ", action: () => setPage("forge") },
      { label: "subtitles", rune: "ᛊ", action: () => setPage("subtitles") },
      { label: "", rune: "", action: () => {}, divider: true },
      { label: "rune tags", rune: "ᚠ", action: () => setPage("runes") },
      { label: "watch folders", rune: "ᚱ", action: () => setPage("watch") },
      { label: "settings", rune: "ᚲ", action: () => setPage("settings") },
    ]);
  }, [show, setPage]);

  const handlePathNavContext = useCallback((e: React.MouseEvent, target: Page, label: string) => {
    e.stopPropagation();
    show(e, [
      { label: `navigate to ${label}`, rune: "ᛏ", action: () => setPage(target) },
    ]);
  }, [show, setPage]);

  const handleVersionContext = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    show(e, [
      { label: `copy (v${appVersion})`, rune: "ᚷ", action: () => navigator.clipboard.writeText(appVersion) },
      { label: "check for updates", rune: "ᚠ", action: () => useGaldrStore.getState().setUpdateDismissed(false) },
    ]);
  }, [show, appVersion]);

  return (
    <div className="app-shell" onContextMenu={handleGlobalContextMenu}>
      <DropZone
        onFiles={(paths) => {
          // Route dropped files to the convert page. A full smart-routing
          // implementation would sniff extension/folder, but convert is the
          // safest default landing zone for arbitrary media files.
          setPage("convert");
          useToastStore.getState().push({
            kind: "info",
            title: `${paths.length} file${paths.length === 1 ? "" : "s"} dropped`,
            message: "routed to convert — pick a format and cast",
          });
        }}
        onUrl={(url) => {
          setPage("import");
          useToastStore.getState().push({
            kind: "info",
            title: "url dropped",
            message: url.length > 60 ? url.slice(0, 60) + "…" : url,
          });
        }}
      />
      <header className="titlebar" data-tauri-drag-region>
        <div
          className={`titlebar-queue-bar${queueProgress === null ? " idle" : ""}`}
          style={{ width: queueProgress === null ? "0%" : `${Math.round(queueProgress * 100)}%` }}
        />
        <div className="titlebar-left">
          {showRuneInTitlebar && (
            <button className="titlebar-btn titlebar-rune-btn" onClick={handleRunes}>
              <span className="ts-rune">ᚠ</span>
            </button>
          )}
          <QueueDropdown />
          <NotificationCenter />
          <button
            className="titlebar-btn titlebar-help"
            onClick={() => setHelpOverlayOpen(true)}
            title="Keyboard shortcuts (?)"
          >
            ?
          </button>
          <button className="titlebar-btn titlebar-settings" onClick={handleSettings}>
            <span className="ts-rune">ᚲ</span>
            <span className="ts-label">settings</span>
          </button>
        </div>
        <ScrambleText as="span" className="titlebar-logo" text="ᚷ Galdr" hover load />
        <div className="titlebar-controls">
          <button className="titlebar-btn" onClick={() => win.minimize()}>
            _
          </button>
          <button className="titlebar-btn" onClick={() => win.toggleMaximize()}>
            []
          </button>
          <button className="titlebar-btn titlebar-close" onClick={() => win.close()}>
            x
          </button>
        </div>
      </header>

      <nav className="path-nav">
        {pathSegs.map((seg, i) => (
          <span key={i} className="path-group">
            {i > 0 && <span className="path-sep">/</span>}
            <span
              className={`path-seg${i === pathSegs.length - 1 ? " active" : ""}`}
              onClick={() => seg.target !== page && setPage(seg.target)}
              onContextMenu={(e) => handlePathNavContext(e, seg.target, seg.label)}
            >
              {seg.label}
            </span>
          </span>
        ))}
        <span className="path-sep trail">/</span>
        {appVersion && <span className="path-version" onContextMenu={handleVersionContext}>v{appVersion}</span>}
      </nav>

      <main className="main-content">
        <UpdateBanner />
        <Toaster />
        {!onboardingSeen && (
          <OnboardingTour onComplete={() => setOnboardingSeen(true)} />
        )}
        <HelpOverlay open={helpOverlayOpen} onClose={() => setHelpOverlayOpen(false)} />
        <CommandPalette open={commandPaletteOpen} onClose={() => setCommandPaletteOpen(false)} onNavigate={setPage} onShowHelp={() => setHelpOverlayOpen(true)} />
        <PageTransition style={transitionStyle} pageKey={page}>
          {page === "home" && <HomePage onNavigate={setPage} />}
          {page === "convert" && <ConvertPage onNavigate={setPage} />}
          {page === "compress" && <CompressPage onNavigate={setPage} />}
          {page === "settings" && <SettingsPage onNavigate={setPage} />}
          {page === "runes" && <RunesPage />}
          {page === "forge" && <ForgePage />}
          {page === "watch" && <WatchFoldersPage />}
          {page === "subtitles" && <SubtitlesPage />}
          {page === "import" && <ImportPage />}
          {page === "history" && <HistoryPage onNavigate={setPage} />}
          {page === "stats" && <StatsPage />}
        </PageTransition>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <ContextMenuProvider>
      <AppShell />
    </ContextMenuProvider>
  );
}
