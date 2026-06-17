import { useState, useRef, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  originalPath: string;
  compressedPath: string;
  originalSize: number;
  compressedSize: number;
}

export default function ImageComparison({
  originalPath,
  compressedPath,
  originalSize,
  compressedSize,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState(50);
  const [hovering, setHovering] = useState(false);
  const [originalSrc, setOriginalSrc] = useState<string | null>(null);
  const [compressedSrc, setCompressedSrc] = useState<string | null>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [orig, comp] = await Promise.all([
          invoke<string>("read_image_data_url", { path: originalPath }),
          invoke<string>("read_image_data_url", { path: compressedPath }),
        ]);
        if (!cancelled) {
          setOriginalSrc(orig);
          setCompressedSrc(comp);
          setLoading(false);
        }
      } catch {
        if (!cancelled) {
          setError(true);
          setLoading(false);
        }
      }
    })();
    return () => { cancelled = true; };
  }, [originalPath, compressedPath]);

  const handleMove = useCallback(
    (clientX: number) => {
      const el = containerRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const x = ((clientX - rect.left) / rect.width) * 100;
      setPos(Math.min(100, Math.max(0, x)));
    },
    [],
  );

  if (loading) {
    return (
      <div className="comparison-fallback">
        <span className="comparison-fallback-text">loading preview...</span>
      </div>
    );
  }

  if (error || !originalSrc || !compressedSrc) {
    return (
      <div className="comparison-fallback">
        <span className="comparison-fallback-text">preview not available</span>
      </div>
    );
  }

  const bigger = compressedSize > originalSize;
  const pct = bigger
    ? `+${Math.round((compressedSize / originalSize - 1) * 100)}%`
    : `-${Math.round((1 - compressedSize / originalSize) * 100)}%`;

  return (
    <div className="comparison-wrapper">
      <div
        ref={containerRef}
        className="comparison-container"
        onMouseMove={(e) => { setHovering(true); handleMove(e.clientX); }}
        onMouseLeave={() => setHovering(false)}
      >
        <div className="comparison-original">
          <img src={originalSrc} alt="original" draggable={false} />
          <span className="comparison-label left">original</span>
        </div>
        <div
          className="comparison-compressed"
          style={{ clipPath: `inset(0 ${100 - pos}% 0 0)` }}
        >
          <img src={compressedSrc} alt="compressed" draggable={false} />
          <span className="comparison-label right">compressed</span>
        </div>
        <div className="comparison-handle" style={{ left: `${pos}%` }}>
          {hovering && <div className="comparison-handle-nub">⟷</div>}
        </div>
        <img
          src={originalSrc}
          alt=""
          draggable={false}
          className="comparison-spacer"
          onLoad={() => {}}
        />
      </div>
      <div className="comparison-pos-info">{pct}</div>
    </div>
  );
}
