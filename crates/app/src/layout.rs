//! How the window is arranged: which panels show, how big they are, and the theme. Kept in
//! `%APPDATA%\galdr\layout.json` between runs.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::theme::Mode;

pub const MEDIA_W: (f32, f32) = (220.0, 520.0);
pub const INSPECTOR_W: (f32, f32) = (260.0, 520.0);
pub const TIMELINE_H: (f32, f32) = (150.0, 640.0);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    pub media: bool,
    pub inspector: bool,
    pub media_w: f32,
    pub inspector_w: f32,
    pub timeline_h: f32,
    pub theme: Mode,
}

impl Default for Layout {
    fn default() -> Self {
        Self { media: true, inspector: true, media_w: 290.0, inspector_w: 330.0, timeline_h: 250.0, theme: Mode::Light }
    }
}

fn path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("galdr").join("layout.json"))
}

impl Layout {
    pub fn load() -> Self {
        let d = Self::default();
        let Some(v) = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        else {
            return d;
        };
        let f = |k: &str, def: f32, (lo, hi): (f32, f32)| v[k].as_f64().map_or(def, |x| (x as f32).clamp(lo, hi));
        Self {
            media: v["media"].as_bool().unwrap_or(d.media),
            inspector: v["inspector"].as_bool().unwrap_or(d.inspector),
            media_w: f("media_w", d.media_w, MEDIA_W),
            inspector_w: f("inspector_w", d.inspector_w, INSPECTOR_W),
            timeline_h: f("timeline_h", d.timeline_h, TIMELINE_H),
            theme: if v["theme"] == "dark" { Mode::Dark } else { Mode::Light },
        }
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        let v = json!({
            "media": self.media,
            "inspector": self.inspector,
            "media_w": self.media_w,
            "inspector_w": self.inspector_w,
            "timeline_h": self.timeline_h,
            "theme": if self.theme == Mode::Dark { "dark" } else { "light" },
        });
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_vec_pretty(&v).unwrap_or_default());
    }
}
