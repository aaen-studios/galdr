import { useEffect, useState, useCallback } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useHistoryStore, relativeTime, formatBytes } from "../store/historyStore";
import { useContextMenu } from "../components/ContextMenu";

interface Props {
  onNavigate: (page: "convert" | "import" | "subtitles") => void;
}

export default function HistoryPage({ onNavigate }: Props) {
  const { entries, loaded, load, remove, clear } = useHistoryStore();
  const [filter, setFilter] = useState<"all" | "completed" | "failed">("all");
  const { show } = useContextMenu();

  useEffect(() => {
    load();
  }, [load]);

  const filtered = entries.filter((e) => filter === "all" || e.status === filter);

  const handleRerun = useCallback((entry: typeof entries[number]) => {
    // Route to the relevant page for re-running. The params snapshot (if any)
    // could be pre-filled, but routing is the primary affordance.
    if (entry.op === "download") onNavigate("import");
    else if (entry.op === "transcription" || entry.op === "subtitle_burn") onNavigate("subtitles");
    else onNavigate("convert");
  }, [onNavigate]);

  return (
    <div className="page">
      <h2>ᚷ history</h2>

      <div className="row" style={{ marginBottom: 16, gap: 8 }}>
        <div className="mode-toggle">
          {(["all", "completed", "failed"] as const).map((f) => (
            <button
              key={f}
              className={`mode-tab${filter === f ? " active" : ""}`}
              onClick={() => setFilter(f)}
            >
              {f}
            </button>
          ))}
        </div>
        <span style={{ flex: 1 }} />
        {entries.length > 0 && (
          <button className="btn" onClick={() => { if (confirm("clear all history?")) clear(); }}>
            clear all
          </button>
        )}
      </div>

      {!loaded ? (
        <div className="skeleton-bar" />
      ) : filtered.length === 0 ? (
        <div className="card" style={{ textAlign: "center", padding: 40 }}>
          <p style={{ color: "var(--fg-dim)" }}>
            {filter === "all" ? "no operations yet — cast something to fill the history." : `no ${filter} operations.`}
          </p>
        </div>
      ) : (
        <div className="history-list">
          {filtered.map((e) => (
            <div
              key={e.id}
              className={`history-item${e.status === "failed" ? " failed" : ""}`}
              onContextMenu={(ev) => show(ev, [
                ...(e.outputPath ? [{ label: "reveal", rune: "ᚨ", action: () => revealItemInDir(e.outputPath!).catch(() => {}) }] : []),
                { label: "rerun", rune: "ᚱ", action: () => handleRerun(e) },
                { label: "remove", rune: "ᛏ", action: () => remove(e.id) },
              ])}
            >
              <span className={`history-dot ${e.status}`} />
              <div className="history-body">
                <div className="history-label">{e.label}</div>
                <div className="history-meta">
                  <span>{relativeTime(e.createdAt)}</span>
                  {e.outputSize && <span>· {formatBytes(e.outputSize)}</span>}
                  {e.outputPath && <span className="history-path">· {e.outputPath}</span>}
                </div>
              </div>
              <button className="btn history-rerun" onClick={() => handleRerun(e)}>
                {e.status === "failed" ? "retry" : "rerun"}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
