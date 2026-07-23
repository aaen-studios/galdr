# ᚲ galdr

> A rune-encrusted desktop GUI around FFmpeg — convert, compress, transcribe, and edit video, audio, and image files with the elegance of ancient incantations.

galdr (Old Norse for "magical incantation") frames media work as spellcasting: raw media in, enchanted media out. No command-line incantations to memorize.

---

## Features

### Conversion

- **Single-file conversion** — drag-and-drop, pick a format, tweak parameters, watch the FFmpeg command build in real time
- **Batch conversion** — point at a folder, auto-scan for media, process with skip/resume
- **Compression** — quality slider with live size estimation, target-size two-pass encoding, before/after preview
- **Concatenation** — join multiple video clips losslessly using FFmpeg's concat demuxer (stream copy, no re-encode)
- **Audio extraction** — extract audio tracks to MP3, AAC, OGG, Opus, FLAC, or WAV
- **Two-pass encoding** — encode to a specific target file size with bitrate analysis
- **Video operations** — trim, crop (with ratio lock), rotate, flip, speed adjust (0.25x–4x)
- **Audio operations** — loudness normalization (EBU R128), dynamic normalization, fade in/out, sample rate and channel control
- **Frame extraction** — extract frames from video at custom timestamps
- **Image conversion** — convert between PNG, JPEG, WebP, BMP, TIFF, AVIF

#### Supported Formats

| Category | Formats |
|----------|---------|
| Video | MP4, MKV, AVI, MOV, WebM, M4V, FLV, OGV, WMV, TS, 3GP, MOD, GIF |
| Audio | MP3, FLAC, WAV, AAC, OGG, Opus, WMA, M4A, AIFF, AC3 |
| Image | PNG, JPEG, WebP, BMP, TIFF, AVIF |

### Subtitles

- **AI transcription** — powered by [whisper.cpp](https://github.com/ggerganov/whisper.cpp) with 9 downloadable models (multilingual and English-only, from tiny 75 MB to large-v3 3 GB)
- **Language detection** — auto-detect spoken language from audio
- **Translate to English** — transcribe in any language and output English translations
- **Output formats** — SRT, VTT, JSON, or all at once
- **Subtitle burn-in** — render subtitles directly onto video frames with full ASS `force_style` support (font, size, color, outline, alignment, bold, background)
- **Burn-in preview** — generate a single-frame PNG preview showing how burned subtitles will look
- **Subtitle embed** — soft-embed subtitle tracks into containers (auto-selects codec: `mov_text` for MP4, `webvtt` for WebM, `copy` for MKV)
- **Subtitle extraction** — pull subtitle streams from existing videos to SRT/VTT/ASS
- **Format conversion** — convert between SRT, VTT, and ASS formats
- **In-app transcript editor** — edit SRT/VTT cues with auto-save and crash recovery

### Forge (Video Editor)

- **Timeline editor** — multi-track video and audio timeline with clip splitting, trimming, and reordering
- **Per-clip speed control** — adjust speed from 0.25x to 4.0x on individual clips
- **Media library** — import and browse source clips
- **Undo/redo** — full history stack
- **Pre-render preview** — quick low-quality render for timeline playback
- **Project files** — save and load `.galdr` project files
- **Crash recovery** — auto-saves project state (debounced), restores on next launch
- **Export** — render timeline to MP4 or MKV with quality (high/medium/fast) and resolution (source/1080p/720p) options

### Watch Folders

- **Auto-convert** — monitor directories and convert new files automatically using a saved preset
- **Queue for review** — detect new files and add them to a manual review queue
- **Extension filters** — restrict monitoring to specific file types
- **Debounced detection** — waits for files to settle before processing
- **Per-folder serialization** — prevents concurrent FFmpeg processes within a single folder
- **Background operation** — keeps running while the app is minimized to tray

### Import (yt-dlp)

- **URL download** — fetch video, audio, or subtitles from any yt-dlp-supported URL
- **Auto-installs yt-dlp** — the binary is downloaded from GitHub (~15 MB) on first use and cached on your machine
- **Metadata fetch** — preview title, duration, and available formats before downloading
- **Playlist support** — download individual videos or entire playlists with item selection
- **Quality & format selection** — pick resolution, container, and audio codec per download
- **Subtitle options** — automatically download available subtitles and optionally embed them into the output file (configurable in Settings)

### Background Queue

- **Unified job tracking** — all operations funnel into one queue: conversions, batch conversions, transcriptions, subtitle operations, concatenations, audio extractions, forge exports, and yt-dlp downloads
- **Scoped cancellation** — cancel individual jobs by PID without killing others
- **Real-time progress** — live progress events, titlebar progress strip, and Windows taskbar integration
- **Completion flash** — taskbar attention request when a job finishes
- **Queue dropdown** — titlebar indicator showing active job count with quick access

### Rune Tags (Presets)

- **Save/load presets** — capture all conversion parameters (minus file paths) as reusable "runes"
- **Import/export** — share rune collections as `.galdr` files
- **12 bundled seed runes** — named after Elder Futhark runes (Fehu, Kaunan, Tiwaz, Dagaz, Jera, Mannaz, Sowilo, Ansuz, Berkano, Laguz, Kenaz, Ehwaz, Raido) covering archive-quality H.265, web-ready H.264, YouTube upload, tiny clips, vertical 9:16, MP3 extraction, podcast voice, animated GIF, and more

### Interface

- **Command Alchemy** — live FFmpeg command preview with syntax highlighting
- **Side-by-side preview** — synchronized video wipe, waveform overlay, and pixel-diff comparison
- **Custom titlebar** — undecorated window with ScrambleText logo and runic window controls
- **ScrambleText** — headings and labels animate through random Elder Futhark runes before revealing text
- **Page transitions** — 5 animated styles (rune dissolve, terminal scroll, runic portal, ink ripple, angular carve)
- **Context menus** — right-click on cards, navigation, and files for quick actions
- **Media preview** — in-app video/image playback
- **Log panel** — real-time FFmpeg output

### System

- **System tray** — close-to-tray keeps conversions and watchers running; dynamic tooltip reflects watcher status
- **Taskbar integration** — progress bar and completion flash in the OS taskbar
- **Discord Rich Presence** — shows what you're converting on your profile (with progress %, model info, and per-page labels)
- **In-app updater** — checks GitHub releases, downloads and installs with verified signatures
- **Bundled FFmpeg** — zero configuration, portable static binaries included
- **Bundled whisper-cli** — AI transcription binary bundled alongside FFmpeg
- **Single instance** — second launch brings the existing window to front and forwards `.galdr` file arguments
- **OS autostart** — optional launch on system boot
- **File associations** — double-click `.galdr` files to open them in Forge (projects) or Runes (preset collections)
- **Window state persistence** — remembers position, size, and maximized state across restarts

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | [Tauri 2](https://v2.tauri.app/) (Rust) |
| Frontend | [React 19](https://react.dev/) + TypeScript 5.8 |
| Build tool | [Vite 7](https://vite.dev/) |
| State | [Zustand 5](https://zustand.docs.pmnd.rs/) |
| Animation | [Framer Motion 12](https://motion.dev/) |
| Package manager | [Bun](https://bun.sh/) |
| Media engine | [FFmpeg](https://ffmpeg.org/) (bundled static build) |
| Transcription | [whisper.cpp](https://github.com/ggerganov/whisper.cpp) (bundled) |
| Filesystem watching | [notify](https://github.com/notify-rs/notify) |
| Discord RPC | [discord-rich-presence](https://github.com/EmbarkStudios/discord-rich-presence-rs) |

**Tauri plugins:** dialog, single-instance, updater, autostart, opener.

---

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) (≥ 1.x)
- [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/)

### Development

```bash
bun install
bun tauri dev
```

This starts a Vite dev server on port 1420 and opens the Tauri window.

### Building

```bash
# Windows (PowerShell)
.\build-binaries.ps1
.\build-and-deploy.ps1

# Cross-platform (Git Bash / Linux / macOS)
./deploy.sh [new_version]
```

`deploy.sh` handles: version bumping → platform-specific FFmpeg download → `bun tauri build` → artifact packaging (.exe.zip, .tar.gz, .dmg.gz) → signing → `update.json` generation with multi-platform merge support.

---

## Releasing

### Windows (PowerShell)

```powershell
.\build-binaries.ps1
.\build-and-deploy.ps1
```

### Cross-platform (Git Bash / Linux / macOS)

```bash
./deploy.sh [new_version]
```

### Steps

1. **Bump the version** in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` (or pass the version to `deploy.sh` — it handles this automatically)
2. **Download binaries** — `.\build-binaries.ps1` fetches the static FFmpeg and whisper-cli builds for your platform into `src-tauri/binaries/`
3. **Build the Tauri app** — `.\build-and-deploy.ps1` runs `bun tauri build` and creates the installer
4. **Sign the archive** — the script prompts for a signature via `bun tauri signer sign --private-key-path src-tauri/updater.key <archive>`
5. **Upload to GitHub** — create a release tagged `v{version}` and upload:
   - The installer archive (`.exe.zip` for Windows, `.dmg` for macOS, `.AppImage` or `.deb` for Linux)
   - The generated `update.json`
6. **Publish the release** — the in-app updater checks `releases/latest/download/update.json` automatically

### Multi-platform releases

Build on each platform separately using `deploy.sh [version]`. Each run merges its platform entry into `update.json` automatically.

---

## Documentation

| Document | Purpose |
|----------|---------|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Complete system reference — tech stack, module layout, IPC commands, event system, queue architecture, data persistence, known issues |
| [`docs/AGENTS.md`](docs/AGENTS.md) | Rules for AI agents — mandatory reading, documentation update policy, code conventions, forbidden patterns |
| [`docs/STYLE_GUIDE.md`](docs/STYLE_GUIDE.md) | Visual design system — colors, typography, spacing, component patterns |
| [`docs/PLAN.md`](docs/PLAN.md) | Original build plan and feature roadmap |

**For AI agents:** Always read `ARCHITECTURE.md` before making changes, and update it after any structural change. See `AGENTS.md` for the full rules.

---

## Project Structure

```
src/                             # Frontend (React + TypeScript)
├── components/                  # UI components
│   ├── forge/                   #   Forge editor (Timeline, VideoPreview, SourceBrowser, PropertiesPanel, ConfirmDialog)
│   ├── whisper/                  #   Whisper model manager
│   ├── AudioComparison.tsx      #   Side-by-side waveform overlay
│   ├── CommandPreview.tsx       #   Live FFmpeg command preview with syntax highlighting
│   ├── CompressionControls.tsx  #   Quality slider and target-size controls
│   ├── ConvertOperations.tsx    #   Trim, crop, rotate, speed controls
│   ├── ExtractFramesPanel.tsx   #   Frame extraction at custom timestamps
│   ├── ImageComparison.tsx       #   Side-by-side pixel-diff comparison
│   ├── LogPanel.tsx             #   Real-time FFmpeg output
│   ├── MediaInfoCard.tsx        #   Media file metadata display
│   ├── MediaPreview.tsx         #   In-app video/image playback
│   ├── PresetPicker.tsx         #   Rune tag preset selector
│   ├── QueueDropdown.tsx        #   Background queue indicator in titlebar
│   ├── RuneTagEditor.tsx        #   Create/edit rune presets
│   ├── SubtitleStylePanel.tsx   #   ASS subtitle styling controls
│   ├── TranscriptEditor.tsx     #   SRT/VTT cue editor with auto-save
│   ├── VideoComparison.tsx      #   Side-by-side video wipe comparison
│   └── ...
├── pages/                       # Application pages
│   ├── HomePage.tsx             #   Tool selection cards
│   ├── ConvertPage.tsx          #   Single/batch file conversion
│   ├── CompressPage.tsx         #   Compression with live size estimation
│   ├── ForgePage.tsx            #   Timeline video editor
│   ├── ImportPage.tsx           #   yt-dlp URL import (metadata, quality/format selection)
│   ├── RunesPage.tsx             #   Saved preset management
│   ├── SubtitlesPage.tsx         #   Transcription, burn-in, embed, extract, convert
│   ├── WatchFoldersPage.tsx      #   Watch folder configuration
│   └── SettingsPage.tsx         #   App settings (output dir, transitions, Discord, autostart, downloads, subtitles)
├── store/                       # Zustand state management
│   ├── index.ts                 #   Main store (media info, conversion, settings, updates)
│   ├── downloadStore.ts         #   yt-dlp download state (quality/format options, progress events)
│   ├── forgeStore.ts            #   Forge editor state (timeline, clips, undo/redo)
│   ├── queueStore.ts            #   Background job queue state
│   ├── watchStore.ts            #   Watch folder state
│   └── subtitleStore.ts         #   Subtitle editor state
├── types/                       # TypeScript type definitions
├── utils/                       # Utility functions
│   ├── ffmpegBuilder.ts         #   Frontend FFmpeg command string builder
│   ├── ffmpegSyntax.ts          #   FFmpeg syntax highlighting
│   ├── runeMerge.ts            #   Merge/import rune presets
│   └── srt.ts                  #   SRT/VTT parsing utilities
├── options.ts                   # Format and codec option lists
├── options/languages.ts         # Whisper language options
└── transitions.tsx              # Page transition animation definitions

src-tauri/                       # Backend (Rust)
├── src/
│   ├── lib.rs                   #   Tauri app setup, plugin registration, event handlers
│   ├── main.rs                  #   Rust entry point
│   ├── commands/                 #   IPC command handlers
│   │   ├── convert.rs           #     Conversion, batch, concat, audio extract, compress estimate
│   │   ├── download.rs          #     yt-dlp URL import: metadata, download, list/delete, cancel
│   │   ├── forge.rs             #     Timeline export, pre-render, project file I/O
│   │   ├── info.rs              #     Media info via ffprobe
│   │   ├── preview.rs           #     Frame extraction, image data URL generation
│   │   ├── queue.rs             #     Background job queue management
│   │   ├── reziser.rs           #     Compression size estimation heuristics
│   │   ├── rune_tags.rs         #     Rune tag CRUD
│   │   ├── settings.rs          #     Settings persistence, window state, recovery
│   │   ├── subtitles.rs         #     Whisper transcription, embed/extract/convert subtitles
│   │   └── watch_folder.rs      #     Watch folder CRUD + queue operations
│   ├── ffmpeg/                   #   FFmpeg integration layer
│   │   ├── builder.rs           #     FFmpeg argument builder (full command construction)
│   │   ├── runner.rs            #     FFmpeg process runner with progress/event streaming
│   │   └── probe.rs             #     ffprobe wrapper for media info
│   ├── models/                   #   Rust data models
│   │   ├── conversion.rs
│   │   ├── job.rs               #     Background queue job model
│   │   ├── media_info.rs
│   │   ├── rune_tag.rs
│   │   ├── settings.rs          #     AppSettings + WindowState
│   │   ├── subtitle.rs
│   │   └── watch_folder.rs
│   ├── whisper/                  #   Whisper.cpp integration
│   │   ├── mod.rs               #     Binary path resolution, audio preparation
│   │   ├── models.rs            #     Model catalog (9 models, sizes, tiers)
│   │   └── runner.rs            #     Whisper process runner with streaming events
│   ├── queue/                    #   Background job queue engine
│   │   ├── mod.rs               #     Queue core (register, complete, fail, cancel)
│   │   └── pids.rs              #     Per-job PID tracking for scoped cancellation
│   ├── discord_rpc/              #   Discord Rich Presence integration
│   ├── tray.rs                   #   System tray with close-to-tray
│   └── watcher.rs                #   File system watcher daemon (notify crate)
├── binaries/                    #   Bundled FFmpeg, FFprobe, and whisper-cli static binaries
├── icons/                        #   App icons (various sizes)
├── windows/                      #   Windows installer customization (NSIS)
├── tauri.conf.json               #   Tauri configuration
└── Cargo.toml                    #   Rust dependencies
```

---

## Platforms

| Platform | Installer | Updater |
|----------|-----------|---------|
| Windows x86_64 | NSIS (.exe) | ✅ `.exe.zip` |
| macOS (Intel / Apple Silicon) | DMG | ✅ |
| Linux (x86_64 / aarch64) | AppImage / .deb | ✅ |

Mobile (Android/iOS) infrastructure exists but is not yet functional.

---

## Licensing

This project incorporates static FFmpeg binaries distributed under their respective licenses. The application source code is available for reference.
