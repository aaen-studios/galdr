import { useState, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import type { MediaInfo } from "../types";
import ImageComparison from "./ImageComparison";
import VideoComparison from "./VideoComparison";
import AudioComparison from "./AudioComparison";

interface Props {
  originalPath: string;
  compressedPath: string;
  originalInfo: MediaInfo;
  compressedInfo: MediaInfo;
}

function detectMediaType(mi: MediaInfo): "image" | "video" | "audio" {
  const imageCodecs = ["png", "jpeg", "gif", "bmp", "tiff", "webp", "avif"];
  const isImage =
    mi.container.includes("image") ||
    (mi.duration === 0 && mi.streams.some((s) => imageCodecs.includes(s.codec)));
  if (isImage) return "image";
  if (
    mi.streams.some((s) => s.kind === "audio") &&
    !mi.streams.some((s) => s.kind === "video")
  )
    return "audio";
  return "video";
}

export default function MediaPreview({
  originalPath,
  compressedPath,
  originalInfo,
  compressedInfo,
}: Props) {
  const [open, setOpen] = useState(false);
  const toggle = useCallback(() => setOpen((v) => !v), []);

  const mt = detectMediaType(originalInfo);

  const pct = compressedInfo.size < originalInfo.size
    ? `-${Math.round((1 - compressedInfo.size / originalInfo.size) * 100)}%`
    : `+${Math.round((compressedInfo.size / originalInfo.size - 1) * 100)}%`;

  return (
    <div className="media-preview">
      <button className="preview-toggle" onClick={toggle}>
        <span className={`preview-toggle-arrow${open ? " open" : ""}`}>▸</span>
        <span className="preview-toggle-label">before / after preview</span>
        <span className="preview-toggle-delta">{pct}</span>
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            className="preview-body"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.25, ease: "easeInOut" }}
          >
            <div className="preview-content">
              {(() => {
                if (mt === "image") {
                  return (
                    <ImageComparison
                      originalPath={originalPath}
                      compressedPath={compressedPath}
                      originalSize={originalInfo.size}
                      compressedSize={compressedInfo.size}
                    />
                  );
                }
                if (mt === "video") {
                  return (
                    <VideoComparison
                      originalPath={originalPath}
                      compressedPath={compressedPath}
                      originalSize={originalInfo.size}
                      compressedSize={compressedInfo.size}
                      duration={originalInfo.duration}
                    />
                  );
                }
                if (mt === "audio") {
                  return (
                    <AudioComparison
                      originalInfo={originalInfo}
                      compressedInfo={compressedInfo}
                    />
                  );
                }
                return (
                  <div className="comparison-fallback">
                    <span className="comparison-fallback-text">preview not available for this file type</span>
                  </div>
                );
              })()}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
