import { useEffect, useRef, useState } from "react";

interface Props {
  /** Called with absolute file/folder paths when files are dropped. */
  onFiles: (paths: string[]) => void;
  /** Called when a URL (http/https) is dropped. Optional. */
  onUrl?: (url: string) => void;
}

/**
 * A window-wide drag-and-drop overlay. Mount once near the app root.
 *
 * Uses a counter to track nested dragenter/dragleave events (the browser fires
 * one pair per element crossed), so the overlay only hides when the cursor
 * truly leaves the window. On drop it routes file paths to `onFiles` and URLs
 * to `onUrl`. Renders null unless a drag is in progress.
 *
 * Inline styles only — no App.css changes required. Pulls from the CSS tokens
 * (--scrim, --fg, --fg-dim, --fg-faint, --info).
 */
export default function DropZone({ onFiles, onUrl }: Props) {
  const [active, setActive] = useState(false);
  // Ref so the drag counter survives re-renders without causing them.
  const depthRef = useRef(0);

  useEffect(() => {
    // Preventing the default on dragover is what tells the browser "we accept
    // drops here" — without it the drop event never fires.
    const onDragEnter = (e: DragEvent) => {
      if (!e.dataTransfer || !hasDropData(e.dataTransfer)) return;
      e.preventDefault();
      depthRef.current += 1;
      if (depthRef.current === 1) setActive(true);
    };

    const onDragOver = (e: DragEvent) => {
      if (!e.dataTransfer || !hasDropData(e.dataTransfer)) return;
      e.preventDefault();
      // A dropEffect hint keeps the cursor from showing the "blocked" glyph.
      e.dataTransfer.dropEffect = "copy";
    };

    const onDragLeave = (e: DragEvent) => {
      e.preventDefault();
      depthRef.current = Math.max(0, depthRef.current - 1);
      if (depthRef.current === 0) setActive(false);
    };

    const onDrop = (e: DragEvent) => {
      e.preventDefault();
      depthRef.current = 0;
      setActive(false);
      if (!e.dataTransfer) return;

      const url = readDroppedUrl(e.dataTransfer);
      if (url && onUrl) {
        onUrl(url);
        return;
      }

      const paths = readDroppedPaths(e.dataTransfer);
      if (paths.length > 0) onFiles(paths);
    };

    window.addEventListener("dragenter", onDragEnter);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("dragleave", onDragLeave);
    window.addEventListener("drop", onDrop);

    return () => {
      window.removeEventListener("dragenter", onDragEnter);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("dragleave", onDragLeave);
      window.removeEventListener("drop", onDrop);
    };
  }, [onFiles, onUrl]);

  if (!active) return null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 6000,
        background: "var(--scrim)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        pointerEvents: "none", // let the drop land on the window, not this div
        fontFamily: "var(--font-mono, ui-monospace, monospace)",
      }}
      aria-hidden={true}
    >
      <div
        style={{
          border: `2px dashed var(--info, #8bc8ff)`,
          borderRadius: 12,
          padding: "48px 64px",
          background: "var(--bg-dim, #111)",
          color: "var(--fg, #c8c8c8)",
          textAlign: "center",
          boxShadow: "0 0 0 1px var(--fg-faint, #262626)",
        }}
      >
        <div style={{ fontSize: 40, lineHeight: 1, marginBottom: 16 }}>ᛞ</div>
        <div style={{ fontSize: 15, letterSpacing: "0.04em" }}>
          drop files, folders, or urls
        </div>
        <div style={{ fontSize: 11, color: "var(--fg-dim, #6a6a6a)", marginTop: 8 }}>
          release to add to queue
        </div>
      </div>
    </div>
  );
}

/** True if the data transfer carries files or a recognised text/url flavour. */
function hasDropData(dt: DataTransfer): boolean {
  if (dt.types && dt.types.length > 0) {
    // During dragover the files list is empty (browser hides it for security),
    // but types like "Files" or "text/uri-list" are still advertised.
    for (const t of dt.types) {
      if (t === "Files" || t === "text/uri-list" || t === "text/plain") return true;
    }
  }
  return dt.files && dt.files.length > 0;
}

/**
 * Pull absolute paths out of a drop. In Tauri/webkit, dropped files expose
 * their real filesystem path via the File's `.path`. Fall back to the name
 * when path isn't available (some embedded contexts strip it).
 */
function readDroppedPaths(dt: DataTransfer): string[] {
  const paths: string[] = [];
  if (dt.files && dt.files.length > 0) {
    for (let i = 0; i < dt.files.length; i++) {
      const f = dt.files[i] as File & { path?: string };
      if (f.path) paths.push(f.path);
    }
  }
  // Some Tauri builds expose paths via items/DataTransferItem.getAsFile too.
  if (paths.length === 0 && dt.items && dt.items.length > 0) {
    for (let i = 0; i < dt.items.length; i++) {
      const item = dt.items[i];
      if (item.kind === "file") {
        const f = item.getAsFile() as (File & { path?: string }) | null;
        const path = f?.path;
        if (path) paths.push(path);
      }
    }
  }
  return paths;
}

/** If the drop carries an http(s) URL, return it; otherwise null. */
function readDroppedUrl(dt: DataTransfer): string | null {
  const fromUriList = dt.getData("text/uri-list");
  if (fromUriList) {
    const url = fromUriList.split(/\r?\n/).map((l) => l.trim()).find((l) => l && !l.startsWith("#"));
    if (url && /^https?:\/\//i.test(url)) return url;
  }
  const plain = dt.getData("text/plain");
  if (plain && /^https?:\/\//i.test(plain.trim())) return plain.trim();
  return null;
}
