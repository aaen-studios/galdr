pub mod builder;
pub mod encoders;
pub mod probe;
pub mod runner;

pub use builder::*;
pub use encoders::*;
pub use probe::*;
pub use runner::*;

use once_cell::sync::OnceCell;
use std::path::PathBuf;
use tauri::Manager;

static FFMPEG_PATH: OnceCell<PathBuf> = OnceCell::new();
static FFPROBE_PATH: OnceCell<PathBuf> = OnceCell::new();

pub fn init_paths(app_handle: &tauri::AppHandle) {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .expect("failed to resolve resource dir");
    FFMPEG_PATH
        .set(resource_dir.join("binaries").join("ffmpeg.exe"))
        .ok();
    FFPROBE_PATH
        .set(resource_dir.join("binaries").join("ffprobe.exe"))
        .ok();
}

pub fn ffmpeg_path() -> PathBuf {
    for path in FFMPEG_PATH.get() {
        if path.exists() {
            return path.clone();
        }
        let no_ext = path.with_extension("");
        if no_ext.exists() {
            return no_ext;
        }
    }
    PathBuf::from("ffmpeg")
}

pub fn ffprobe_path() -> PathBuf {
    for path in FFPROBE_PATH.get() {
        if path.exists() {
            return path.clone();
        }
        let no_ext = path.with_extension("");
        if no_ext.exists() {
            return no_ext;
        }
    }
    PathBuf::from("ffprobe")
}
