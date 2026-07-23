// Brand constants for the galdr marketing reel.
// Sourced from galdr's actual design system (src/App.css :root + docs/STYLE_GUIDE.md).
// The deployed app uses #c8c8c8 (not pure #fff) for foreground — we match that.

export const COLORS = {
  bg: "#000000",
  bgDim: "#111111",
  fg: "#c8c8c8",
  fgDim: "#6a6a6a",
  fgFaint: "#262626",
  flag: "#8bc8ff", // token accent from command-preview
} as const;

// Monospace stack. Cascadia Mono is loaded via @remotion/google-fonts in Root.tsx
// for reproducible rendering; Noto Sans Runic (also loaded there) is the fallback
// the browser uses automatically for Elder Futhark glyphs, which the OS monospace
// stack may not include.
export const FONT_STACK =
  '"Cascadia Mono", "Noto Sans Runic", "Courier New", "Liberation Mono", monospace';

// Elder Futhark — the rune set used throughout galdr.
export const RUNES = "ᚠᚢᚦᚨᚱᚲᚷᚹᚺᚾᛁᛃᛇᛈᛉᛊᛏᛒᛖᛗᛚᛝᛟᛞ";

// Named runes (from the style guide).
export const RUNE = {
  fehu: "ᚠ", // start
  ansuz: "ᚨ", // messages
  gebo: "ᚷ", // gifts/forms
  kaunan: "ᚲ", // callouts
  tiwaz: "ᛏ", // actions
  othala: "ᛟ", // footer/meta
} as const;

// Video format.
export const VIDEO = {
  width: 1920,
  height: 1080,
  fps: 60,
} as const;

// Scene timing in frames (60fps). Total = 2160 frames = 36s.
export const TIMING = {
  boot: { from: 0, durationInFrames: 300 },
  logoReveal: { from: 300, durationInFrames: 240 },
  convert: { from: 540, durationInFrames: 420 },
  transcribe: { from: 960, durationInFrames: 420 },
  forge: { from: 1380, durationInFrames: 420 },
  logoClose: { from: 1800, durationInFrames: 360 },
} as const;

export const TOTAL_DURATION = 2160;
