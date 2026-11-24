//! Copies the FFmpeg DLLs from `FFMPEG_DIR/bin` next to the binary so it runs without PATH changes.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    let Ok(ffmpeg) = std::env::var("FFMPEG_DIR") else { return };
    let bin = PathBuf::from(ffmpeg).join("bin");
    // OUT_DIR is somewhere under target/<profile>/build/; the layout varies between Cargo versions.
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let Some(profile_dir) = out
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "build"))
        .and_then(|p| p.parent())
    else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&bin) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")) {
            let target = profile_dir.join(entry.file_name());
            let _ = std::fs::copy(&path, &target);
        }
    }
}
