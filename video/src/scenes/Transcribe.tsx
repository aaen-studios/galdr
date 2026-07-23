import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, random } from "remotion";
import { COLORS, FONT_STACK, RUNE, VIDEO } from "../theme";

/**
 * Scene 4 — Transcribe (0:16–0:23, 420 frames).
 * Label 'ᚨ TRANSCRIBE' (Ansuz = messages). An animated waveform plays; transcript
 * lines stream in word-by-word as if whisper.cpp were decoding them in real time,
 * with a tracking caret.
 */
export const Transcribe: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = VIDEO;

  const labelIn = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 24 });

  // waveform: 64 bars, each a seeded sin so it bobs deterministically
  const bars = Array.from({ length: 64 }, (_, i) => {
    const phase = random(`wf-${i}`) * Math.PI * 2;
    const amp = (random(`wf-a-${i}`) * 0.5 + 0.5) * interpolate(frame, [20, 60, 360, 420], [0, 1, 1, 0.3], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
    const h = 40 + Math.abs(Math.sin(frame * 0.25 + phase + i * 0.4)) * 120 * amp;
    return (
      <div
        key={i}
        style={{
          width: 6,
          height: h,
          background: COLORS.fg,
          opacity: 0.5 + amp * 0.5,
        }}
      />
    );
  });

  // transcript: a few lines that decode word-by-word from frame 60
  const lines = [
    "the runes remember every spoken word",
    "and inscribe them as light",
    "across the dark of the terminal",
  ];
  const wordsPerLine = lines.map((l) => l.split(" "));
  const totalWords = wordsPerLine.reduce((a, l) => a + l.length, 0);
  const decodeStart = 60;
  const wordsPerFrame = totalWords / 280; // spread over ~280 frames
  const wordsShown = Math.min(totalWords, Math.floor((frame - decodeStart) * wordsPerFrame));

  let rendered: React.ReactNode[] = [];
  let count = 0;
  for (let li = 0; li < wordsPerLine.length && count <= wordsShown; li++) {
    const lineWords = wordsPerLine[li];
    const visibleInLine = Math.max(0, Math.min(lineWords.length, wordsShown - count));
    const partial = lineWords.slice(0, visibleInLine).join(" ");
    const isCurrent = count + lineWords.length > wordsShown && count <= wordsShown;
    rendered.push(
      <div
        key={li}
        style={{
          fontFamily: FONT_STACK,
          fontSize: 34,
          letterSpacing: "0.05em",
          color: isCurrent ? COLORS.fg : COLORS.fgDim,
          minHeight: 48,
          position: "relative",
        }}
      >
        {partial}
        {isCurrent && partial.length > 0 && (
          <span style={{ display: "inline-block", width: 12, height: 30, background: COLORS.fg, marginLeft: 6, verticalAlign: "middle", boxShadow: "0 0 6px rgba(200,200,200,0.6)" }} />
        )}
      </div>
    );
    count += lineWords.length;
  }

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
        {RUNE.ansuz} TRANSCRIBE
      </div>
      <div
        style={{
          position: "absolute",
          top: 110,
          left: 160,
          marginTop: 70,
          fontFamily: FONT_STACK,
          color: COLORS.fgDim,
          fontSize: 20,
          letterSpacing: "0.1em",
          opacity: labelIn,
        }}
      >
        whisper.cpp // local // offline
      </div>

      {/* waveform */}
      <div
        style={{
          position: "absolute",
          top: "32%",
          left: 160,
          right: 160,
          height: 180,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          gap: 6,
          borderBottom: `1px solid ${COLORS.fgFaint}`,
          paddingBottom: 30,
        }}
      >
        {bars}
      </div>

      {/* transcript */}
      <div
        style={{
          position: "absolute",
          top: "58%",
          left: 160,
          right: 160,
          display: "flex",
          flexDirection: "column",
          gap: 18,
        }}
      >
        {rendered}
      </div>
    </AbsoluteFill>
  );
};
