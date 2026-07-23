import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate } from "remotion";
import { COLORS, FONT_STACK } from "../theme";
import { TerminalLine } from "../components/Transitions";

/**
 * Scene 1 — Boot (0:00–0:05, 300 frames).
 * Black screen, terminal boot lines type out one after another, blinking cursor.
 * Borrowed copy from galdr's terminal-scroll transition (src/transitions.tsx):
 *   galdr init ... ok
 *   rune engine ... loaded
 *   ffmpeg core ... bound
 *   whisper bindings ... awake
 */
export const Boot: React.FC = () => {
  const frame = useCurrentFrame();
  const lines = [
    { text: "galdr init ... ok", at: 6 },
    { text: "rune engine ... loaded", at: 54 },
    { text: "ffmpeg core ... bound", at: 102 },
    { text: "whisper bindings ... awake", at: 150 },
    { text: "_", at: 198 }, // prompt
  ];

  // whole-screen subtle fade in at the very start
  const introOpacity = interpolate(frame, [0, 12], [0, 1], { extrapolateRight: "clamp" });

  return (
    <AbsoluteFill style={{ background: COLORS.bg }}>
      <div
        style={{
          position: "absolute",
          inset: 0,
          padding: "120px 160px",
          display: "flex",
          flexDirection: "column",
          gap: 18,
          opacity: introOpacity,
        }}
      >
        <div
          style={{
            fontFamily: FONT_STACK,
            color: COLORS.fgDim,
            fontSize: 22,
            letterSpacing: "0.1em",
            marginBottom: 24,
          }}
        >
          ᚠ GALDR // MEDIA WORKBENCH
        </div>
        {lines.map((l, i) => (
          <TerminalLine key={i} text={l.text} startFrame={l.at} typeFrames={6} />
        ))}
      </div>
    </AbsoluteFill>
  );
};
