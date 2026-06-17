import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import ImageComparison from "./ImageComparison";

interface Props {
  originalPath: string;
  compressedPath: string;
  originalSize: number;
  compressedSize: number;
  duration: number;
}

export default function VideoComparison({
  originalPath,
  compressedPath,
  originalSize,
  compressedSize,
  duration,
}: Props) {
  const [frames, setFrames] = useState<string[] | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const timestamp = Math.min(duration * 0.15, 5.0);
    (async () => {
      try {
        const result = await invoke<string[]>("extract_frames", {
          paths: [originalPath, compressedPath],
          timestamp,
        });
        if (!cancelled) {
          if (result[0] && result[1]) {
            setFrames(result);
          } else {
            setError(true);
          }
        }
      } catch {
        if (!cancelled) setError(true);
      }
    })();
    return () => { cancelled = true; };
  }, [originalPath, compressedPath, duration]);

  if (error) {
    return (
      <div className="comparison-fallback">
        <span className="comparison-fallback-text">preview not available for this format</span>
      </div>
    );
  }

  if (!frames) {
    return (
      <div className="comparison-fallback">
        <span className="comparison-fallback-text">extracting frames...</span>
      </div>
    );
  }

  return (
    <ImageComparison
      originalPath={frames[0]}
      compressedPath={frames[1]}
      originalSize={originalSize}
      compressedSize={compressedSize}
    />
  );
}
