pub mod models;
pub mod runner;

pub use models::*;
pub use runner::*;

use once_cell::sync::OnceCell;
use std::path::PathBuf;
use tauri::Manager;

/// Primary candidate for the whisper-cli binary: `resource_dir()/binaries/whisper-cli(.exe)`.
/// Set once during `init_paths`. We store it unconditionally (without checking
/// `.exists()`) and re-probe lazily in `whisper_path()` so a transient absence
/// at startup doesn't permanently break resolution.
static WHISPER_PRIMARY: OnceCell<PathBuf> = OnceCell::new();

/// Per-user directory for downloaded whisper models.
/// Lives alongside `settings.json` in `%APPDATA%/galdr/models/` so models
/// survive app updates and aren't bundled into the installer.
static MODELS_DIR: OnceCell<PathBuf> = OnceCell::new();

/// Filename of the bundled whisper-cli binary, with the platform extension.
fn exe_name() -> &'static str {
    if cfg!(windows) {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    }
}

/// Build the ordered candidate list for the whisper-cli binary.
///
/// Probing order:
///   1. `resource_dir()/binaries/<exe>` — the cached primary from `init_paths`
///   2. `<exe_dir>/resources/binaries/<exe>` — derived from `current_exe()`;
///      this is exactly where the NSIS installer writes the file
///      (`$INSTDIR\resources\binaries\`) and survives cases where
///      `resource_dir()` misbehaves under updater-launched installs
///   3. `<exe_dir>/binaries/<exe>` — sidecar-style layout
///   4. `<exe_dir>/<exe>` — defensive last-ditch sibling
///   5. `src-tauri/binaries/<exe>` — dev fallback (relative to CWD)
///   6. `binaries/<exe>` — dev fallback
///   7. PATH lookup via `which_whisper()`
fn whisper_candidates() -> Vec<PathBuf> {
    let exe = exe_name();
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. Cached primary from init_paths (resource_dir-based).
    if let Some(primary) = WHISPER_PRIMARY.get() {
        candidates.push(primary.clone());
    }

    // 2-4. Relative to the running .exe — always reliable, regardless of
    // what resource_dir() resolved to.
    if let Ok(run_exe) = std::env::current_exe() {
        if let Some(parent) = run_exe.parent() {
            candidates.push(parent.join("resources").join("binaries").join(exe));
            candidates.push(parent.join("binaries").join(exe));
            candidates.push(parent.join(exe));
        }
    }

    // 5-6. Dev fallbacks relative to CWD.
    candidates.push(PathBuf::from("src-tauri").join("binaries").join(exe));
    candidates.push(PathBuf::from("binaries").join(exe));

    candidates
}

/// Best-effort PATH lookup for the whisper-cli binary.
fn which_whisper() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" };
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub fn init_paths(app_handle: &tauri::AppHandle) {
    // Store the resource_dir-based primary unconditionally — we don't gate on
    // `.exists()` here. If the file isn't present at this instant (or
    // resource_dir is temporarily wrong), `whisper_path()` re-probes a richer
    // candidate set lazily at call time. This mirrors ffmpeg::init_paths.
    let resource = app_handle
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    WHISPER_PRIMARY
        .set(resource.join("binaries").join(exe_name()))
        .ok();

    // Models dir mirrors the settings storage location (see settings.rs).
    let mut dir = data_dir();
    dir.push("galdr");
    dir.push("models");
    let _ = std::fs::create_dir_all(&dir);
    MODELS_DIR.set(dir).ok();
}

fn data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("USERPROFILE").unwrap_or_default();
                PathBuf::from(home).join("AppData").join("Roaming")
            })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".config")
    }
}

/// Resolve the whisper-cli binary path, probing lazily at call time.
///
/// Unlike the previous once-at-startup resolution, this re-checks the
/// candidate list on every call so a transient absence during `init_paths`
/// (or a flaky `resource_dir()`) can't permanently break transcription.
/// Falls back to the bare command name so a PATH-resolved install still works
/// at spawn time.
pub fn whisper_path() -> PathBuf {
    for candidate in whisper_candidates() {
        if candidate.exists() {
            return candidate;
        }
    }
    // PATH lookup as a last resort before giving up on a real path.
    if let Some(resolved) = which_whisper() {
        return resolved;
    }
    // Bare name so a PATH-resolved install still works; includes the .exe on
    // Windows so .exists() checks in detect_whisper() can match it.
    PathBuf::from(exe_name())
}

/// Directory holding installed whisper ggml model files.
pub fn models_dir() -> PathBuf {
    MODELS_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| data_dir().join("galdr").join("models"))
}
