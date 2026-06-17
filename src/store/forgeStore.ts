import { create } from "zustand";
import type { ForgeClip, ForgeTrack, ForgeProjectData, MediaLibraryItem, GaldrProjectFile } from "../types";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";

// Module-level drag state — bypasses React and HTML5 DnD entirely.
// Source items store payload via onPointerDown. ForgePage reads it
// via pointermove/pointerup with elementFromPoint() for hit detection.
let _dragPayload: { id: string; path: string; duration: number; name: string } | null = null;
let _dragActive = false;
export function beginDrag(payload: typeof _dragPayload) {
  _dragPayload = payload;
  _dragActive = true;
}
/** Returns the payload and clears drag state. */
export function endDrag(): typeof _dragPayload {
  const p = _dragPayload;
  _dragPayload = null;
  _dragActive = false;
  return p;
}
export function isDragActive() { return _dragActive; }

const MAX_UNDO = 50;

function emptyTrack(height: number): ForgeTrack {
  return { clips: [], height, muted: false, locked: false };
}

function createEmptyProject(): ForgeProjectData {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    videoTrack: emptyTrack(60),
    audioTrack: emptyTrack(40),
    markers: [],
    playheadTime: 0,
    zoomLevel: 100,
  };
}

function deepClone<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj));
}

function sortClips(track: ForgeTrack) {
  track.clips.sort((a, b) => a.startTime - b.startTime);
}

interface ForgeState {
  project: ForgeProjectData;
  mediaLibrary: MediaLibraryItem[];
  undoStack: ForgeProjectData[];
  redoStack: ForgeProjectData[];
  isExporting: boolean;
  exportProgress: number;
  snapEnabled: boolean;
  dragPayload: { id: string; path: string; duration: number; name: string } | null;

  pushUndo: () => void;
  undo: () => void;
  redo: () => void;

  addClipToVideo: (clip: ForgeClip) => void;
  addClipToAudio: (clip: ForgeClip) => void;
  moveClip: (clipId: string, newStartTime: number, track: "video" | "audio") => void;
  trimClip: (clipId: string, sourceStart: number, sourceEnd: number, track: "video" | "audio") => void;
  splitClipAtPlayhead: () => void;
  deleteClip: (clipId: string, track: "video" | "audio") => void;
  rippleDeleteClip: (clipId: string, track: "video" | "audio") => void;
  selectClip: (clipId: string | null, track: "video" | "audio") => void;
  updateClip: (clipId: string, changes: Partial<ForgeClip>, track: "video" | "audio") => void;

  setPlayhead: (time: number) => void;
  setZoom: (level: number) => void;
  addMarker: (time: number, label?: string) => void;
  removeMarker: (time: number) => void;

  addToLibrary: (item: MediaLibraryItem) => void;
  removeFromLibrary: (id: string) => void;

  importMediaFiles: () => Promise<void>;
  saveProject: () => Promise<void>;
  loadProject: () => Promise<void>;
  resetProject: () => void;

  setExporting: (v: boolean) => void;
  setExportProgress: (v: number) => void;
  setSnapEnabled: (v: boolean) => void;
  setDragPayload: (payload: { id: string; path: string; duration: number; name: string } | null) => void;
}

export const useForgeStore = create<ForgeState>((set, get) => ({
  project: createEmptyProject(),
  mediaLibrary: [],
  undoStack: [],
  redoStack: [],
  isExporting: false,
  exportProgress: 0,
  snapEnabled: true,
  dragPayload: null,

  pushUndo: () => {
    const { project, undoStack } = get();
    const stack = [...undoStack, deepClone(project)];
    if (stack.length > MAX_UNDO) stack.shift();
    set({ undoStack: stack, redoStack: [] });
  },

  undo: () => {
    const { project, undoStack, redoStack } = get();
    if (undoStack.length === 0) return;
    const prev = undoStack[undoStack.length - 1];
    set({
      project: deepClone(prev),
      undoStack: undoStack.slice(0, -1),
      redoStack: [...redoStack, deepClone(project)],
    });
  },

  redo: () => {
    const { project, undoStack, redoStack } = get();
    if (redoStack.length === 0) return;
    const next = redoStack[redoStack.length - 1];
    set({
      project: deepClone(next),
      undoStack: [...undoStack, deepClone(project)],
      redoStack: redoStack.slice(0, -1),
    });
  },

  addClipToVideo: (clip) => {
    get().pushUndo();
    const track = get().project.videoTrack;
    track.clips.push(clip);
    sortClips(track);
    set({ project: { ...get().project } });
  },

  addClipToAudio: (clip) => {
    get().pushUndo();
    const track = get().project.audioTrack;
    track.clips.push(clip);
    sortClips(track);
    set({ project: { ...get().project } });
  },

  moveClip: (clipId, newStartTime, trackKey) => {
    get().pushUndo();
    const track = get().project[trackKey === "video" ? "videoTrack" : "audioTrack"];
    const clip = track.clips.find((c) => c.id === clipId);
    if (!clip) return;
    clip.startTime = Math.max(0, newStartTime);
    track.clips = track.clips.filter((c) => c.id !== clipId);
    track.clips.push(clip);
    sortClips(track);
    set({ project: { ...get().project } });
  },

  trimClip: (clipId, sourceStart, sourceEnd, trackKey) => {
    get().pushUndo();
    const track = get().project[trackKey === "video" ? "videoTrack" : "audioTrack"];
    const clip = track.clips.find((c) => c.id === clipId);
    if (!clip) return;
    clip.sourceStart = Math.max(0, sourceStart);
    clip.sourceEnd = sourceEnd > clip.sourceStart ? sourceEnd : clip.sourceStart + 0.1;
    clip.duration = (clip.sourceEnd - clip.sourceStart) / clip.speed;
    set({ project: { ...get().project } });
  },

  splitClipAtPlayhead: () => {
    get().pushUndo();
    const { project } = get();
    const ph = project.playheadTime;
    let modified = false;

    for (const trackKey of ["videoTrack", "audioTrack"] as const) {
      const track = project[trackKey];
      const clip = track.clips.find(
        (c) => ph >= c.startTime && ph < c.startTime + c.duration
      );
      if (!clip) continue;

      const splitOffset = ph - clip.startTime;
      const sourceOffset = clip.sourceStart + splitOffset * clip.speed;
      if (sourceOffset <= clip.sourceStart || sourceOffset >= clip.sourceEnd) continue;

      const rightClip: ForgeClip = deepClone({
        ...clip,
        id: crypto.randomUUID(),
        startTime: ph,
        duration: clip.duration - splitOffset,
        sourceStart: sourceOffset,
        selected: false,
      });
      clip.duration = splitOffset;
      clip.sourceEnd = sourceOffset;

      track.clips.push(rightClip);
      sortClips(track);
      modified = true;
    }

    if (modified) set({ project: { ...project } });
  },

  deleteClip: (clipId, trackKey) => {
    get().pushUndo();
    const track = get().project[trackKey === "video" ? "videoTrack" : "audioTrack"];
    track.clips = track.clips.filter((c) => c.id !== clipId);
    set({ project: { ...get().project } });
  },

  rippleDeleteClip: (clipId, trackKey) => {
    get().pushUndo();
    const project = get().project;
    const track = project[trackKey === "video" ? "videoTrack" : "audioTrack"];
    const idx = track.clips.findIndex((c) => c.id === clipId);
    if (idx === -1) return;
    const removed = track.clips[idx];
    const gap = removed.duration;
    track.clips = track.clips.filter((c) => c.id !== clipId);
    for (let i = idx; i < track.clips.length; i++) {
      track.clips[i].startTime -= gap;
    }
    set({ project: { ...project } });
  },

  selectClip: (clipId, trackKey) => {
    const project = get().project;
    for (const key of ["videoTrack", "audioTrack"] as const) {
      const matchesTrack = (key === "videoTrack" && trackKey === "video") ||
        (key === "audioTrack" && trackKey === "audio");
      for (const c of project[key].clips) {
        c.selected = matchesTrack && c.id === clipId;
      }
    }
    set({ project: { ...project } });
  },

  updateClip: (clipId, changes, trackKey) => {
    get().pushUndo();
    const track = get().project[trackKey === "video" ? "videoTrack" : "audioTrack"];
    const clip = track.clips.find((c) => c.id === clipId);
    if (!clip) return;
    Object.assign(clip, changes);
    if (changes.speed !== undefined) {
      clip.duration = (clip.sourceEnd - clip.sourceStart) / clip.speed;
    }
    set({ project: { ...get().project } });
  },

  setPlayhead: (time) => set((s) => ({ project: { ...s.project, playheadTime: Math.max(0, time) } })),
  setZoom: (level) => set((s) => ({ project: { ...s.project, zoomLevel: Math.max(20, Math.min(500, level)) } })),

  addMarker: (time, label) => {
    const markers = [...get().project.markers, { time, label: label || "" }];
    markers.sort((a, b) => a.time - b.time);
    set((s) => ({ project: { ...s.project, markers } }));
  },

  removeMarker: (time) => {
    set((s) => ({
      project: {
        ...s.project,
        markers: s.project.markers.filter((m) => m.time !== time),
      },
    }));
  },

  addToLibrary: (item) => {
    set((s) => {
      if (s.mediaLibrary.some((x) => x.path === item.path)) return s;
      return { mediaLibrary: [...s.mediaLibrary, item] };
    });
  },

  removeFromLibrary: (id) => {
    set((s) => ({ mediaLibrary: s.mediaLibrary.filter((x) => x.id !== id) }));
  },

  importMediaFiles: async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          { name: "Media", extensions: ["mp4", "mkv", "avi", "mov", "webm", "m4v", "flv", "ogv", "wmv", "mp3", "flac", "wav", "aac", "ogg", "opus", "png", "jpeg", "jpg", "webp", "gif"] },
        ],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      for (const path of paths) {
        try {
          const info = await invoke<{ duration: number; width?: number; height?: number }>("get_media_info", { path });
          const name = path.split(/[/\\]/).pop() || path;
          get().addToLibrary({
            id: crypto.randomUUID(),
            name,
            path,
            duration: (info as any).duration || 0,
            width: (info as any).width,
            height: (info as any).height,
          });
        } catch {
          const name = path.split(/[/\\]/).pop() || path;
          get().addToLibrary({
            id: crypto.randomUUID(),
            name,
            path,
            duration: 0,
          });
        }
      }
    } catch {}
  },

  saveProject: async () => {
    try {
      const { project, mediaLibrary } = get();
      const now = new Date().toISOString();
      const file: GaldrProjectFile = {
        version: "1.0",
        type: "galdr-project",
        app: "forge",
        name: "untitled",
        created: now,
        updated: now,
        data: deepClone(project),
        extensions: { mediaLibrary: deepClone(mediaLibrary) },
      };
      const dest = await save({
        filters: [{ name: "Galdr Project", extensions: ["galdr"] }],
        defaultPath: "untitled.galdr",
      });
      if (!dest) return;
      const content = JSON.stringify(file, null, 2);
      await invoke("save_project_file", { path: dest, content });
    } catch {}
  },

  loadProject: async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Galdr Project", extensions: ["galdr"] }],
      });
      if (!selected) return;
      const raw = await invoke<string>("load_project_file", { path: selected as string });
      const file: GaldrProjectFile = JSON.parse(raw);
      if (file.type !== "galdr-project") return;
      set({
        project: deepClone(file.data),
        mediaLibrary: deepClone((file.extensions?.mediaLibrary as MediaLibraryItem[]) || []),
        undoStack: [],
        redoStack: [],
      });
    } catch {}
  },

  resetProject: () => {
    set({
      project: createEmptyProject(),
      mediaLibrary: [],
      undoStack: [],
      redoStack: [],
      isExporting: false,
      exportProgress: 0,
    });
  },

  setExporting: (v) => set({ isExporting: v }),
  setExportProgress: (v) => set({ exportProgress: v }),
  setSnapEnabled: (v) => set({ snapEnabled: v }),
  setDragPayload: (payload) => set({ dragPayload: payload }),
}));
