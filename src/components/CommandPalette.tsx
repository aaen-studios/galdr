import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";

type Page = "home" | "convert" | "compress" | "settings" | "runes" | "forge" | "watch" | "subtitles" | "import";

interface CommandItem {
  label: string;
  rune: string;
  shortcut?: string;
  target?: Page;
  action?: "help";
}

const COMMANDS: CommandItem[] = [
  { label: "Home", rune: "ᚷ", target: "home" },
  { label: "Quick Convert", rune: "ᛏ", shortcut: "Ctrl+2", target: "convert" },
  { label: "Compress", rune: "ᛉ", shortcut: "Ctrl+3", target: "compress" },
  { label: "Forge Editor", rune: "ᚲ", shortcut: "Ctrl+4", target: "forge" },
  { label: "Subtitles", rune: "ᛊ", shortcut: "Ctrl+5", target: "subtitles" },
  { label: "Watch Folders", rune: "ᚱ", shortcut: "Ctrl+6", target: "watch" },
  { label: "Rune Tags", rune: "ᚠ", shortcut: "Ctrl+7", target: "runes" },
  { label: "Settings", rune: "ᚲ", shortcut: "Ctrl+,", target: "settings" },
  { label: "Keyboard Shortcuts", rune: "ᚷ", shortcut: "?", action: "help" },
];

interface Props {
  open: boolean;
  onClose: () => void;
  onNavigate: (page: Page) => void;
  onShowHelp: () => void;
}

export default function CommandPalette({ open, onClose, onNavigate, onShowHelp }: Props) {
  const [query, setQuery] = useState("");
  const [highlightIdx, setHighlightIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Filter items by query
  const filtered = useMemo(() => {
    if (!query.trim()) return COMMANDS;
    const q = query.toLowerCase();
    return COMMANDS.filter(
      (item) =>
        item.label.toLowerCase().includes(q)
    );
  }, [query]);

  // Reset state on open
  useEffect(() => {
    if (open) {
      setQuery("");
      setHighlightIdx(0);
      // Focus input after animation frame
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  // Clamp highlight when filter changes
  useEffect(() => {
    setHighlightIdx(0);
  }, [filtered.length]);

  const execute = useCallback(
    (item: CommandItem) => {
      if (item.action === "help") {
        onShowHelp();
        onClose();
        return;
      }
      if (item.target) {
        onNavigate(item.target);
      }
      onClose();
    },
    [onNavigate, onClose, onShowHelp]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlightIdx((prev) =>
          prev < filtered.length - 1 ? prev + 1 : 0
        );
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlightIdx((prev) =>
          prev > 0 ? prev - 1 : filtered.length - 1
        );
        return;
      }
      if (e.key === "Enter" && filtered[highlightIdx]) {
        e.preventDefault();
        execute(filtered[highlightIdx]);
      }
    },
    [filtered, highlightIdx, onClose, execute]
  );

  // Keep highlight in bounds if filter shrinks
  const safeHighlight = highlightIdx >= filtered.length ? 0 : highlightIdx;

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="cmd-palette-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.1 }}
          onClick={onClose}
        >
          <motion.div
            className="cmd-palette"
            initial={{ opacity: 0, scale: 0.97, y: -10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: -10 }}
            transition={{ duration: 0.1 }}
            onClick={(e) => e.stopPropagation()}
          >
            <input
              ref={inputRef}
              className="cmd-palette-input"
              type="text"
              placeholder="Search pages & commands…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
            />
            <div className="cmd-palette-results">
              {filtered.length === 0 ? (
                <div className="cmd-palette-empty">No results</div>
              ) : (
                filtered.map((item, i) => (
                  <div
                    key={item.label}
                    className={`cmd-palette-item${safeHighlight === i ? " highlighted" : ""}`}
                    onClick={() => execute(item)}
                    onMouseEnter={() => setHighlightIdx(i)}
                  >
                    <span className="cmd-palette-rune">{item.rune}</span>
                    <span className="cmd-palette-label">{item.label}</span>
                    {item.shortcut && (
                      <kbd className="cmd-palette-hint">{item.shortcut}</kbd>
                    )}
                  </div>
                ))
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
