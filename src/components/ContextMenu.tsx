import { createContext, useContext, useState, useCallback, useEffect, useRef, type ReactNode } from "react";
import { motion, AnimatePresence } from "framer-motion";

export interface ContextMenuItem {
  label: string;
  rune?: string;
  action: () => void;
  disabled?: boolean;
  divider?: boolean;
}

interface ContextMenuState {
  x: number;
  y: number;
  items: ContextMenuItem[];
}

interface ContextMenuContextValue {
  show: (e: React.MouseEvent | MouseEvent, items: ContextMenuItem[]) => void;
  hide: () => void;
}

const CtxMenuContext = createContext<ContextMenuContextValue | null>(null);

export function useContextMenu(): ContextMenuContextValue {
  const ctx = useContext(CtxMenuContext);
  if (!ctx) throw new Error("useContextMenu must be used within ContextMenuProvider");
  return ctx;
}

export function ContextMenuProvider({ children }: { children: ReactNode }) {
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [highlightIdx, setHighlightIdx] = useState(-1);
  const menuRef = useRef<HTMLDivElement>(null);

  const show = useCallback((e: React.MouseEvent | MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, items });
    setHighlightIdx(-1);
  }, []);

  const hide = useCallback(() => {
    setMenu(null);
    setHighlightIdx(-1);
  }, []);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!menu) return;
    if (e.key === "Escape") {
      hide();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setHighlightIdx((prev) => {
        const items = menu.items;
        let next = prev < items.length - 1 ? prev + 1 : 0;
        while (next < items.length && (items[next].divider || items[next].disabled)) {
          next = next < items.length - 1 ? next + 1 : 0;
          if (next === prev) break;
        }
        return next;
      });
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setHighlightIdx((prev) => {
        const items = menu.items;
        let next = prev > 0 ? prev - 1 : items.length - 1;
        while (next >= 0 && (items[next].divider || items[next].disabled)) {
          next = next > 0 ? next - 1 : items.length - 1;
          if (next === prev) break;
        }
        return next;
      });
      return;
    }
    if (e.key === "Enter" && highlightIdx >= 0) {
      const item = menu.items[highlightIdx];
      if (item && !item.disabled && !item.divider) {
        e.preventDefault();
        item.action();
        hide();
      }
    }
  }, [menu, highlightIdx, hide]);

  useEffect(() => {
    if (!menu) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [menu, handleKeyDown]);

  useEffect(() => {
    if (!menu || !menuRef.current) return;
    const rect = menuRef.current.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let x = menu.x;
    let y = menu.y;
    if (x + rect.width > vw - 8) x = vw - rect.width - 8;
    if (y + rect.height > vh - 8) y = vh - rect.height - 8;
    if (x !== menu.x || y !== menu.y) {
      setMenu((prev) => prev ? { ...prev, x, y } : prev);
    }
  }, [menu]);

  return (
    <CtxMenuContext.Provider value={{ show, hide }}>
      {children}
      <AnimatePresence>
        {menu && (
          <motion.div
            className="ctx-backdrop"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.1 }}
            onClick={hide}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              const x = e.clientX;
              const y = e.clientY;
              setMenu(null);
              setHighlightIdx(-1);
              requestAnimationFrame(() => {
                const target = document.elementFromPoint(x, y);
                if (target) {
                  target.dispatchEvent(new MouseEvent("contextmenu", {
                    clientX: x, clientY: y, bubbles: true, cancelable: true,
                  }));
                }
              });
            }}
          >
            <motion.div
              ref={menuRef}
              className="ctx-menu"
              initial={{ opacity: 0, scale: 0.97 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.97 }}
              transition={{ duration: 0.1 }}
              style={{ left: menu.x, top: menu.y }}
              onClick={(e) => e.stopPropagation()}
              onContextMenu={(e) => { e.preventDefault(); e.stopPropagation(); }}
              role="menu"
            >
              {menu.items.map((item, i) => {
                if (item.divider) {
                  return <div key={i} className="ctx-menu-divider" />;
                }
                return (
                  <div
                    key={i}
                    className={`ctx-menu-item${highlightIdx === i ? " highlighted" : ""}${item.disabled ? " disabled" : ""}`}
                    onClick={() => {
                      if (!item.disabled) {
                        item.action();
                        hide();
                      }
                    }}
                    onMouseEnter={() => setHighlightIdx(i)}
                    role="menuitem"
                    tabIndex={-1}
                  >
                    {item.rune && <span className="ctx-menu-rune">{item.rune}</span>}
                    <span className="ctx-menu-label">{item.label}</span>
                  </div>
                );
              })}
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </CtxMenuContext.Provider>
  );
}