Acta board: aaen-studios/galdr

# galdr

A video editor with media tools built in (transcription, conversion, compression, downloads and more).

- UI, timeline and playback: Rust with GPUI.
- Everything else (tool planners, export, job queue, ffmpeg / whisper / yt-dlp, downloads, project files): Calx, built with `calx build --staticlib` and linked into the Rust app. JSON goes in, events come back through a callback.

This branch (`main`) is a rewrite from scratch and will replace `master`. The old Tauri app and the landing site (`web/`) are on `master` until then.
