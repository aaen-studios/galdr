import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring } from "remotion";
import { COLORS, FONT_STACK, RUNE, VIDEO } from "../theme";

/**
 * Scene 5 — Forge (0:23–0:30, 420 frames).
 * Label 'ᛏ FORGE' (Tiwaz = actions). An animated timeline editor mockup: clips
 * slide onto multiple tracks, a playhead sweeps across, a preview frame updates.
 * The richest scene — shows galdr's creative-tool UI craft.
 */
export const Forge: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = VIDEO;

  const labelIn = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 24 });

  // panel reveal
  const panelScale = spring({ frame: frame - 10, fps, config: { damping: 200 }, durationInFrames: 30 });

  // tracks
  const tracks = [
    { label: "V1", color: COLORS.fg, clips: [{ start: 0, len: 0.4 }, { start: 0.42, len: 0.3 }] },
    { label: "V2", color: COLORS.fgDim, clips: [{ start: 0.1, len: 0.25 }] },
    { label: "A1", color: COLORS.flag, clips: [{ start: 0.05, len: 0.7 }] },
  ];

  // playhead sweeps 0..1 over frames 120..360
  const playhead = interpolate(frame, [120, 360], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  return (
    <AbsoluteFill style={{ background: COLORS.bg }}>
      {/* label */}
      <div
        style={{
          position: "absolute",
          top: 110,
          left: 160,
          fontFamily: FONT_STACK,
          color: COLORS.fg,
          fontSize: 38,
          letterSpacing: "0.2em",
          opacity: labelIn,
          transform: `translateY(${(1 - labelIn) * -20}px)`,
        }}
      >
        {RUNE.tiwaz} FORGE
      </div>

      {/* editor mockup */}
      <div
        style={{
          position: "absolute",
          left: 160,
          right: 160,
          top: 200,
          bottom: 110,
          border: `1px solid ${COLORS.fgFaint}`,
          background: COLORS.bgDim,
          display: "flex",
          flexDirection: "column",
          transform: `scale(${0.92 + panelScale * 0.08})`,
          opacity: panelScale,
        }}
      >
        {/* toolbar */}
        <div style={{ display: "flex", gap: 24, padding: "14px 20px", borderBottom: `1px solid ${COLORS.fgFaint}`, fontFamily: FONT_STACK, fontSize: 16, color: COLORS.fgDim, letterSpacing: "0.15em" }}>
          <span style={{ color: COLORS.fg }}>ᛏ FORGE</span>
          <span>PROJECT.GALDR</span>
          <span style={{ marginLeft: "auto" }}>{Math.round(playhead * 100)}%</span>
        </div>

        {/* preview + timeline split */}
        <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
          {/* preview */}
          <div style={{ width: 360, borderRight: `1px solid ${COLORS.fgFaint}`, padding: 20, display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ fontFamily: FONT_STACK, fontSize: 13, color: COLORS.fgDim, letterSpacing: "0.15em" }}>PREVIEW</div>
            <div
              style={{
                flex: 1,
                border: `1px solid ${COLORS.fgFaint}`,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                position: "relative",
                overflow: "hidden",
              }}
            >
              {/* a compositing preview: layered rectangles that the playhead reveals */}
              <div style={{ position: "absolute", width: "70%", height: "40%", border: `2px solid ${COLORS.fg}`, opacity: 0.3 + playhead * 0.7 }} />
              <div style={{ position: "absolute", width: "40%", height: "40%", border: `2px solid ${COLORS.flag}`, transform: `translateX(${(playhead - 0.5) * 120}px)`, opacity: 0.4 + playhead * 0.6 }} />
              <span style={{ fontFamily: FONT_STACK, fontSize: 40, color: COLORS.fg, opacity: 0.6 }}>ᚷ</span>
            </div>
          </div>

          {/* timeline */}
          <div style={{ flex: 1, padding: 20, display: "flex", flexDirection: "column", gap: 16, position: "relative" }}>
            {/* ruler */}
            <div style={{ display: "flex", fontFamily: FONT_STACK, fontSize: 12, color: COLORS.fgFaint, letterSpacing: "0.1em", marginLeft: 60 }}>
              {Array.from({ length: 10 }, (_, i) => (
                <span key={i} style={{ flex: 1, borderLeft: `1px solid ${COLORS.fgFaint}`, paddingLeft: 4 }}>{i}s</span>
              ))}
            </div>

            {tracks.map((tr, ti) => (
              <div key={ti} style={{ display: "flex", alignItems: "center", gap: 12, height: 56 }}>
                <span style={{ width: 48, fontFamily: FONT_STACK, fontSize: 16, color: tr.color, letterSpacing: "0.1em" }}>{tr.label}</span>
                <div style={{ flex: 1, height: 48, position: "relative", borderTop: `1px solid ${COLORS.fgFaint}` }}>
                  {tr.clips.map((c, ci) => {
                    const clipIn = spring({ frame: frame - 40 - ti * 20 - ci * 15, fps, config: { damping: 200 }, durationInFrames: 24 });
                    return (
                      <div
                        key={ci}
                        style={{
                          position: "absolute",
                          left: `${c.start * 100}%`,
                          width: `${c.len * 100}%`,
                          top: 6,
                          bottom: 6,
                          border: `1px solid ${tr.color}`,
                          background: ti === 2 ? "rgba(139,200,255,0.08)" : "rgba(200,200,200,0.06)",
                          opacity: clipIn,
                          transform: `translateX(${(1 - clipIn) * 40}px)`,
                          display: "flex",
                          alignItems: "center",
                          paddingLeft: 8,
                          fontFamily: FONT_STACK,
                          fontSize: 12,
                          color: tr.color,
                          letterSpacing: "0.1em",
                        }}
                      >
                        {ti === 2 ? "AUDIO" : ti === 1 ? "OVERLAY" : "CLIP"}
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}

            {/* playhead */}
            <div
              style={{
                position: "absolute",
                left: `calc(60px + ${playhead} * (100% - 60px))`,
                top: 8,
                bottom: 8,
                width: 2,
                background: COLORS.fg,
                boxShadow: "0 0 8px rgba(200,200,200,0.7)",
              }}
            />
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
