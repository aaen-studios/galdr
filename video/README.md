# galdr reel

A ~36s marketing reel for **galdr**, rendered programmatically with [Remotion](https://remotion.dev). Built in galdr's own runic/CRT aesthetic for use as a portfolio piece.

## Output

`out/galdr-reel.mp4` — 1920×1080, 60fps, H.264 + AAC stereo, ~36s.

## Quick start

```bash
cd video
bun install

# open the live preview in Remotion Studio (each scene is a separate composition)
bunx remotion studio src/index.ts

# render the full reel to MP4
bun run render
```

`bun run render` runs `remotion render src/index.ts GaldrReel out/galdr-reel.mp4`.

## Structure

```
video/
├─ src/
│  ├─ index.ts            # registerRoot entry point
│  ├─ Root.tsx            # composition registration (master reel + each scene)
│  ├─ GaldrReel.tsx       # master composition: sequences the 6 scenes + transitions
│  ├─ theme.ts            # brand constants (colors, font stack, runes, timing)
│  ├─ components/
│  │  ├─ RuneScramble.tsx # deterministic, frame-driven port of galdr's ScrambleText
│  │  ├─ CrtOverlay.tsx   # persistent scanline + vignette overlay
│  │  └─ Transitions.tsx  # 5 ported overlays + TerminalLine helper
│  └─ scenes/
│     ├─ Boot.tsx         # 1. terminal boot lines (0:00–0:05)
│     ├─ LogoReveal.tsx   # 2. sigil draws itself + "galdr" scramble (0:05–0:09)
│     ├─ Convert.tsx      # 3. file → rune gate → file + ffmpeg cmd (0:09–0:16)
│     ├─ Transcribe.tsx   # 4. waveform + streaming transcript (0:16–0:23)
│     ├─ Forge.tsx        # 5. animated timeline editor (0:23–0:30)
│     └─ LogoClose.tsx    # 6. sigil + "galdr", glow pulse, hold (0:30–0:36)
├─ public/audio/
│  └─ soundtrack.mp3      # royalty-free dark ambient bed (see license below)
├─ remotion.config.ts
├─ package.json
└─ tsconfig.json
```

## Brand fidelity

Every visual decision comes from galdr's actual design system (`src/App.css` `:root`, `docs/STYLE_GUIDE.md`, `src/transitions.css`):

- **Foreground `#c8c8c8`** on `#000000` (the deployed app's real value, not pure white).
- **Pure monospace**, all-caps headings, `letter-spacing 0.15em`.
- **Border radius 0**, no shadows (except functional phosphor glow).
- **Elder Futhark runes** (`ᚠᚢᚦᚨᚱᚲᚷᚹᚺᚾᛁᛃᛇᛈᛉᛊᛏᛒᛖᛗᛚᛝᛟᛞ`) as ornament only.
- The **sigil** is drawn from `src-tauri/icons/app-icon.svg`.
- The five **transitions** (rune-dissolve, terminal-scroll, runic-portal, ink-ripple, angular-carve) and the **rune-scramble** effect are frame-driven ports of galdr's existing motion design.

## Fonts

Fonts are bundled via `@remotion/google-fonts` (loaded in `Root.tsx`) so renders are reproducible across machines:

- **Cascadia Mono** — primary monospace face for all text.
- **Noto Sans Runic** — automatic fallback the browser uses for Elder Futhark glyphs (the OS monospace stack may not include the Runic Unicode block).

## Audio

`public/audio/soundtrack.mp3` is a royalty-free energetic synthwave / retro-80s track sourced from Pixabay under the [Pixabay Content License](https://pixabay.com/service/license-summary/) (free for commercial and personal use, no attribution required). The track is 163s; the reel uses a 36s window starting ~6s in (into the driving section) with a 1s fade-in and a 1.5s tail fade-out over the logo close. To swap the track, replace the file and adjust `startFromFrames` / volume in `GaldrReel.tsx`. If the file is absent, Remotion renders the video silently.

## Editing the reel

All timing is in `theme.ts` (`TIMING` object, in frames at 60fps). To retime a scene, change its `durationInFrames` and the `from` of subsequent scenes. Each scene is also a standalone composition in Remotion Studio, so you can preview and tweak one in isolation without rendering the whole reel.
