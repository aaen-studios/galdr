import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate } from "remotion";
import { COLORS, FONT_STACK } from "../theme";
import { RuneScramble } from "../components/RuneScramble";

/**
 * Scene 2 — Logo Reveal (0:05–0:09, 240 frames).
 * Boot text dissolves; the galdr sigil (from src-tauri/icons/app-icon.svg)
 * draws itself via animated stroke-dashoffset; the word 'galdr' resolves from a
 * rune scramble beneath it.
 */
export const LogoReveal: React.FC = () => {
  const frame = useCurrentFrame();
  const local = frame; // this scene starts at 0 within its own Sequence

  // sigil draws in over the first ~120 frames
  const drawProgress = interpolate(local, [10, 130], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const sigilGlow = interpolate(local, [80, 140, 200], [0, 0.5, 0.25], { extrapolateRight: "clamp" });

  // whole sigil scale-up as it resolves
  const scale = interpolate(local, [10, 140], [0.7, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // stroke path lengths (rough, for dash animation). 8 lines + 1 dot.
  const paths = [
    { d: "M200 50 L200 355", len: 305 }, // central stave
    { d: "M115 95 L285 215", len: 215 }, // upper X
    { d: "M285 95 L115 215", len: 215 },
    { d: "M115 95 L115 215", len: 120 }, // side verticals
    { d: "M285 95 L285 215", len: 120 },
    { d: "M200 300 L155 345", len: 60 }, // roots
    { d: "M200 300 L245 345", len: 60 },
    { d: "M200 328 L170 355", len: 36 },
    { d: "M200 328 L230 355", len: 36 },
  ];

  const dotOpacity = interpolate(local, [120, 150], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // text scramble resolves between frame 90 and 210
  const textStart = 90;
  const textDuration = 120;

  return (
    <AbsoluteFill
      style={{
        background: COLORS.bg,
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
      }}
    >
      <svg
        viewBox="0 0 400 400"
        width="420"
        height="420"
        style={{
          transform: `scale(${scale})`,
          filter: sigilGlow > 0 ? `drop-shadow(0 0 24px rgba(200,200,200,${sigilGlow}))` : "none",
        }}
      >
        {paths.map((p, i) => {
          const offset = interpolate(drawProgress, [0, 1], [p.len, 0]);
          return (
            <line
              key={i}
              x1={p.d.match(/M(\d+)/)?.[1]}
              y1={p.d.match(/M\d+ (\d+)/)?.[1]}
              x2={p.d.match(/L(\d+)/)?.[1]}
              y2={p.d.match(/L\d+ (\d+)/)?.[1]}
              stroke={COLORS.fg}
              strokeWidth={12}
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeDasharray={p.len}
              strokeDashoffset={offset}
            />
          );
        })}
        <circle cx="200" cy="62" r="9" fill={COLORS.fg} opacity={dotOpacity} />
      </svg>

      <div style={{ marginTop: 40 }}>
        <RuneScramble
          text="galdr"
          startFrame={textStart}
          durationInFrames={textDuration}
          ticks={20}
          fontSize={64}
          letterSpacing="0.3em"
          seed={7}
        />
      </div>
    </AbsoluteFill>
  );
};
