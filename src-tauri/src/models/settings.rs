use serde::{Deserialize, Serialize};

use crate::models::watch_folder::WatchFolderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub output_dir: String,
    pub transition_style: String,
    pub crt_enabled: bool,
    pub show_rune_in_titlebar: bool,
    pub discord_enabled: bool,
    /// Watch-folder configs. Each entry is monitored by the watcher daemon.
    #[serde(default)]
    pub watch_folders: Vec<WatchFolderConfig>,
    /// Fire an OS toast when a watched-file conversion finishes.
    #[serde(default)]
    pub notify_on_watch_complete: bool,
    /// Preferred video encoder to use when none is explicitly chosen.
    /// "auto" = prefer hardware, fall back to software.
    /// "software" = always use software encoding.
    /// Any other value is treated as an encoder name (e.g. "h264_nvenc").
    #[serde(default)]
    pub preferred_video_encoder: Option<String>,
    /// When the preferred hardware encoder is unavailable, fall back to
    /// the default software encoder instead of failing.
    #[serde(default = "default_true")]
    pub auto_fallback_hw: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            transition_style: "none".into(),
            crt_enabled: false,
            show_rune_in_titlebar: true,
            discord_enabled: true,
            watch_folders: Vec::new(),
            notify_on_watch_complete: true,
            preferred_video_encoder: None,
            auto_fallback_hw: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    /// True when the window was hidden (close-to-tray) at shutdown. The next
    /// launch boots straight to the tray instead of showing the window.
    /// Cleared to false on an explicit Quit from the tray menu.
    #[serde(default)]
    pub start_hidden: bool,
}

impl WindowState {
    pub const DEFAULT_WIDTH: u32 = 1100;
    pub const DEFAULT_HEIGHT: u32 = 750;
    pub const MIN_WIDTH: u32 = 900;
    pub const MIN_HEIGHT: u32 = 600;

    /// Validate and fix the saved window state against available monitors.
    ///
    /// * If width or height is below the minimum (or zero/corrupted), the entire
    ///   state is reset to defaults (size + centered on primary monitor).
    /// * If the position is off-screen (no overlap with any monitor), the window
    ///   is centered on the primary monitor while preserving the validated size.
    /// * `primary` — the primary monitor, if known; used as the centering anchor.
    pub fn sanitize(
        self,
        monitors: &[tauri::Monitor],
        primary: Option<&tauri::Monitor>,
    ) -> Self {
        // ——— Size validation ———
        let size_bad = self.width < Self::MIN_WIDTH
            || self.height < Self::MIN_HEIGHT
            || self.width == 0
            || self.height == 0;

        if size_bad {
            // Corrupted / too-small state: reset everything to defaults.
            return center_on_monitor(primary.or_else(|| monitors.first()), Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT);
        }

        // ——— Position validation ———
        let x = self.x;
        let y = self.y;
        let w = self.width as i32;
        let h = self.height as i32;

        let on_screen = monitors.iter().any(|m| {
            let p = m.position();
            let s = m.size();
            let mx1 = p.x;
            let my1 = p.y;
            let mx2 = mx1.saturating_add(s.width as i32);
            let my2 = my1.saturating_add(s.height as i32);

            // Standard AABB overlap: window rect vs monitor rect.
            x < mx2 && x.saturating_add(w) > mx1 && y < my2 && y.saturating_add(h) > my1
        });

        if on_screen {
            self
        } else {
            center_on_monitor(primary.or_else(|| monitors.first()), self.width, self.height)
        }
    }
}

/// Return a `WindowState` centered on the given monitor (or at (0,0)
/// if no monitor is available).
fn center_on_monitor(
    monitor: Option<&tauri::Monitor>,
    width: u32,
    height: u32,
) -> WindowState {
    if let Some(m) = monitor {
        let p = m.position();
        let s = m.size();
        let cx = p.x + (s.width as i32) / 2;
        let cy = p.y + (s.height as i32) / 2;
        WindowState {
            x: cx - (width as i32) / 2,
            y: cy - (height as i32) / 2,
            width,
            height,
            maximized: false,
        }
    } else {
        WindowState {
            x: 0,
            y: 0,
            width,
            height,
            maximized: false,
        }
    }
}
