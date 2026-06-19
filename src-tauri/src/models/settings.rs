use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub output_dir: String,
    pub transition_style: String,
    pub crt_enabled: bool,
    pub show_rune_in_titlebar: bool,
    pub discord_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            transition_style: "none".into(),
            crt_enabled: false,
            show_rune_in_titlebar: true,
            discord_enabled: true,
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
}
