import { useEffect, useState, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useWatchStore } from "../store/watchStore";
import Dropdown from "../components/Dropdown";
import ScrambleText from "../components/ScrambleText";
import { FORMAT_OPTIONS } from "../options";
import type {
  ConversionParams, WatchAction, WatchFolderConfig,
  WatchOutputFormat, ConflictPolicy, WatchLogStatus,
} from "../types";

const ACTION_OPTIONS = [
  { value: "autoConvert", label: "auto-convert now" },
  { value: "queue", label: "queue for review" },
];

const CONFLICT_OPTIONS: { value: ConflictPolicy; label: string }[] = [
  { value: "skip", label: "skip (don't overwrite)" },
  { value: "overwrite", label: "overwrite existing" },
  { value: "rename", label: "auto-rename (_1, _2, …)" },
];

/** Minimal-but-complete ConversionParams preset. input_path/output_dir are
 *  overwritten per-file at convert time; they just need to exist for serde. */
function defaultParams(): ConversionParams {
  return {
    input_path: "",
    output_dir: "",
    output_format: "mp4",
    quality: 0.8,
  };
}

function defaultOutputFormat(): WatchOutputFormat {
  return { outputFormat: "mp4", quality: 0.8 };
}

function emptyFolder(): WatchFolderConfig {
  return {
    id: "",
    enabled: true,
    path: "",
    patterns: [],
    ignoreOlderThanMinutes: 0,
    settleMs: 10000,
    outputDir: "",
    action: "autoConvert",
    outputFormats: [defaultOutputFormat()],
    conflictPolicy: "skip",
    deleteSource: false,
    recursive: false,
    preservePath: false,
    processingLog: [],
    // deprecated
    extensions: [],
    params: defaultParams(),
  };
}

/** Map a WatchLogStatus to a short display label + css class. */
function logStatusBadge(status: WatchLogStatus): { label: string; cls: string } {
  switch (status) {
    case "success": return { label: "✓ done", cls: "log-badge-success" };
    case "skippedConflict": return { label: "⏭ skip", cls: "log-badge-skip" };
    case "skippedAge": return { label: "⏭ old", cls: "log-badge-skip" };
    case "failed": return { label: "✗ fail", cls: "log-badge-error" };
  }
}

export default function WatchFoldersPage() {
  const {
    folders, paused, queue, activity, load, saveFolder, deleteFolder,
    setPaused, convertQueued, dequeue, clearQueue, bindEvents, clearLog,
  } = useWatchStore();

  const [editing, setEditing] = useState<WatchFolderConfig | null>(null);
  const [patternInput, setPatternInput] = useState("");
  const [expandedLog, setExpandedLog] = useState<string | null>(null);

  useEffect(() => {
    load();
    let unbind: (() => void) | undefined;
    bindEvents().then((u) => { unbind = u; });
    return () => { unbind?.(); };
  }, [load, bindEvents]);

  const pickFolder = useCallback(async () => {
    const sel = await open({ directory: true, multiple: false });
    if (sel && editing) setEditing({ ...editing, path: sel as string });
  }, [editing]);

  const pickOutput = useCallback(async () => {
    const sel = await open({ directory: true, multiple: false });
    if (sel && editing) setEditing({ ...editing, outputDir: sel as string });
  }, [editing]);

  const pickFormatOutput = useCallback(async (index: number) => {
    const sel = await open({ directory: true, multiple: false });
    if (sel && editing) {
      const formats = [...editing.outputFormats];
      formats[index] = { ...formats[index], outputDir: sel as string };
      setEditing({ ...editing, outputFormats: formats });
    }
  }, [editing]);

  const addPattern = useCallback(() => {
    if (!editing || !patternInput.trim()) return;
    const pat = patternInput.trim();
    if (!editing.patterns.includes(pat)) {
      setEditing({ ...editing, patterns: [...editing.patterns, pat] });
    }
    setPatternInput("");
  }, [editing, patternInput]);

  const addFormat = useCallback(() => {
    if (!editing) return;
    setEditing({
      ...editing,
      outputFormats: [...editing.outputFormats, defaultOutputFormat()],
    });
  }, [editing]);

  const removeFormat = useCallback((index: number) => {
    if (!editing || editing.outputFormats.length <= 1) return;
    setEditing({
      ...editing,
      outputFormats: editing.outputFormats.filter((_, i) => i !== index),
    });
  }, [editing]);

  const updateFormat = useCallback((index: number, updates: Partial<WatchOutputFormat>) => {
    if (!editing) return;
    const formats = [...editing.outputFormats];
    formats[index] = { ...formats[index], ...updates };
    setEditing({ ...editing, outputFormats: formats });
  }, [editing]);

  const save = useCallback(async () => {
    if (!editing || !editing.path || !editing.outputDir) return;
    await saveFolder(editing);
    setEditing(null);
  }, [editing, saveFolder]);

  const activityList = Object.values(activity);
  const enabledCount = folders.filter((f) => f.enabled).length;

  return (
    <div className="page">
      <div className="ops-row" style={{ justifyContent: "space-between", marginBottom: 16 }}>
        <div>
          <ScrambleText as="span" className="ops-rune" text="ᛟ" hover ticks={2} />
          <span className="label" style={{ display: "inline", marginLeft: 8 }}>watch folders</span>
        </div>
        <div className="ops-row">
          <span className="ops-sub">
            {paused ? "paused" : `watching ${enabledCount}/${folders.length}`}
          </span>
          <button
            className={`btn watch-pause-btn${paused ? " active" : ""}`}
            onClick={() => setPaused(!paused)}
          >
            {paused ? "resume" : "pause"}
          </button>
          <button className="btn btn-primary" onClick={() => setEditing(emptyFolder())}>
            add folder
          </button>
        </div>
      </div>

      {/* Folder list */}
      {folders.length === 0 && !editing && (
        <div className="card" style={{ textAlign: "center", padding: 32 }}>
          <span className="ops-sub">no watch folders configured — click "add folder"</span>
        </div>
      )}

      {folders.map((f) => (
        <div className="card" key={f.id}>
          <div className="ops-row" style={{ justifyContent: "space-between" }}>
            <div>
              <div style={{ fontSize: 14 }}>{f.path || "(no path)"}</div>
              <div className="ops-sub">
                {f.patterns.length ? f.patterns.join(", ") : "all files"} ·{" "}
                {f.action === "autoConvert" ? "auto-convert" : "queue"} ·{" "}
                {f.outputFormats.map((fmt) => fmt.outputFormat).join(" + ")}
                {f.conflictPolicy !== "skip" ? ` · ${f.conflictPolicy}` : ""}
                {f.deleteSource ? " · delete source" : ""}
                {f.recursive ? " · subfolders" : ""}
                {f.recursive && f.preservePath ? " · preserve paths" : ""}
                {f.settleMs !== 10000 ? ` · ${f.settleMs / 1000}s debounce` : ""}
                {f.ignoreOlderThanMinutes > 0 ? ` · skip >${f.ignoreOlderThanMinutes}m` : ""}
              </div>
            </div>
            <div className="ops-row">
              <button
                className={`ops-toggle${f.enabled ? " on" : ""}`}
                style={{ padding: "6px 16px" }}
                onClick={async () => {
                  const updated = await saveFolder({ ...f, enabled: !f.enabled });
                  void updated;
                }}
              >
                {f.enabled ? "on" : "off"}
              </button>
              <button className="btn" onClick={() => setEditing({ ...f })}>edit</button>
              <button className="btn" onClick={() => deleteFolder(f.id)}>delete</button>
            </div>
          </div>

          {/* Processing log (collapsible) */}
          {f.processingLog.length > 0 && (
            <div style={{ marginTop: 10, borderTop: "1px solid var(--fg-faint)", paddingTop: 8 }}>
              <button
                className="btn"
                style={{ fontSize: 12, padding: "2px 8px" }}
                onClick={() => setExpandedLog(expandedLog === f.id ? null : f.id)}
              >
                {expandedLog === f.id ? "▼" : "▶"} history ({f.processingLog.length})
              </button>
              {expandedLog === f.id && (
                <div style={{ marginTop: 6, maxHeight: 200, overflowY: "auto" }}>
                  {f.processingLog.slice(0, 20).map((entry, i) => {
                    const badge = logStatusBadge(entry.status);
                    return (
                      <div
                        key={i}
                        className="ops-row"
                        style={{ justifyContent: "space-between", fontSize: 12, padding: "2px 0" }}
                      >
                        <span className="ops-sub" style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {entry.inputPath.split(/[/\\]/).pop()}
                        </span>
                        <span className={`log-badge ${badge.cls}`} style={{ padding: "1px 6px", fontSize: 11 }}>
                          {badge.label}
                        </span>
                        <span className="ops-sub" style={{ fontSize: 10, marginLeft: 8 }}>
                          {new Date(entry.timestamp).toLocaleTimeString()}
                        </span>
                      </div>
                    );
                  })}
                  <div className="ops-row" style={{ marginTop: 6 }}>
                    <button
                      className="btn"
                      style={{ fontSize: 11, padding: "2px 8px" }}
                      onClick={() => clearLog(f.id)}
                    >
                      clear history
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      ))}

      {/* Editor */}
      {editing && (
        <div className="card ops-card" style={{ marginTop: 16 }}>
          <div className="ops-header" style={{ borderBottom: "1px solid var(--fg-faint)" }}>
            <span className="ops-rune">ᚦ</span>
            <span className="label">{editing.id ? "edit folder" : "new folder"}</span>
          </div>
          <div className="ops-body" style={{ overflow: "visible" }}>
            {/* Folder to watch */}
            <div className="ops-group">
              <span className="ops-group-label">folder to watch</span>
              <div className="ops-row">
                <span className="ops-sub" style={{ flex: 1 }}>{editing.path || "—"}</span>
                <button className="btn" onClick={pickFolder}>browse</button>
              </div>
            </div>

            {/* Output folder */}
            <div className="ops-group">
              <span className="ops-group-label">output folder</span>
              <div className="ops-row">
                <span className="ops-sub" style={{ flex: 1 }}>{editing.outputDir || "—"}</span>
                <button className="btn" onClick={pickOutput}>browse</button>
              </div>
            </div>

            {/* Glob patterns */}
            <div className="ops-group">
              <span className="ops-group-label">file patterns (empty = all)</span>
              <div className="ops-row">
                {editing.patterns.map((p) => (
                  <button
                    key={p}
                    className="ops-toggle on"
                    onClick={() => setEditing({ ...editing, patterns: editing.patterns.filter((x) => x !== p) })}
                  >{p} ✕</button>
                ))}
                <input
                  className="ops-field-input"
                  value={patternInput}
                  onChange={(ev) => setPatternInput(ev.target.value)}
                  onKeyDown={(ev) => ev.key === "Enter" && addPattern()}
                  placeholder="*.mp4"
                  style={{ background: "transparent", border: "1px solid var(--fg-faint)", color: "var(--fg)", padding: "4px 6px", fontFamily: "inherit", fontSize: 13, width: 120 }}
                />
                <button className="btn" onClick={addPattern}>add</button>
              </div>
              <div className="ops-sub" style={{ fontSize: 11, marginTop: 4 }}>
                glob patterns: *.mp4, *_hq.*, screenshot_*.png
              </div>
            </div>

            {/* Output formats */}
            <div className="ops-group">
              <span className="ops-group-label">output formats</span>
              {editing.outputFormats.map((fmt, i) => (
                <div key={i} style={{ marginBottom: 8, padding: 8, border: "1px solid var(--fg-faint)", borderRadius: 4 }}>
                  <div className="ops-row" style={{ justifyContent: "space-between", marginBottom: 4 }}>
                    <span className="ops-sub">{i === 0 ? "primary" : `format ${i + 1}`}</span>
                    {editing.outputFormats.length > 1 && (
                      <button
                        className="btn"
                        style={{ fontSize: 11, padding: "2px 8px" }}
                        onClick={() => removeFormat(i)}
                      >remove</button>
                    )}
                  </div>
                  <div className="ops-row" style={{ gap: 8 }}>
                    <Dropdown
                      options={FORMAT_OPTIONS}
                      value={fmt.outputFormat}
                      onChange={(v) => updateFormat(i, { outputFormat: v })}
                      showCategories
                    />
                    <span className="ops-sub" style={{ whiteSpace: "nowrap" }}>
                      quality ({Math.round((fmt.quality ?? 0.8) * 100)}%)
                    </span>
                  </div>
                  <input
                    type="range"
                    className="watch-slider"
                    min={0}
                    max={1}
                    step={0.05}
                    value={fmt.quality ?? 0.8}
                    onChange={(e) => updateFormat(i, { quality: Number(e.target.value) })}
                    style={{ width: "100%", marginTop: 4 }}
                  />
                  <div className="ops-row" style={{ marginTop: 4 }}>
                    <span className="ops-sub" style={{ fontSize: 11, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {fmt.outputDir || `default: ${editing.outputDir || "—"}`}
                    </span>
                    <button
                      className="btn"
                      style={{ fontSize: 11, padding: "2px 8px" }}
                      onClick={() => pickFormatOutput(i)}
                    >custom dir</button>
                  </div>
                </div>
              ))}
              <button className="btn" style={{ fontSize: 12, padding: "4px 12px" }} onClick={addFormat}>
                + add format
              </button>
            </div>

            {/* Debounce window */}
            <div className="ops-group">
              <span className="ops-group-label">
                debounce window ({(editing.settleMs / 1000).toFixed(1)}s)
              </span>
              <input
                type="range"
                className="watch-slider"
                min={0.5}
                max={60}
                step={0.5}
                value={editing.settleMs / 1000}
                onChange={(e) => setEditing({ ...editing, settleMs: Math.round(Number(e.target.value) * 1000) })}
              />
              <div className="ops-sub" style={{ fontSize: 11 }}>
                wait for file to be stable before processing
              </div>
            </div>

            {/* File age filter */}
            <div className="ops-group">
              <span className="ops-group-label">ignore files older than</span>
              <div className="ops-row" style={{ gap: 8 }}>
                <input
                  type="number"
                  min={0}
                  max={10080}
                  value={editing.ignoreOlderThanMinutes}
                  onChange={(e) => setEditing({ ...editing, ignoreOlderThanMinutes: Math.max(0, Number(e.target.value)) })}
                  style={{ background: "transparent", border: "1px solid var(--fg-faint)", color: "var(--fg)", padding: "4px 6px", fontFamily: "inherit", fontSize: 13, width: 80 }}
                />
                <span className="ops-sub">minutes (0 = no limit)</span>
              </div>
            </div>

            {/* Conflict policy */}
            <div className="ops-group">
              <span className="ops-group-label">when output exists</span>
              <Dropdown
                options={CONFLICT_OPTIONS}
                value={editing.conflictPolicy}
                onChange={(v) => setEditing({ ...editing, conflictPolicy: v as ConflictPolicy })}
              />
            </div>

            {/* Action */}
            <div className="ops-group">
              <span className="ops-group-label">action</span>
              <Dropdown
                options={ACTION_OPTIONS}
                value={editing.action}
                onChange={(v) => setEditing({ ...editing, action: v as WatchAction })}
              />
            </div>

            <label className="ops-row watch-check-row" style={{ gap: 8 }}>
              <input
                type="checkbox"
                className="watch-check"
                checked={editing.deleteSource}
                onChange={(e) => setEditing({ ...editing, deleteSource: e.target.checked })}
              />
              <span className="ops-sub">delete source after successful auto-convert</span>
            </label>

            <label className="ops-row watch-check-row" style={{ gap: 8 }}>
              <input
                type="checkbox"
                className="watch-check"
                checked={editing.recursive}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    recursive: e.target.checked,
                    preservePath: e.target.checked ? true : false,
                  })
                }
              />
              <span className="ops-sub">include subfolders</span>
            </label>

            {editing.recursive && (
              <label
                className="ops-row watch-check-row"
                style={{ gap: 8, marginLeft: 28 }}
              >
                <input
                  type="checkbox"
                  className="watch-check"
                  checked={editing.preservePath}
                  onChange={(e) =>
                    setEditing({ ...editing, preservePath: e.target.checked })
                  }
                />
                <span className="ops-sub">preserve subfolder structure in output</span>
              </label>
            )}

            <div className="convert-actions" style={{ marginTop: 8 }}>
              <button className="btn btn-primary" onClick={save} disabled={!editing.path || !editing.outputDir}>
                save
              </button>
              <button className="btn" onClick={() => setEditing(null)}>cancel</button>
            </div>
          </div>
        </div>
      )}

      {/* Live activity */}
      {activityList.length > 0 && (
        <>
          <ScrambleText as="div" className="rune-divider" text="ᛟ ᛟ ᛟ" hover ticks={2} />
          <div className="card">
            <span className="label">active conversions</span>
            {activityList.map((a) => {
              const key = `${a.folderId}:${a.path}`;
              return (
                <div className="ops-row" key={key} style={{ justifyContent: "space-between", marginTop: 8 }}>
                  <span className="ops-sub" style={{ flex: 1 }}>{a.path.split(/[/\\]/).pop()}</span>
                  {a.status === "running" && (
                    <div className="progress-bar-container" style={{ flex: 1, margin: "0 12px" }}>
                      <div className="progress-bar" style={{ width: `${a.progress * 100}%` }} />
                    </div>
                  )}
                  {a.status === "done" && a.outputPath && (
                    <button className="btn" onClick={() => revealItemInDir(a.outputPath!)}>show</button>
                  )}
                  {a.status === "error" && <span className="alert-error" style={{ padding: "2px 6px" }}>failed</span>}
                </div>
              );
            })}
          </div>
        </>
      )}

      {/* Queue */}
      {queue.length > 0 && (
        <>
          <ScrambleText as="div" className="rune-divider" text="ᛟ ᛟ ᛟ" hover ticks={2} />
          <div className="card">
            <div className="ops-row" style={{ justifyContent: "space-between" }}>
              <span className="label" style={{ margin: 0 }}>queue ({queue.length})</span>
              <button className="btn" onClick={clearQueue}>clear all</button>
            </div>
            {queue.map((q) => (
              <div className="ops-row" key={q.id} style={{ justifyContent: "space-between", marginTop: 8 }}>
                <span className="ops-sub" style={{ flex: 1 }}>{q.name}</span>
                <button className="btn btn-primary" onClick={() => convertQueued(q.id)}>convert</button>
                <button className="btn" onClick={() => dequeue(q.id)}>remove</button>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
