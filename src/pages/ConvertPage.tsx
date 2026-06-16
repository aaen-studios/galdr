import { useState, useEffect, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useGaldrStore } from "../store";
import CustomSelect from "../components/CustomSelect";
import ScrambleText from "../components/ScrambleText";

const IMAGE_CODECS = ["png", "jpeg", "gif", "bmp", "tiff", "webp"];

const FORMAT_OPTIONS = [
  { value: "mp4", label: "mp4 (video)", type: "video" as const },
  { value: "mkv", label: "mkv (video)", type: "video" as const },
  { value: "avi", label: "avi (video)", type: "video" as const },
  { value: "mov", label: "mov (video)", type: "video" as const },
  { value: "webm", label: "webm (video)", type: "video" as const },
  { value: "m4v", label: "m4v (video)", type: "video" as const },
  { value: "flv", label: "flv (video)", type: "video" as const },
  { value: "ogv", label: "ogv (video)", type: "video" as const },
  { value: "wmv", label: "wmv (video)", type: "video" as const },
  { value: "gif", label: "gif (video)", type: "video" as const },
  { value: "mod", label: "mod (video)", type: "video" as const },
  { value: "mp3", label: "mp3 (audio)", type: "audio" as const },
  { value: "flac", label: "flac (audio)", type: "audio" as const },
  { value: "wav", label: "wav (audio)", type: "audio" as const },
  { value: "aac", label: "aac (audio)", type: "audio" as const },
  { value: "ogg", label: "ogg (audio)", type: "audio" as const },
  { value: "opus", label: "opus (audio)", type: "audio" as const },
  { value: "wma", label: "wma (audio)", type: "audio" as const },
  { value: "m4a", label: "m4a (audio)", type: "audio" as const },
  { value: "aiff", label: "aiff (audio)", type: "audio" as const },
  { value: "ac3", label: "ac3 (audio)", type: "audio" as const },
  { value: "png", label: "png (image)", type: "image" as const },
  { value: "jpeg", label: "jpeg (image)", type: "image" as const },
  { value: "webp", label: "webp (image)", type: "image" as const },
  { value: "bmp", label: "bmp (image)", type: "image" as const },
  { value: "tiff", label: "tiff (image)", type: "image" as const },
  { value: "avif", label: "avif (image)", type: "image" as const },
];

type MediaType = "video" | "audio" | "image" | null;

function detectMediaType(mi: import("../types").MediaInfo): MediaType {
  const isImage = mi.container.includes("image") ||
    (mi.duration === 0 && mi.streams.some((s) => IMAGE_CODECS.includes(s.codec)));
  if (isImage) return "image";
  if (mi.streams.some((s) => s.kind === "audio") &&
    !mi.streams.some((s) => s.kind === "video")) return "audio";
  return "video";
}

function fmtDur(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (h) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

export default function ConvertPage() {
  const {
    mediaInfo, conversionParams, isConverting,
    conversionProgress, lastOutputPath, error, ffmpegFound, outputDir,
    setMediaInfo, setConversionParams,
    setIsConverting, setConversionProgress,
    setLastOutputPath, setError, setFfmpegFound, setOutputDir,
  } = useGaldrStore();

  const [log, setLog] = useState<string[]>([]);
  const [isDragOver, setIsDragOver] = useState(false);
  const [filteredOptions, setFilteredOptions] = useState(FORMAT_OPTIONS);
  const [mediaType, setMediaType] = useState<MediaType>(null);
  const [btnHover, setBtnHover] = useState(false);

  useEffect(() => {
    invoke<boolean>("detect_ffmpeg").then(setFfmpegFound);
  }, [setFfmpegFound]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    (async () => {
      unlisten = await listen<{ job_id: string; progress: number }>(
        "conversion-progress",
        (e) => {
          setConversionProgress(e.payload.progress);
          setLog((p) => {
            const pct = `${Math.round(e.payload.progress * 100)}%`;
            return p[p.length - 1] !== pct ? [...p, pct] : p;
          });
        },
      );
    })();
    return () => { if (unlisten) unlisten(); };
  }, [setConversionProgress]);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    (async () => {
      unlisteners.push(
        await listen("tauri://drag-enter", () => setIsDragOver(true)),
      );
      unlisteners.push(
        await listen("tauri://drag-leave", () => setIsDragOver(false)),
      );
      unlisteners.push(
        await listen<{ paths: string[] }>("tauri://drag-drop", (e) => {
          setIsDragOver(false);
          const path = e.payload.paths?.[0];
          if (path) loadFile(path);
        }),
      );
    })();
    return () => { unlisteners.forEach((u) => u()); };
  }, []);

  useEffect(() => {
    if (!mediaInfo) {
      setFilteredOptions(FORMAT_OPTIONS);
      setMediaType(null);
      return;
    }
    const mt = detectMediaType(mediaInfo);
    setMediaType(mt);
    let allowed: Set<string>;
    if (mt === "image") {
      allowed = new Set(["image"]);
    } else if (mt === "audio") {
      allowed = new Set(["audio"]);
    } else {
      allowed = new Set(["video", "audio", "image"]);
    }
    const filtered = FORMAT_OPTIONS.filter((o) => allowed.has(o.type));
    setFilteredOptions(filtered);
    const cur = conversionParams.output_format;
    if (!filtered.some((o) => o.value === cur)) {
      const ext = conversionParams.input_path
        ?.split("/").pop()?.split(".").pop()?.toLowerCase();
      let match: (typeof FORMAT_OPTIONS)[number] | undefined;
      if (ext) match = filtered.find((o) => o.value === ext);
      setConversionParams({ output_format: match?.value ?? filtered[0]?.value ?? "mp4" });
    }
  }, [mediaInfo]);

  const loadFile = useCallback(async (path: string) => {
    setError(null);
    setLog([]);
    setLastOutputPath(null);
    try {
      const info = await invoke<import("../types").MediaInfo>("get_media_info", { path });
      setMediaInfo(info);
      setConversionParams({ input_path: path });
    } catch (e) {
      setMediaInfo(null);
      setError(String(e));
    }
  }, [setConversionParams, setMediaInfo, setError, setLastOutputPath]);

  const pickFile = useCallback(async () => {
    const sel = await open({
      multiple: false,
      filters: [{ name: "Media", extensions: [
        "mp4","mkv","avi","mov","webm","m4v","flv","ogv","wmv","ts","3gp","mod",
        "mp3","flac","wav","aac","ogg","opus","wma","m4a","aiff","ac3","dts",
        "png","jpg","jpeg","webp","gif","bmp","tiff","avif","svg",
      ] }],
    });
    if (sel) loadFile(sel as string);
  }, [loadFile]);

  const convert = useCallback(async () => {
    if (!conversionParams.input_path) return;

    let dir = outputDir;
    if (!dir) {
      const defaultDir = await invoke<string>("get_default_output_dir");
      const picked = await open({
        directory: true,
        multiple: false,
        defaultPath: defaultDir,
      });
      if (!picked) return;
      dir = picked as string;
      setOutputDir(dir);
    }

    const typeDir = mediaType ? `${dir}/${mediaType}` : dir;
    const params = { ...conversionParams, output_dir: typeDir };

    setIsConverting(true);
    setError(null);
    setLog(["> start"]);
    setLastOutputPath(null);
    setConversionProgress(0);
    try {
      const r = await invoke<{ job_id: string; output_path: string }>(
        "start_conversion", { params },
      );
      setLog((p) => [...p, `> ${r.output_path}`]);
      setLastOutputPath(r.output_path);
    } catch (e) {
      const m = typeof e === "string" ? e : "failed";
      setLog((p) => [...p, `! ${m}`]);
      setError(m);
    } finally {
      setIsConverting(false);
    }
  }, [conversionParams, outputDir, mediaType, setOutputDir, setIsConverting, setError, setLastOutputPath, setConversionProgress]);

  return (
    <div className="page">
      {!ffmpegFound && (
        <div className="alert-error">! ffmpeg not found on PATH</div>
      )}

      <div
        className={`drop-zone${isDragOver ? " drag-over" : ""}${conversionParams.input_path ? " has-file" : ""}`}
        onClick={pickFile}
      >
        {conversionParams.input_path ? (
          <span className="drop-file">{conversionParams.input_path}</span>
        ) : (
          <>
            <span className="drop-rune">ᚨ</span>
            <ScrambleText as="span" className="drop-text" text="drop media or click to browse" hover />
          </>
        )}
      </div>

      <AnimatePresence>
        {mediaInfo && (
          <motion.div
            className="media-info"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
          >
          <div className="primary">
            {mediaInfo.container} | {fmtDur(mediaInfo.duration)} | {fmtSize(mediaInfo.size)}
            {mediaInfo.bitrate && ` | ${(mediaInfo.bitrate / 1000).toFixed(0)}kbps`}
            {mediaType && ` | ${mediaType}`}
          </div>
          {mediaInfo.streams.map((s, i) => (
            <div key={i} className="stream">
              [{s.kind}] {s.codec}
              {s.width && ` ${s.width}x${s.height}`}
              {s.frame_rate && ` @ ${s.frame_rate.toFixed(1)}fps`}
              {s.channels && ` ${s.channels}ch`}
              {s.sample_rate && ` ${(s.sample_rate / 1000).toFixed(0)}kHz`}
            </div>
          ))}
          </motion.div>
        )}
      </AnimatePresence>

      <ScrambleText as="div" className="rune-divider" text="ᛟ ᛟ ᛟ ᛟ ᛟ" hover ticks={4} />

      <div className="card">
        <label className="label">output format</label>
        <CustomSelect
          options={filteredOptions}
          value={conversionParams.output_format}
          onChange={(v) => setConversionParams({ output_format: v })}
        />
      </div>

      {mediaType && outputDir && (
        <div className="card">
          <span className="label">output path</span>
          <span className="path-preview">{outputDir}/{mediaType}/</span>
        </div>
      )}

      {error && <div className="alert-error">! {error}</div>}

      <div className="convert-actions">
        <button
          className="btn btn-primary"
          disabled={!conversionParams.input_path || isConverting}
          onClick={convert}
          onMouseEnter={() => setBtnHover(true)}
          onMouseLeave={() => setBtnHover(false)}
        >
          {isConverting ? "converting..." : <ScrambleText text="convert" trigger={btnHover} ticks={4} />}
        </button>
        {isConverting && (
          <button className="btn btn-cancel" onClick={() => invoke("cancel_conversion")} title="cancel">
            ■
          </button>
        )}
      </div>

      {isConverting && (
        <div className="progress-bar-container">
          <div className="progress-bar" style={{ width: `${conversionProgress * 100}%` }} />
          <span className="progress-text">{Math.round(conversionProgress * 100)}%</span>
        </div>
      )}

      {log.length > 0 && (
        <div className="log-panel">
          {log.map((l, i) => <div key={i} className="log-line">{l}</div>)}
        </div>
      )}

      {lastOutputPath && (
        <div className="result-bar">
          <span className="result-path">{lastOutputPath}</span>
          <button className="btn" onClick={() => revealItemInDir(lastOutputPath)}>
            show in folder
          </button>
        </div>
      )}
    </div>
  );
}
