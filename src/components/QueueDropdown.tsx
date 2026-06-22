import { useState, useEffect, useRef, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  useQueueStore,
  selectActive,
  selectFinished,
} from "../store/queueStore";
import type { QueueJob } from "../types";

/** Short status glyph for a job. */
function statusGlyph(status: QueueJob["status"]): string {
  switch (status) {
    case "running":
    case "queued":
      return "ᛃ";
    case "completed":
      return "✓";
    case "failed":
      return "!";
    case "cancelled":
      return "·";
    default:
      return "?";
  }
}

function pct(progress: number): number {
  return Math.max(0, Math.min(100, Math.round(progress * 100)));
}

interface JobRowProps {
  job: QueueJob;
  onCancel: (id: string) => void;
}

function JobRow({ job, onCancel }: JobRowProps) {
  const path = job.outputPath ?? job.inputPath;
  const fileName = path
    ? path.split(/[/\\]/).pop() || path
    : "";
  const isFinished =
    job.status === "completed" || job.status === "failed" || job.status === "cancelled";

  return (
    <div className={`queue-job-row qstatus-${job.status}`}>
      <div className="queue-job-head">
        <span className="queue-job-glyph">{statusGlyph(job.status)}</span>
        <span className="queue-job-label" title={job.label}>{job.label}</span>
      </div>
      {(job.status === "running" || job.status === "queued") && (
        <div className="queue-progress">
          <div className="progress-bar-container">
            <div className="progress-bar" style={{ width: `${pct(job.progress)}%` }} />
          </div>
          <span className="queue-progress-pct">{pct(job.progress)}%</span>
        </div>
      )}
      {job.status === "failed" && job.error && (
        <div className="queue-job-error" title={job.error}>! {job.error}</div>
      )}
      {isFinished && (
        <div className="queue-job-footer">
          {fileName && <span className="queue-job-path" title={path}>{fileName}</span>}
          <div className="queue-job-actions">
            {job.outputPath && (
              <button
                className="queue-job-btn"
                title="show in folder"
                onClick={() =>
                  revealItemInDir(job.outputPath!).catch(() => { })
                }
              >
                folder
              </button>
            )}
          </div>
        </div>
      )}
      {!isFinished && (
        <div className="queue-job-footer">
          <span className="queue-job-path" title={job.inputPath}>
            {fileName}
          </span>
          <div className="queue-job-actions">
            <button
              className="queue-job-btn queue-cancel"
              onClick={() => onCancel(job.id)}
            >
              cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default function QueueDropdown() {
  const jobs = useQueueStore((s) => s.jobs);
  const cancelJob = useQueueStore((s) => s.cancelJob);
  const clearCompleted = useQueueStore((s) => s.clearCompleted);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  const active = useMemo(() => selectActive(jobs), [jobs]);
  const finished = useMemo(() => selectFinished(jobs), [jobs]);
  const activeCount = active.length;
  const hasActive = activeCount > 0;
  const hasFailed = finished.some((j) => j.status === "failed");

  // Close on outside click + Escape
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="queue-dropdown" ref={ref}>
      <button
        ref={triggerRef}
        className={`titlebar-btn queue-trigger${hasActive ? " is-active" : ""}${hasFailed ? " is-failed" : ""}`}
        onClick={() => setOpen((p) => !p)}
        aria-haspopup="true"
        aria-expanded={open}
        title="background queue"
      >
        <span className="ts-rune">ᛟ</span>
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            className="queue-panel"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.12 }}
          >
            {jobs.length === 0 ? (
              <div className="queue-empty">no background jobs</div>
            ) : (
              <>
                {active.length > 0 && (
                  <div className="queue-section">
                    <div className="queue-section-head">
                      <span>active</span>
                      <span className="queue-count">{active.length}</span>
                    </div>
                    {active.map((job) => (
                      <JobRow key={job.id} job={job} onCancel={cancelJob} />
                    ))}
                  </div>
                )}

                {finished.length > 0 && (
                  <div className="queue-section">
                    <div className="queue-section-head">
                      <span>recent</span>
                      <span className="queue-count">{finished.length}</span>
                    </div>
                    {finished.slice(0, 20).map((job) => (
                      <JobRow key={job.id} job={job} onCancel={cancelJob} />
                    ))}
                  </div>
                )}

                {finished.length > 0 && (
                  <button
                    className="queue-clear-btn"
                    onClick={() => clearCompleted()}
                  >
                    clear completed
                  </button>
                )}
              </>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
