Acta board: aaen-studios/galdr

# galdr

A video editor with media tools built in (transcription, conversion, compression, downloads and more).

- UI, timeline and playback: Rust with GPUI.
- Everything else (tool planners, export, job queue, ffmpeg / whisper / yt-dlp, downloads, project files): Calx, built with `calx build --staticlib` and linked into the Rust app. JSON goes in, events come back through a callback.

This branch (`main`) is a rewrite from scratch and will replace `master`. The old Tauri app and the landing site (`web/`) are on `master` until then.

## Building

- GPUI comes from `gpui-pre` (weekly snapshots of Zed's gpui, by the gpui-component maintainer), pinned exactly. The crates.io `gpui` is a year old.
- Video decoding links libav through `ffmpeg-next`. It needs a shared FFmpeg build and libclang, set in `.cargo/config.toml` (not committed):

  ```toml
  [env]
  FFMPEG_DIR = "C:/path/to/ffmpeg-n9.0-latest-win64-lgpl-shared-9.0"  # BtbN/FFmpeg-Builds
  LIBCLANG_PATH = "C:/path/to/llvm/bin"
  ```

  `build.rs` copies the FFmpeg DLLs next to the binary.
- Run GPUI windows with `--background` when testing: off-screen, unfocused and muted.
