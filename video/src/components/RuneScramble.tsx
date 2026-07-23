import React from "react";
import { useCurrentFrame, random } from "remotion";
import { RUNES, FONT_STACK, COLORS } from "../theme";

/**
 * Frame-driven, deterministic port of galdr's ScrambleText component.
 * The original (src/components/ScrambleText.tsx) scrambles text through a rune
 * storm that decays to the real string over `ticks` steps. There it's driven by
 * setInterval + Math.random; here we derive the step from the current frame and
 * seed each character with `random(seed)` so renders are perfectly reproducible.
 *
 * The scramble begins at `startFrame` and resolves over `ticks` steps spaced
 * `frameStep` frames apart.
 */

interface Props {
  text: string;
  /** Frame at which the scramble begins resolving. */
  startFrame: number;
  /** Total resolution time in frames. */
  durationInFrames?: number;
  /** Resolution steps (more = slower crawl). Default 16. */
  ticks?: number;
  fontSize?: number;
  letterSpacing?: string;
  color?: string;
  style?: React.CSSProperties;
  seed?: number;
}

export const RuneScramble: React.FC<Props> = ({
  text,
  startFrame,
  durationInFrames = 60,
  ticks = 16,
  fontSize = 48,
  letterSpacing = "0.15em",
  color = COLORS.fg,
  style,
  seed = 1,
}) => {
  const frame = useCurrentFrame();
  const localFrame = frame - startFrame;

  // Before the scramble window, show full runes (a storm). After, show the text.
  if (localFrame < 0) {
    return (
      <span style={{ fontFamily: FONT_STACK, color, fontSize, letterSpacing, ...style }}>
        {text
          .split("")
          .map((ch, i) => (ch === " " ? " " : random(seed + i) < 0.5 ? " " : RUNES[Math.floor(random(seed + i + 100) * RUNES.length)]))
          .join("")}
      </span>
    );
  }

  const step = Math.min(ticks, Math.floor((localFrame / durationInFrames) * ticks));
  const factor = Math.max(0, 1 - step / ticks);

  if (factor === 0) {
    return (
      <span style={{ fontFamily: FONT_STACK, color, fontSize, letterSpacing, ...style }}>
        {text}
      </span>
    );
  }

  return (
    <span style={{ fontFamily: FONT_STACK, color, fontSize, letterSpacing, ...style }}>
      {text
        .split("")
        .map((ch, i) => {
          if (ch === " ") return " ";
          const r = random(seed + i + step);
          if (r < factor) {
            return RUNES[Math.floor(random(seed + i + step + 200) * RUNES.length)];
          }
          return ch;
        })
        .join("")}
    </span>
  );
};
