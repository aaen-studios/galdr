# ᚲ galdr

> A rune-encrusted desktop GUI around FFmpeg — convert, compress, and edit video, audio, and image files with the elegance of ancient incantations.

galdr (Old Norse for "magical incantation") frames media work as spellcasting: raw media in, enchanted media out. No command-line incantations to memorize.

---

## Features

### Conversion

- **Single-file conversion** — drag-and-drop, pick a format, tweak parameters, watch the FFmpeg command build in real time
- **Batch conversion** — point at a folder, auto-scan for media, process with skip/resume
- **Compression** — quality slider with live size estimation, before/after preview
- **Audio operations** — loudness normalization (EBU R128), dynamic normalization, fade in/out, sample rate and channel control
- **Video operations** — trim, crop (with ratio lock), rotate, flip, speed adjust (0.25x–4x)
- **Frame extraction** — extract frames from video at custom intervals
- **Image conversion** — convert between PNG, JPEG, WebP, BMP, TIFF, AVIF

### Forge (Video Editor)

- **Timeline editor** — multi-track video and audio timeline with clip splitting, trimming, and ripple delete
- **Media library** — import and browse source clips, drag to timeline
- **Undo/redo** — full history stack with keyboard shortcuts
- **Project files** — save and load `.galdr` project files
- **Crash recovery** — auto-saves project state, restores on launch
- **Export** — render timeline to MP4 or MKV with quality and resolution options

### Watch Folders

- **Auto-convert** — monitor directories and convert new files automatically using a saved preset
- **Queue for review** — detect new files and add them to a review queue
- **Extension filters** — restrict monitoring to specific file types
- **Debounced detection** — waits for files to settle before processing
- **Background operation** — keeps running while the app is minimized to tray

### Interface

- **Command Alchemy** — live FFmpeg command preview with syntax highlighting
- **Side-by-side preview** — synchronized video wipe, waveform overlay, pixel-diff comparison
- **Rune Tags** — save/load conversion presets as named "runes"
- **Custom titlebar** — undecorated window with ScrambleText logo and runic window controls
- **Page transitions** — 5 animated styles (rune dissolve, terminal scroll, runic portal, ink ripple, angular carve)
- **Context menus** — right-click on cards, navigation, and files for quick actions
- **System tray** — close-to-tray keeps conversions and watchers running in the background
- **Taskbar integration** — progress bar and completion flash in the OS taskbar

### System

- **Discord Rich Presence** — shows what you're converting on your profile
- **In-app updater** — checks GitHub releases, downloads and installs with verified signatures
- **Bundled FFmpeg** — zero configuration, portable static binaries included
- **Single instance** — second launch brings the existing window to front
- **OS autostart** — optional launch on system boot
- **File associations** — double-click `.galdr` files to open them in Forge

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | [Tauri 2](https://v2.tauri.app/) (Rust) |
| Frontend | [React 19](https://react.dev/) + TypeScript |
| Build tool | [Vite 7](https://vite.dev/) |
| State | [Zustand 5](https://zustand.docs.pmnd.rs/) |
| Animation | [Framer Motion 12](https://motion.dev/) |
| Package manager | [Bun](https://bun.sh/) |
| Media engine | [FFmpeg](https://ffmpeg.org/) (bundled static build) |

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
.\build-ffmpeg.ps1
.\build-and-deploy.ps1

# Cross-platform (Git Bash / Linux / macOS)
./deploy.sh [new_version]
```

`deploy.sh` handles: version bumping → platform-specific FFmpeg download → `bun tauri build` → artifact packaging (.exe.zip, .tar.gz, .dmg.gz) → signing → `update.json` generation with multi-platform merge support.

---

## Releasing

Step-by-step guide to build and release a new version of galdr.

### Windows (PowerShell)

```powershell
.\build-ffmpeg.ps1
.\build-and-deploy.ps1
```

### Cross-platform (Git Bash / Linux / macOS)

```bash
./deploy.sh [new_version]
```

### Steps

1. **Bump the version** in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` (or pass the version to `deploy.sh` — it handles this automatically)
2. **Download FFmpeg binaries** — `.\build-ffmpeg.ps1` fetches the static build for your platform
3. **Build the Tauri app** — `.\build-and-deploy.ps1` runs `bun tauri build` and creates the installer
4. **Sign the archive** — the script prompts for a signature via `bun tauri signer sign --private-key-path src-tauri/updater.key <archive>`
5. **Upload to GitHub** — create a release tagged `v{version}` and upload:
   - The installer archive (`.exe.zip` for Windows, `.dmg` for macOS, `.AppImage` or `.deb` for Linux)
   - The generated `update.json`
6. **Publish the release** — the in-app updater checks `releases/latest/download/update.json` automatically

### Multi-platform releases

Build on each platform separately using `deploy.sh [version]`. Each run merges its platform entry into `update.json` automatically.

---

## Project Structure

```
src/                        # Frontend (React + TypeScript)
├── components/             # UI components (dropdown, previews, sliders, etc.)
│   └── forge/              # Forge editor (Timeline, VideoPreview, SourceBrowser, etc.)
├── pages/                  # Pages (home, convert, batch, compress, forge, runes, watch, settings)
├── store/                  # Zustand stores (index, forgeStore, watchStore)
├── types/                  # TypeScript type definitions
├── utils/                  # Utility functions
├── options.ts              # Format and codec option lists
├── transitions.tsx         # Page transition animations
src-tauri/                  # Backend (Rust)
├── src/
│   ├── lib.rs              # Tauri app setup, plugin registration
│   ├── commands/           # IPC handlers (convert, forge, info, preview, rune_tags, settings, etc.)
│   ├── ffmpeg/             # FFmpeg builder, runner, probe
│   ├── discord_rpc/        # Discord Rich Presence
│   ├── models/             # Data models (settings, watch folders)
│   ├── tray.rs             # System tray icon and close-to-tray
│   └── watcher.rs          # Watch folder daemon
├── binaries/               # Downloaded FFmpeg/FFprobe static binaries
├── tauri.conf.json         # Tauri configuration
└── Cargo.toml              # Rust dependencies
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
