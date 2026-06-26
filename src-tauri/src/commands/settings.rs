use crate::models::settings::{AppSettings, WindowState};
use crate::models::watch_folder::{WatchOutputFormat, MAX_LOG_ENTRIES};
use std::fs;
use std::path::PathBuf;

pub(crate) fn store_dir() -> PathBuf {
    let mut dir = dirs_data_dir();
    dir.push("galdr");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn dirs_data_dir() -> PathBuf {
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

/// Migrate watch-folder configs from older schema versions. Mutates in place.
/// Handles:
///   - extensions → patterns (["mp4"] → ["*.mp4"])
///   - params/output_dir → output_formats (single preset → one-element vec)
///   - settle_ms == 0 → 10000 (apply default)
fn migrate_watch_folders(settings: &mut AppSettings) {
    for folder in &mut settings.watch_folders {
        // extensions → patterns
        if folder.patterns.is_empty() && !folder.extensions.is_empty() {
            folder.patterns = folder
                .extensions
                .iter()
                .map(|e| format!("*.{}", e))
                .collect();
        }
        // legacy params → output_formats
        if folder.output_formats.is_empty() && !folder.params.output_format.is_empty() {
            folder.output_formats = vec![WatchOutputFormat {
                output_format: folder.params.output_format.clone(),
                quality: folder.params.quality,
                output_dir: folder.output_dir.clone(),
            }];
        }
        // apply default debounce if unset
        if folder.settle_ms == 0 {
            folder.settle_ms = 10000;
        }
        // bound the log to prevent unbounded growth
        if folder.processing_log.len() > MAX_LOG_ENTRIES {
            folder.processing_log.truncate(MAX_LOG_ENTRIES);
        }
    }
}

#[tauri::command]
pub fn load_settings() -> AppSettings {
    let path = store_dir().join("settings.json");
    if !path.exists() {
        return AppSettings::default();
    }
    let mut settings = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    migrate_watch_folders(&mut settings);
    settings
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    let path = store_dir().join("settings.json");
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

/// The frontend auto-saves preferences (output_dir, toggles, etc.) whenever
/// the user changes any general setting.  We load the *full* `settings.json`
/// off disk and only overwrite the UI-managed fields, so that
/// backend-only fields such as `watch_folders` and `notify_on_watch_complete`
/// are never accidentally wiped.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn save_app_preferences(
    output_dir: String,
    transition_style: String,
    crt_enabled: bool,
    show_rune_in_titlebar: bool,
    discord_enabled: bool,
    preferred_video_encoder: Option<String>,
    auto_fallback_hw: bool,
    download_dir: String,
    auto_download_subtitles: bool,
    auto_embed_subtitles: bool,
) -> Result<(), String> {
    let path = store_dir().join("settings.json");
    let mut existing = load_settings();
    existing.output_dir = output_dir;
    existing.transition_style = transition_style;
    existing.crt_enabled = crt_enabled;
    existing.show_rune_in_titlebar = show_rune_in_titlebar;
    existing.discord_enabled = discord_enabled;
    existing.preferred_video_encoder = preferred_video_encoder;
    existing.auto_fallback_hw = auto_fallback_hw;
    existing.download_dir = download_dir;
    existing.auto_download_subtitles = auto_download_subtitles;
    existing.auto_embed_subtitles = auto_embed_subtitles;
    let json = serde_json::to_string_pretty(&existing).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

/// Detect hardware encoders available in the system's ffmpeg installation.
#[tauri::command]
pub fn detect_hardware_encoders() -> Vec<crate::ffmpeg::encoders::HardwareEncoderInfo> {
    crate::ffmpeg::encoders::detect_hardware_encoders()
}

#[tauri::command]
pub fn load_window_state() -> Option<WindowState> {
    let path = store_dir().join("window-state.json");
    if !path.exists() {
        return None;
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

#[tauri::command]
pub fn save_window_state(state: WindowState) -> Result<(), String> {
    let path = store_dir().join("window-state.json");
    let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_forge_recovery(data: String) -> Result<(), String> {
    let path = store_dir().join("forge-recovery.json");
    fs::write(&path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_forge_recovery() -> Option<String> {
    let path = store_dir().join("forge-recovery.json");
    if !path.exists() {
        return None;
    }
    fs::read_to_string(&path).ok()
}

#[tauri::command]
pub fn clear_forge_recovery() -> Result<(), String> {
    let path = store_dir().join("forge-recovery.json");
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
