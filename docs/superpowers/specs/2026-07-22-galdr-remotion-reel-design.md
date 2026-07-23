# Design: galdr marketing reel (Remotion)

**Date:** 2026-07-22
**Status:** Implemented
**Output:** `video/out/galdr-reel.mp4` — 1920×1080, 60fps, ~36s, H.264 + AAC

## Goal

A single, concrete marketing reel for galdr, built with Remotion (React → video), for use as a portfolio piece. Not a feature inside galdr, not a reusable generator — one rendered MP4 that showcases galdr's identity.

## Decisions (from brainstorming)

- **Intent:** one specific marketing video (MP4 output), not an app feature.
- **Subject:** promo for galdr, aimed at a portfolio audience.
- **Format:** landscape reel, ~30–45s, full feature showcase.
- **Style:** galdr's own runic/CRT aesthetic (`#c8c8c8` on black, monospace, Elder Futhark, zero rounded corners).
- **Location:** `video/` subfolder inside the galdr repo, shared Bun/TS toolchain.
- **Narrative:** terminal-boot open → feature showcase → logo close (designer-chosen hybrid).
- **Features shown:** Convert, Transcribe (whisper.cpp), Forge (timeline editor).
- **Audio:** full soundtrack + SFX; royalty-free library track.
- **Closing:** logo + "galdr" only (no tagline, no links).
- **Render target:** 1080p / 60fps.
- **Architecture:** modular scene components (Approach A).

## Storyboard

~36s total at 60fps (2160 frames). Six scenes, each a separate composition.

| # | Scene | Time | Frames | Beat |
|---|---|---|---|---|
| 1 | Boot | 0:00–0:05 | 0–300 | Terminal boot lines type out (`galdr init... ok`, `rune engine... loaded`, `ffmpeg core... bound`, `whisper bindings... awake`). Blinking cursor, scanlines. |
| 2 | Logo Reveal | 0:05–0:09 | 300–540 | Boot dissolves in a rune storm. The galdr sigil draws itself via animated SVG stroke-dashoffset. `galdr` resolves from a rune scramble. |
| 3 | Convert | 0:09–0:16 | 540–960 | `ᚲ CONVERT`. A file passes through a glowing rune gate and emerges transformed (`video.mov → audio.mp3`). An FFmpeg command types out with token-colored flags. |
| 4 | Transcribe | 0:16–0:23 | 960–1380 | `ᚨ TRANSCRIBE`. Waveform animates; transcript lines stream in word-by-word as if whisper.cpp were decoding them live. |
| 5 | Forge | 0:23–0:30 | 1380–1800 | `ᛏ FORGE`. Timeline editor mockup: clips slide onto tracks (V1/V2/A1), a playhead sweeps, a preview frame composites. |
| 6 | Logo Close | 0:30–0:36 | 1800–2160 | Dissolve to black. Sigil fades in with a slow phosphor glow pulse. Just `galdr`. Hold. |

**Transitions between scenes:** the five galdr overlays (rune-dissolve, terminal-scroll, runic-portal, ink-ripple, angular-carve) are ported as ~0.5s `<Sequence>` overlays that wipe over the outgoing scene's tail.

## Architecture

Master `GaldrReel` composition sequences six scene components via `<Sequence>`. Each scene is also registered standalone in `Root.tsx` so it can be previewed in isolation in Remotion Studio.

Shared primitives:
- `RuneScramble` — deterministic, frame-driven port of galdr's `ScrambleText` (`random(seed)` + frame-derived step, no `Math.random`).
- `CrtOverlay` — persistent scanline + vignette, ported from `.to-terminal-scroll::before`.
- `Transitions.tsx` — the five overlay ports + a `TerminalLine` typing helper.

## Brand fidelity

Sourced from galdr's real design system:
- Foreground `#c8c8c8` on `#000` (the deployed `App.css :root` value, not the style guide's pure `#fff`).
- `#111` surfaces, `#8bc8ff` flag-token accent (from command preview), `#6a6a6a`/`#262626` muted/border.
- Pure monospace, all-caps headings, `letter-spacing 0.15em`, border-radius 0, no shadows (except phosphor glow).
- Elder Futhark runes as ornament.
- Sigil from `src-tauri/icons/app-icon.svg`.

## Fonts

Bundled via `@remotion/google-fonts` for reproducible cross-machine rendering:
- **Cascadia Mono** — primary face.
- **Noto Sans Runic** — fallback the browser uses automatically for Elder Futhark glyphs (the OS monospace stack may lack the Runic Unicode block).

## Audio

Royalty-free dark-ambient cinematic track from Pixabay (Pixabay Content License: free for commercial/personal use, no attribution). 186s source; reel uses a 36s window starting ~8s in, with a tail fade-out over the logo close. Wired via Remotion `<Audio>` with a frame-based volume callback.

## Verification

- All six scenes render as stills without errors.
- Runic glyphs confirmed rendering as actual angular glyphs (not tofu boxes) after font bundling.
- Full render completed: 2160/2160 frames → `out/galdr-reel.mp4`, 3.9 MB.
- ffprobe confirms: 1920×1080, 60fps, 36.05s, H.264 video + AAC 48kHz stereo audio.
