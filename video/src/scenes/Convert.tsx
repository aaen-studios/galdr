import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring } from "remotion";
import { COLORS, FONT_STACK, RUNE, VIDEO } from "../theme";

/**
 * Scene 3 — Convert (0:09–0:16, 420 frames).
 * Label 'ᚲ CONVERT' (Kaunan = callouts). A file icon travels through a glowing
 * rune gate and emerges as a different format; an FFmpeg command types out
 * below in token-colored monospace (#8bc8ff for flags).
 */
export const Convert: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = VIDEO;

  // label slides in
  const labelIn = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 24 });

  // file travels left→gate→right over frames 40..240
  const travel = interpolate(frame, [40, 240], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const fileX = interpolate(travel, [0, 0.5, 1], [-360, 0, 360]);
  const fileOpacity = interpolate(travel, [0, 0.08, 0.42, 0.58, 0.92, 1], [0, 1, 1, 0, 1, 1]);

  // gate pulse intensifies as the file passes through (0.4..0.6)
  const gateIntensity = interpolate(travel, [0.3, 0.5, 0.7], [0.3, 1, 0.3], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  // format swap mid-gate
  const isOutput = travel > 0.5;
  const fileName = isOutput ? "audio.mp3" : "video.mov";

  // command types in from frame 200
  const cmd = "ffmpeg -i video.mov -c:a libmp3lame -q:a 2 audio.mp3";
  const cmdStart = 200;
  const localCmd = frame - cmdStart;
  const cmdChars = localCmd < 0 ? 0 : Math.min(cmd.length, Math.floor((localCmd / 2)));
  const shownCmd = cmd.slice(0, cmdChars);

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
        {RUNE.kaunan} CONVERT
      </div>

      {/* the gate */}
      <div
        style={{
          position: "absolute",
          left: "50%",
          top: "48%",
          transform: "translate(-50%, -50%)",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
        }}
      >
        <div style={{ display: "flex", gap: 40, fontSize: 90, fontFamily: FONT_STACK, color: COLORS.fg }}>
          <span style={{ opacity: 0.4 + gateIntensity * 0.6 }}>ᛏ</span>
          <span style={{ opacity: 0.4 + gateIntensity * 0.6, transform: `scale(${1 + gateIntensity * 0.15})` }}>ᚷ</span>
          <span style={{ opacity: 0.4 + gateIntensity * 0.6 }}>ᛟ</span>
        </div>
        {/* vertical gate bars */}
        <div style={{ position: "relative", width: 8, height: 220, marginTop: -120 }}>
          <div style={{ position: "absolute", inset: 0, background: COLORS.fg, opacity: 0.15 }} />
          <div
            style={{
              position: "absolute",
              inset: 0,
              background: COLORS.fg,
              opacity: gateIntensity * 0.5,
              boxShadow: `0 0 ${30 * gateIntensity}px rgba(200,200,200,${gateIntensity})`,
            }}
          />
        </div>
      </div>

      {/* traveling file */}
      <div
        style={{
          position: "absolute",
          left: "50%",
          top: "48%",
          transform: `translate(calc(-50% + ${fileX}px), -50%)`,
          opacity: fileOpacity,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 10,
        }}
      >
        <div
          style={{
            width: 120,
            height: 150,
            border: `2px solid ${COLORS.fg}`,
            background: COLORS.bgDim,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontFamily: FONT_STACK,
            color: COLORS.fgDim,
            fontSize: 14,
            letterSpacing: "0.1em",
          }}
        >
          {isOutput ? "MP3" : "MOV"}
        </div>
        <div style={{ fontFamily: FONT_STACK, color: COLORS.fg, fontSize: 20, letterSpacing: "0.1em" }}>{fileName}</div>
      </div>

      {/* command */}
      <div
        style={{
          position: "absolute",
          bottom: 150,
          left: 160,
          right: 160,
          fontFamily: FONT_STACK,
          fontSize: 30,
          letterSpacing: "0.05em",
          whiteSpace: "pre",
          minHeight: 40,
        }}
      >
        <CommandTokens text={shownCmd} full={cmd} />
        {cmdChars < cmd.length && cmdChars > 0 && (
          <span style={{ display: "inline-block", width: 16, height: 30, background: COLORS.fg, marginLeft: 4, verticalAlign: "middle" }} />
        )}
      </div>
    </AbsoluteFill>
  );
};

/** Renders a typed FFmpeg command with flag tokens (#8bc8ff), matching galdr's command preview. */
const CommandTokens: React.FC<{ text: string; full: string }> = ({ text }) => {
  const tokens = text.split(/(\s+)/);
  return (
    <>
      {tokens.map((tok, i) => {
        const isFlag = tok.startsWith("-");
        const isPath = /\.(mov|mp3|mp4|wav|mkv|flac|m4a)$/i.test(tok);
        return (
          <span
            key={i}
            style={{
              color: isFlag ? COLORS.flag : isPath ? COLORS.fg : COLORS.fgDim,
            }}
          >
            {tok}
          </span>
        );
      })}
    </>
  );
};
