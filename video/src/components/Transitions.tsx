import React from "react";
import { useCurrentFrame, interpolate, random } from "remotion";
import { COLORS, RUNES, FONT_STACK } from "../theme";

/**
 * Frame-driven ports of galdr's five page-transition overlays
 * (src/transitions.css). Each is a full-frame component meant to be placed
 * inside a short <Sequence> that overlaps two scenes by ~30 frames (0.5s).
 *
 * They use interpolate() on the current frame so progress is deterministic and
 * scrubbable, and random(seed) for reproducible rune placement.
 *
 * Usage: place the chosen overlay as the LAST layer of the outgoing scene's
 * Sequence (or in its own overlapping Sequence) so it wipes over the content.
 */

const COVER: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  background: COLORS.bg,
  overflow: "hidden",
};

/* ---------------- Rune Dissolve — parallax rune storm ---------------- */
export const RuneDissolve: React.FC<{ durationInFrames?: number }> = ({
  durationInFrames = 30,
}) => {
  const frame = useCurrentFrame();
  const progress = (frame / durationInFrames) * 100; // 0..100 like the CSS %

  const runes = Array.from({ length: 60 }, (_, i) => {
    const x = random(`rd-x-${i}`) * 100;
    const drift = (random(`rd-d-${i}`) - 0.5) * 40;
    const startOpacity = random(`rd-o-${i}`) * 0.6 + 0.4;
    const speed = random(`rd-s-${i}`) * 0.6 + 0.7;
    const delay = random(`rd-dl-${i}`) * 30;
    const rot = random(`rd-r-${i}`) * 360;

    // local progress 0..1 across a fall cycle offset by delay
    const local = Math.max(0, Math.min(1, (progress * speed - delay) / 60));
    const y = interpolate(local, [0, 1], [-12, 112]);
    const opacity =
      local <= 0 ? 0 : interpolate(local, [0, 0.1, 0.5, 1], [0, startOpacity, startOpacity * 0.6, 0]);
    const size = 14 + random(`rd-sz-${i}`) * 36;
    const char = RUNES[Math.floor(random(`rd-c-${i}`) * RUNES.length)];

    return (
      <span
        key={i}
        style={{
          position: "absolute",
          left: `${x}%`,
          top: `${y}%`,
          transform: `translateX(${drift}px) rotate(${rot * local}deg)`,
          opacity,
          fontSize: size,
          color: COLORS.fgDim,
          fontFamily: FONT_STACK,
          mixBlendMode: "screen",
        }}
      >
        {char}
      </span>
    );
  });

  return (
    <div style={{ ...COVER, perspective: 600 }}>
      <div style={{ position: "absolute", inset: 0 }}>{runes}</div>
    </div>
  );
};

/* ---------------- Terminal Scroll — phosphor boot lines ----------------
   (Used structurally inside the Boot scene; exported here for reuse.) */
export const TerminalLine: React.FC<{
  text: string;
  startFrame: number;
  typeFrames?: number;
}> = ({ text, startFrame, typeFrames = 12 }) => {
  const frame = useCurrentFrame();
  const local = frame - startFrame;
  if (local < 0) return null;
  const chars = Math.min(text.length, Math.floor((local / typeFrames) * text.length));
  const shown = text.slice(0, chars);
  const glow = interpolate(local, [0, 4, 12], [0, 0.4, 0], { extrapolateRight: "clamp" });
  return (
    <div
      style={{
        fontFamily: FONT_STACK,
        color: COLORS.fg,
        textTransform: "uppercase",
        letterSpacing: "0.05em",
        fontSize: 28,
        textShadow: glow > 0 ? `0 0 12px rgba(200,200,200,${glow}), 0 0 4px rgba(200,200,200,${glow / 2})` : "none",
      }}
    >
      {shown}
      {chars < text.length && <span style={{ background: COLORS.fg, width: 14, height: 28, display: "inline-block", marginLeft: 2, boxShadow: "0 0 6px rgba(200,200,200,0.5)" }} />}
    </div>
  );
};

/* ---------------- Runic Portal — expanding counter-rotating rune rings ---------------- */
export const RunicPortal: React.FC<{ durationInFrames?: number }> = ({
  durationInFrames = 30,
}) => {
  const frame = useCurrentFrame();
  const p = frame / durationInFrames; // 0..1
  const ringScale = interpolate(p, [0, 0.25, 1], [0.3, 1, 1.6]);
  const ringOpacity = interpolate(p, [0, 0.25, 0.7, 1], [0, 1, 0.8, 0]);

  const renderRing = (count: number, radius: number, dir: 1 | -1, ringSeed: string, fontSize: number) => {
    return Array.from({ length: count }, (_, i) => {
      const angle = (i / count) * 360;
      const rad = (angle * Math.PI) / 180;
      const x = Math.cos(rad) * radius;
      const y = Math.sin(rad) * radius;
      const char = RUNES[i % RUNES.length];
      return (
        <span
          key={i}
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            transform: `translate(-50%, -50%) translate(${x}px, ${y}px) rotate(${angle + dir * p * 360}deg)`,
            color: COLORS.fgDim,
            fontSize,
            fontFamily: FONT_STACK,
          }}
        >
          {char}
        </span>
      );
    });
  };

  return (
    <div style={COVER}>
      <div
        style={{
          position: "absolute",
          left: "50%",
          top: "50%",
          transform: `translate(-50%, -50%) scale(${ringScale})`,
          opacity: ringOpacity,
          width: 0,
          height: 0,
        }}
      >
        {renderRing(12, 180, 1, "rp1", 34)}
        {renderRing(16, 300, -1, "rp2", 28)}
        {renderRing(20, 430, 1, "rp3", 22)}
      </div>
      {/* center glow */}
      <div
        style={{
          position: "absolute",
          left: "50%",
          top: "50%",
          width: 16,
          height: 16,
          borderRadius: "50%",
          background: COLORS.fg,
          transform: `translate(-50%, -50%) scale(${interpolate(p, [0, 0.3, 0.6, 1], [0.8, 1.2, 1, 1.5])})`,
          opacity: interpolate(p, [0, 0.3, 0.6, 1], [0, 0.7, 1, 0]),
          filter: "blur(3px)",
        }}
      />
    </div>
  );
};

/* ---------------- Ink Ripple — bleeding concentric ripples ---------------- */
export const InkRipple: React.FC<{ durationInFrames?: number }> = ({
  durationInFrames = 30,
}) => {
  const frame = useCurrentFrame();
  const rings = Array.from({ length: 4 }, (_, i) => {
    const start = i * 4;
    const local = Math.max(0, (frame - start) / durationInFrames);
    if (local > 1 || local < 0) return null;
    const scale = interpolate(local, [0, 1], [0.15, 1.8]);
    const opacity = interpolate(local, [0, 0.3, 0.7, 1], [0, 0.35, 0.15, 0]);
    const blur = interpolate(local, [0, 1], [8, 0]);
    return (
      <div
        key={i}
        style={{
          position: "absolute",
          left: "50%",
          top: "50%",
          width: 700,
          height: 700,
          border: `2px solid ${COLORS.fg}`,
          borderRadius: "50%",
          transform: `translate(-50%, -50%) scale(${scale})`,
          opacity,
          filter: `blur(${blur}px)`,
        }}
      />
    );
  });

  // rune sparks
  const sparks = Array.from({ length: 14 }, (_, i) => {
    const angle = (i / 14) * 360;
    const dist = interpolate(frame / durationInFrames, [0, 1], [0, 400 + random(`ir-d-${i}`) * 200]);
    const rad = (angle * Math.PI) / 180;
    const x = Math.cos(rad) * dist;
    const y = Math.sin(rad) * dist;
    const opacity = interpolate(frame / durationInFrames, [0, 0.4, 1], [1, 0.6, 0]);
    return (
      <span
        key={`s${i}`}
        style={{
          position: "absolute",
          left: "50%",
          top: "50%",
          transform: `translate(${x}px, ${y}px)`,
          color: COLORS.fgDim,
          fontFamily: FONT_STACK,
          fontSize: 22,
          opacity,
        }}
      >
        {RUNES[i % RUNES.length]}
      </span>
    );
  });

  return (
    <div style={COVER}>
      {rings}
      {sparks}
    </div>
  );
};

/* ---------------- Angular Carve — dual crossing slashes + flash ---------------- */
export const AngularCarve: React.FC<{ durationInFrames?: number }> = ({
  durationInFrames = 18,
}) => {
  const frame = useCurrentFrame();
  const p = frame / durationInFrames; // 0..1

  const fwdX = interpolate(p, [0, 1], [-60, 160]);
  const revX = interpolate(p, [0, 1], [160, -60]);
  const bladeOpacity = interpolate(p, [0, 0.1, 0.85, 1], [0, 1, 0.9, 0]);
  const flashOpacity = interpolate(p, [0, 0.4, 0.55, 1], [0, 0.6, 0.8, 0], { extrapolateRight: "clamp" });

  const bladeGradient =
    "linear-gradient(135deg, transparent 0%, rgba(200,200,200,0.03) 25%, rgba(200,200,200,0.08) 45%, rgba(200,200,200,0.12) 50%, rgba(200,200,200,0.08) 55%, rgba(200,200,200,0.03) 75%, transparent 100%)";

  const sparks = Array.from({ length: 8 }, (_, i) => {
    const x = random(`ac-x-${i}`) * 100;
    const sparkY = interpolate(p, [0, 1], [0, -60]);
    const sparkOpacity = interpolate(p, [0, 0.6, 1], [1, 0.6, 0]);
    return (
      <span
        key={i}
        style={{
          position: "absolute",
          left: `${x}%`,
          top: `${30 + random(`ac-ty-${i}`) * 40}%`,
          transform: `translateY(${sparkY}px) scale(${1 - p})`,
          color: COLORS.fgDim,
          fontFamily: FONT_STACK,
          fontSize: 22,
          opacity: sparkOpacity,
        }}
      >
        {RUNES[i % RUNES.length]}
      </span>
    );
  });

  return (
    <div style={COVER}>
      <div
        style={{
          position: "absolute",
          top: "-5%",
          left: 0,
          width: "45%",
          height: "110%",
          background: bladeGradient,
          transform: `translateX(${fwdX}%) skewX(-22deg)`,
          opacity: bladeOpacity,
        }}
      />
      <div
        style={{
          position: "absolute",
          top: "-5%",
          right: 0,
          width: "45%",
          height: "110%",
          background: bladeGradient,
          transform: `translateX(${revX}%) skewX(22deg)`,
          opacity: bladeOpacity,
        }}
      />
      <div
        style={{
          position: "absolute",
          top: "50%",
          left: "50%",
          width: 600,
          height: 600,
          transform: "translate(-50%, -50%)",
          background: "radial-gradient(circle, rgba(200,200,200,0.2) 0%, transparent 70%)",
          opacity: flashOpacity,
        }}
      />
      {sparks}
    </div>
  );
};
