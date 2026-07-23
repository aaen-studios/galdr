import { AnimatePresence, motion } from "framer-motion";
import { useToastStore, type ToastKind } from "../store/toastStore";

/** Severity → runic glyph. Reuses the app's existing Elder Futhark vocabulary. */
const GLYPH: Record<ToastKind, string> = {
  success: "ᛟ",
  error: "ᚹ",
  warn: "ᛏ",
  info: "ᚠ",
};

export default function Toaster() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <div className="toaster" aria-live="polite">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <motion.div
            key={t.id}
            className={`toast toast-${t.kind}`}
            layout
            initial={{ opacity: 0, x: 24, scale: 0.98 }}
            animate={{ opacity: 1, x: 0, scale: 1 }}
            exit={{ opacity: 0, x: 24, scale: 0.98 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            role={t.kind === "error" ? "alert" : "status"}
          >
            <span className="toast-glyph">{GLYPH[t.kind]}</span>
            <div className="toast-body">
              <div className="toast-title">{t.title}</div>
              {t.message && <div className="toast-message">{t.message}</div>}
              {t.action && (
                <button
                  className="toast-action"
                  onClick={() => {
                    t.action?.onClick();
                    dismiss(t.id);
                  }}
                >
                  {t.action.label}
                </button>
              )}
            </div>
            <button
              className="toast-close"
              onClick={() => dismiss(t.id)}
              aria-label="dismiss"
            >
              ×
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
