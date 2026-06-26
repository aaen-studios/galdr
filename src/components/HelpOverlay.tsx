import { useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";

interface Props {
  open: boolean;
  onClose: () => void;
}

interface Shortcut {
  keys: string;
  desc: string;
}

interface Section {
  title: string;
  items: Shortcut[];
}

const SECTIONS: Section[] = [
  {
    title: "Navigation",
    items: [
      { keys: "Ctrl+1", desc: "Home" },
      { keys: "Ctrl+2", desc: "Convert" },
      { keys: "Ctrl+3", desc: "Compress" },
      { keys: "Ctrl+4", desc: "Forge editor" },
      { keys: "Ctrl+5", desc: "Subtitles" },
      { keys: "Ctrl+6", desc: "Watch folders" },
      { keys: "Ctrl+7", desc: "Rune tags" },
    ],
  },
  {
    title: "App",
    items: [
      { keys: "Ctrl+K", desc: "Command palette" },
      { keys: "Ctrl+,", desc: "Settings" },
      { keys: "?", desc: "Toggle this help" },
    ],
  },
  {
    title: "Forge editor",
    items: [
      { keys: "Ctrl+Z", desc: "Undo" },
      { keys: "Ctrl+Y", desc: "Redo" },
      { keys: "Ctrl+S", desc: "Save project" },
      { keys: "Space", desc: "Play / pause" },
      { keys: "Delete", desc: "Remove clip" },
      { keys: "S", desc: "Split at playhead" },
      { keys: "I", desc: "Set in-point" },
      { keys: "O", desc: "Set out-point" },
      { keys: "← →", desc: "Nudge playhead" },
      { keys: "Home", desc: "Go to start" },
      { keys: "End", desc: "Go to end" },
    ],
  },
  {
    title: "Transcript editor",
    items: [
      { keys: "Ctrl+Z", desc: "Undo" },
      { keys: "Ctrl+Y", desc: "Redo" },
      { keys: "Ctrl+S", desc: "Save" },
      { keys: "Ctrl+F", desc: "Search" },
      { keys: "Space", desc: "Play / pause" },
      { keys: "Delete", desc: "Delete cue" },
      { keys: "↑ ↓", desc: "Navigate cues" },
      { keys: "Enter", desc: "Edit cue text" },
    ],
  },
];

export default function HelpOverlay({ open, onClose }: Props) {
  const panelRef = useRef<HTMLDivElement>(null);

  // Focus the panel on mount so keyboard works immediately
  useEffect(() => {
    if (open) {
      panelRef.current?.focus();
    }
  }, [open]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="help-overlay"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.1 }}
          onClick={onClose}
        >
          <motion.div
            ref={panelRef}
            className="help-panel"
            initial={{ opacity: 0, scale: 0.97 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.97 }}
            transition={{ duration: 0.1 }}
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.key === "Escape") onClose();
            }}
            tabIndex={-1}
          >
            <div className="help-header">
              <span className="help-rune">ᚷ</span>
              <span className="help-title">Keyboard Shortcuts</span>
              <button className="help-close" onClick={onClose}>
                ×
              </button>
            </div>
            <div className="help-body">
              {SECTIONS.map((section) => (
                <div key={section.title} className="help-section">
                  <div className="help-section-title">{section.title}</div>
                  {section.items.map((item) => (
                    <div key={item.keys} className="help-row">
                      <kbd className="help-key">{item.keys}</kbd>
                      <span className="help-desc">{item.desc}</span>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
