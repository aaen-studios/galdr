import { useMemo, useState, useRef, useCallback } from "react";
import type { ConversionParams, HardwareEncoderInfo } from "../types";
import { buildFFmpegCommand } from "../utils/ffmpegBuilder";
import { getFlagDef, type FlagDef } from "../utils/ffmpegSyntax";

interface CommandPreviewProps {
  params: ConversionParams;
  outputDir?: string;
  mediaType?: "video" | "audio" | "image" | null;
  /** Source media duration in seconds — used for accurate bitrate preview in target-size mode. */
  duration?: number;
  /** Available hardware encoders (for resolving "auto" in the preview). */
  availableEncoders?: HardwareEncoderInfo[];
}

type TokenType = "binary" | "flag" | "value" | "path" | "text";

interface Token {
  text: string;
  type: TokenType;
}

function tokenize(cmd: string): Token[] {
  const tokens: Token[] = [];
  const parts: string[] = [];
  let current = "";
  let inQuote = false;

  for (let i = 0; i < cmd.length; i++) {
    const ch = cmd[i];
    if (ch === '"') {
      inQuote = !inQuote;
      current += ch;
    } else if (ch === " " && !inQuote) {
      if (current) {
        parts.push(current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current) parts.push(current);

  for (let i = 0; i < parts.length; i++) {
    const p = parts[i];
    if (i === 0 && p === "ffmpeg") {
      tokens.push({ text: p, type: "binary" });
    } else if (p.startsWith("-") && !/^\d/.test(p)) {
      tokens.push({ text: p, type: "flag" });
    } else if (i > 0 && tokens[tokens.length - 1]?.type === "flag") {
      const looksLikePath = p.includes("/") || p.includes("\\") || /\.[a-zA-Z0-9]{1,4}$/.test(p);
      tokens.push({ text: p, type: looksLikePath ? "path" : "value" });
    } else if (p.startsWith('"') && p.endsWith('"') && (p.includes("/") || p.includes("\\") || /\.[a-zA-Z0-9]{1,4}$/.test(p))) {
      tokens.push({ text: p, type: "path" });
    } else {
      tokens.push({ text: p, type: "text" });
    }
  }

  return tokens;
}

export default function CommandPreview({ params, outputDir, duration, availableEncoders }: CommandPreviewProps) {
  const cmd = useMemo(() => {
    const merged: ConversionParams = {
      ...params,
      output_dir: outputDir || params.output_dir,
    };
    return buildFFmpegCommand(merged, duration, availableEncoders);
  }, [params, outputDir, duration, availableEncoders]);

  const tokens = useMemo(() => tokenize(cmd), [cmd]);

  const [copied, setCopied] = useState(false);
  const [tooltip, setTooltip] = useState<{ flag: string; def: FlagDef; x: number; y: number } | null>(null);
  const copyTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(cmd).then(() => {
      setCopied(true);
      if (copyTimer.current) clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(false), 1500);
    });
  }, [cmd]);

  const handleFlagHover = useCallback((e: React.MouseEvent, flag: string) => {
    const def = getFlagDef(flag);
    if (!def) return;
    const rect = (e.target as HTMLElement).getBoundingClientRect();
    setTooltip({ flag, def, x: rect.left + rect.width / 2, y: rect.top - 4 });
  }, []);

  const handleFlagLeave = useCallback(() => {
    setTooltip(null);
  }, []);

  const hasInput = !!params.input_path;

  return (
    <div className="command-preview">
      <div className="command-preview-header">
        <span className="command-preview-label">ᚠ command alchemy</span>
        {hasInput && (
          <button className="command-preview-copy" onClick={handleCopy} title="copy command">
            {copied ? "copied" : "copy"}
          </button>
        )}
      </div>
      <div className="command-preview-body">
        {!hasInput ? (
          <span className="command-preview-placeholder">{`// drop a file to see the incantation`}</span>
        ) : (
          <code className="command-preview-code">
            {tokens.map((t, i) => {
              if (t.type === "flag") {
                return (
                  <span
                    key={i}
                    className={`command-token command-token-flag`}
                    onMouseEnter={(e) => handleFlagHover(e, t.text)}
                    onMouseLeave={handleFlagLeave}
                  >
                    {t.text}
                  </span>
                );
              }
              return (
                <span key={i} className={`command-token command-token-${t.type}`}>
                  {t.text}
                </span>
              );
            })}
          </code>
        )}
      </div>
      {tooltip && (
        <div
          className="command-tooltip"
          style={{ left: tooltip.x, top: tooltip.y }}
        >
          <span className="command-tooltip-flag">{tooltip.def.flag}</span>
          <span className="command-tooltip-cat">[{tooltip.def.category}]</span>
          <span className="command-tooltip-desc">{tooltip.def.description}</span>
        </div>
      )}
    </div>
  );
}
