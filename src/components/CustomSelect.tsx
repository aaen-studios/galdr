import { useState, useRef, useEffect, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";

interface Option {
  value: string;
  label: string;
  category?: string;
}

interface Props {
  options: Option[];
  value: string;
  onChange: (value: string) => void;
}

export default function CustomSelect({ options, value, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const [menuUp, setMenuUp] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [highlightIdx, setHighlightIdx] = useState(-1);
  const ref = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLUListElement>(null);
  const searchRef = useRef("");
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const hasCat = options.some((o) => o.category);
  const groups: [string, Option[]][] = hasCat
    ? Object.entries(
        options.reduce<Record<string, Option[]>>((acc, o) => {
          const cat = o.category ?? "";
          (acc[cat] ??= []).push(o);
          return acc;
        }, {}),
      )
    : [["", options]];

  const selected = options.find((o) => o.value === value);

  const flatItems = options;

  const visibleCount = flatItems.length;

  useEffect(() => {
    if (!open) {
      setHighlightIdx(-1);
      return;
    }
    const rect = ref.current!.getBoundingClientRect();
    const menuH = menuRef.current?.scrollHeight ?? 200;
    const spaceBelow = window.innerHeight - rect.bottom - 8;
    const spaceAbove = rect.top - 8;
    setMenuUp(menuH > spaceBelow && spaceAbove > spaceBelow);
    triggerRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const handleSelect = useCallback(
    (val: string) => {
      onChange(val);
      setOpen(false);
      triggerRef.current?.focus();
    },
    [onChange],
  );

  const toggleCategory = useCallback((cat: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });
  }, []);

  const scrollToMatch = useCallback((prefix: string) => {
    if (!prefix) return;
    const idx = options.findIndex((o) =>
      o.label.toLowerCase().startsWith(prefix.toLowerCase()),
    );
    if (idx < 0) return;
    const cat = options[idx].category ?? "";
    setCollapsed((prev) => {
      if (prev.has(cat)) {
        const next = new Set(prev);
        next.delete(cat);
        return next;
      }
      return prev;
    });
    setHighlightIdx(idx);
    const items = menuRef.current?.querySelectorAll<HTMLElement>(".cselect-item");
    if (items?.[idx]) items[idx].scrollIntoView({ block: "nearest" });
  }, [options]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Tab") {
        setOpen(false);
        triggerRef.current?.focus();
        if (e.key === "Tab") e.preventDefault();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlightIdx((prev) => {
          const next = prev < visibleCount - 1 ? prev + 1 : 0;
          const items = menuRef.current?.querySelectorAll<HTMLElement>(".cselect-item");
          if (items?.[next]) items[next].scrollIntoView({ block: "nearest" });
          return next;
        });
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlightIdx((prev) => {
          const next = prev > 0 ? prev - 1 : visibleCount - 1;
          const items = menuRef.current?.querySelectorAll<HTMLElement>(".cselect-item");
          if (items?.[next]) items[next].scrollIntoView({ block: "nearest" });
          return next;
        });
        return;
      }
      if (e.key === "Enter" && highlightIdx >= 0) {
        e.preventDefault();
        handleSelect(flatItems[highlightIdx].value);
        return;
      }
      if (e.key.length === 1) {
        searchRef.current += e.key;
        scrollToMatch(searchRef.current);
        clearTimeout(searchTimerRef.current);
        searchTimerRef.current = setTimeout(() => { searchRef.current = ""; }, 500);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      clearTimeout(searchTimerRef.current);
    };
  }, [open, flatItems, visibleCount, highlightIdx, handleSelect, scrollToMatch]);

  const renderGroup = ([cat, items]: [string, Option[]]) => {
    const isCollapsed = collapsed.has(cat);
    const catKey = cat || "__root";
    const startIdx = flatItems.indexOf(items[0]);
    return (
      <li key={catKey}>
        {hasCat && (
          <button
            className={`cselect-category${isCollapsed ? " collapsed" : ""}`}
            onClick={(e) => { e.stopPropagation(); toggleCategory(cat); }}
          >
            <span className="cselect-cat-arrow">{isCollapsed ? "▶" : "▼"}</span>
            <span className="cselect-cat-label">{cat}</span>
          </button>
        )}
        {(!hasCat || !isCollapsed) && (
          <ul className="cselect-group-items">
            {items.map((opt, j) => {
              const idx = startIdx + j;
              return (
                <li
                  key={opt.value}
                  className={`cselect-item${opt.value === value ? " selected" : ""}${highlightIdx === idx ? " highlighted" : ""}`}
                  onClick={() => handleSelect(opt.value)}
                  onMouseEnter={() => setHighlightIdx(idx)}
                >
                  {opt.label}
                </li>
              );
            })}
          </ul>
        )}
      </li>
    );
  };

  return (
    <div className="cselect" ref={ref}>
      <button
        ref={triggerRef}
        className="cselect-trigger"
        onClick={() => setOpen((p) => !p)}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span>{selected ? selected.label : value}</span>
        <span className="cselect-arrow">ᛏ</span>
      </button>
      <AnimatePresence>
        {open && (
          <motion.ul
            ref={menuRef}
            className={`cselect-menu${menuUp ? " menu-up" : ""}`}
            role="listbox"
            aria-activedescendant={highlightIdx >= 0 ? `cselect-opt-${highlightIdx}` : undefined}
            initial={{ opacity: 0, y: menuUp ? 4 : -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: menuUp ? 4 : -4 }}
            transition={{ duration: 0.12 }}
          >
            {groups.map(renderGroup)}
          </motion.ul>
        )}
      </AnimatePresence>
    </div>
  );
}
