//! Icons and fonts, built into the binary.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/icons/", $name, ".svg")))),*]
    };
}

const ICONS: &[(&str, &[u8])] = icons![
    "check", "compress", "convert", "download", "export", "focus", "folder", "frame-back", "frame-forward", "mark",
    "minus", "panel-left", "panel-right", "pause", "play", "plus", "redo", "restore", "scissors", "select", "settings",
    "split", "square", "to-end", "to-start", "transcribe", "trash", "undo", "upload", "x",
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let name = path.strip_prefix("icons/").and_then(|p| p.strip_suffix(".svg"));
        Ok(ICONS.iter().find(|(n, _)| Some(*n) == name).map(|(_, b)| Cow::Borrowed(*b)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(if path.trim_end_matches('/') == "icons" {
            ICONS.iter().map(|(n, _)| format!("icons/{n}.svg").into()).collect()
        } else {
            Vec::new()
        })
    }
}

/// The asset path of an icon.
pub fn icon(name: &'static str) -> SharedString {
    format!("icons/{name}.svg").into()
}

pub const FONT: &str = "Geist";
pub const MONO: &str = "Geist Mono";

pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Medium.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-SemiBold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/GeistMono-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/GeistMono-Medium.ttf")),
    ]
}
