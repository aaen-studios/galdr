import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate } from "remotion";
import { COLORS, FONT_STACK } from "../theme";

/**
 * Scene 6 — Logo Close (0:30–0:36, 360 frames).
 * Everything dissolves to black. The galdr sigil fades back in, centered, with a
 * slow phosphor glow pulse. Just 'galdr' in monospace beneath it. Nothing else.
 */
export const LogoClose: React.FC = () => {
  const frame = useCurrentFrame();

  const fadeIn = interpolate(frame, [0, 40], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  // slow glow pulse across the whole scene
  const pulse = 0.35 + Math.sin(frame * 0.06) * 0.2;
  // final ~60 frames hold, then subtle dim before cut
  const endDim = interpolate(frame, [320, 360], [1, 0.7], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const paths = [
    { x1: 200, y1: 50, x2: 200, y2: 355 },
    { x1: 115, y1: 95, x2: 285, y2: 215 },
    { x1: 285, y1: 95, x2: 115, y2: 215 },
    { x1: 115, y1: 95, x2: 115, y2: 215 },
    { x1: 285, y1: 95, x2: 285, y2: 215 },
    { x1: 200, y1: 300, x2: 155, y2: 345 },
    { x1: 200, y1: 300, x2: 245, y2: 345 },
    { x1: 200, y1: 328, x2: 170, y2: 355 },
    { x1: 200, y1: 328, x2: 230, y2: 355 },
  ];

  return (
    <AbsoluteFill
      style={{
        background: COLORS.bg,
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
        opacity: fadeIn * endDim,
      }}
    >
      <svg
        viewBox="0 0 400 400"
        width={380}
        height={380}
        style={{ filter: `drop-shadow(0 0 ${28}px rgba(200,200,200,${pulse}))` }}
      >
        {paths.map((p, i) => (
          <line
            key={i}
            {...p}
            stroke={COLORS.fg}
            strokeWidth={12}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        ))}
        <circle cx="200" cy="62" r="9" fill={COLORS.fg} />
      </svg>
      <div style={{ marginTop: 24, fontFamily: FONT_STACK, color: COLORS.fg, fontSize: 56, letterSpacing: "0.3em" }}>
        galdr
      </div>
    </AbsoluteFill>
  );
};
