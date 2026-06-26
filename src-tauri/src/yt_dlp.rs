//! yt-dlp binary resolution and on-demand installation.
//!
//! yt-dlp is not bundled with the installer (it's ~15 MB and updates far more
//! frequently than galdr releases). Instead, the first time the user needs it
//! we download the official release binary from GitHub and cache it in the
//! per-user data dir. Resolution mirrors the pattern in `whisper/mod.rs` and
//! `ffmpeg/mod.rs`: a resource_dir primary candidate, exe-relative fallbacks,
//! dev fallbacks, then PATH.

use once_cell::sync::OnceCell;
use std::path::PathBuf;
use tauri::Manager;

use crate::commands::settings::store_dir;

/// Primary candidate for the yt-dlp binary: `resource_dir()/binaries/yt-dlp(.exe)`.
/// Set once during `init_paths`. Stored unconditionally (without checking
/// `.exists()`) and re-probed lazily in `yt_dlp_path()` so a transient absence
/// at startup doesn't permanently break resolution.
static YTDLP_PRIMARY: OnceCell<PathBuf> = OnceCell::new();

/// Filename of the yt-dlp binary, with the platform extension.
fn exe_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

/// Build the ordered candidate list for the yt-dlp binary.
///
/// Probing order:
///   1. `resource_dir()/binaries/<exe>` — the cached primary from `init_paths`
///   2. `<exe_dir>/resources/binaries/<exe>` — NSIS installer layout
///   3. `<exe_dir>/binaries/<exe>` — sidecar-style layout
///   4. `<exe_dir>/<exe>` — defensive last-ditch sibling
///   5. `src-tauri/binaries/<exe>` — dev fallback (relative to CWD)
///   6. `binaries/<exe>` — dev fallback
///   7. PATH lookup via `which_ytdlp()`
fn ytdlp_candidates() -> Vec<PathBuf> {
    let exe = exe_name();
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 0. On-demand cached download — where `ensure_ytdlp` writes the binary.
    //    Highest priority so a successful install is always found.
    candidates.push(cached_binary_path().clone());

    // 1. Cached primary from init_paths (resource_dir-based).
    if let Some(primary) = YTDLP_PRIMARY.get() {
        candidates.push(primary.clone());
    }

    // 2-4. Relative to the running .exe — always reliable.
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

/// Best-effort PATH lookup for the yt-dlp binary.
fn which_ytdlp() -> Option<PathBuf> {
    let exe = exe_name();
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Store the resource_dir-based primary candidate. Mirrors `whisper::init_paths`.
pub fn init_paths(app_handle: &tauri::AppHandle) {
    let resource = app_handle
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    YTDLP_PRIMARY
        .set(resource.join("binaries").join(exe_name()))
        .ok();
}

/// Resolve the yt-dlp binary path, probing lazily at call time.
///
/// Re-checks the candidate list on every call so a transient absence during
/// `init_paths` can't permanently break downloads. Falls back to the bare
/// command name so a PATH-resolved install still works at spawn time.
pub fn yt_dlp_path() -> PathBuf {
    for candidate in ytdlp_candidates() {
        if candidate.exists() {
            return candidate.clone();
        }
    }
    // PATH lookup as a last resort before giving up on a real path.
    if let Some(resolved) = which_ytdlp() {
        return resolved;
    }
    // Bare name so a PATH-resolved install still works.
    PathBuf::from(exe_name())
}

/// True if a yt-dlp binary can be found on disk.
pub fn is_ytdlp_available() -> bool {
    yt_dlp_path().exists()
}

/// The app-managed downloads folder.
///
/// Defaults to `<data_dir>/galdr/downloads/`. Created on demand. The user can
/// override this via the `download_dir` setting; the caller is responsible for
/// checking the setting first and falling back to this.
pub fn default_downloads_dir() -> PathBuf {
    let dir = store_dir().join("downloads");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Resolve the effective downloads dir: the user's `download_dir` setting if
/// non-empty, otherwise the default. Creates the dir if it doesn't exist.
pub fn resolve_downloads_dir(user_dir: &str) -> PathBuf {
    let dir = if user_dir.trim().is_empty() {
        default_downloads_dir()
    } else {
        PathBuf::from(user_dir)
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// GitHub release download URL for the current platform's yt-dlp binary.
///
/// On Windows we grab the standalone `yt-dlp.exe`. On macOS/Linux we grab the
/// python-based `yt-dlp` (requires Python 3.8+ on the target machine).
pub fn download_url() -> &'static str {
    if cfg!(windows) {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    }
}

/// Destination path where the on-demand download is cached.
///
/// Lives in the per-user data dir (`<data_dir>/galdr/yt-dlp(.exe)`) so it
/// survives app updates and is shared across all galdr installs for the user.
pub fn cached_binary_path() -> PathBuf {
    store_dir().join(exe_name())
}
