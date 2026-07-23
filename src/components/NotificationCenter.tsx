import { useState, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useToastStore } from "../store/toastStore";

const GLYPH: Record<string, string> = {
  success: "ᛟ",
  error: "ᚹ",
  warn: "ᛏ",
  info: "ᚠ",
};

function fmtTime(ts: number): string {
  const diff = Date.now() - ts;
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  return new Date(ts).toLocaleDateString();
}

export default function NotificationCenter() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const log = useToastStore((s) => s.log);
  const unread = useToastStore((s) => s.unread);
  const markRead = useToastStore((s) => s.markRead);
  const clearLog = useToastStore((s) => s.clearLog);

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  // Mark read when opened.
  useEffect(() => {
    if (open && unread > 0) markRead();
  }, [open, unread, markRead]);

  return (
    <div className="notif-center" ref={ref}>
      <button
        className={`titlebar-btn notif-bell${unread > 0 ? " has-unread" : ""}`}
        onClick={() => setOpen((o) => !o)}
        title="notifications"
        aria-label="notifications"
      >
        ᚱ
        {unread > 0 && <span className="notif-badge">{unread > 9 ? "9+" : unread}</span>}
      </button>
      <AnimatePresence>
        {open && (
          <motion.div
            className="notif-panel"
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
            transition={{ duration: 0.14 }}
          >
            <div className="notif-panel-head">
              <span className="notif-panel-title">notifications</span>
              {log.length > 0 && (
                <button className="btn notif-clear" onClick={clearLog}>clear</button>
              )}
            </div>
            {log.length === 0 ? (
              <div className="notif-empty">no notifications yet</div>
            ) : (
              <div className="notif-list">
                {log.map((n) => (
                  <div key={n.id} className={`notif-item notif-${n.kind}`}>
                    <span className="notif-glyph">{GLYPH[n.kind]}</span>
                    <div className="notif-content">
                      <div className="notif-title">{n.title}</div>
                      {n.message && <div className="notif-message">{n.message}</div>}
                      <div className="notif-time">{fmtTime(n.timestamp)}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
