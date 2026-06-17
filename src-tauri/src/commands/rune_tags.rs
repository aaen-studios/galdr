use crate::models::{PresetParams, RuneTag};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn store_dir() -> PathBuf {
    let mut dir = dirs_data_dir();
    dir.push("galdr");
    dir.push("runes");
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

fn bundled_presets() -> Vec<RuneTag> {
    vec![
        RuneTag {
            id: Uuid::new_v4().to_string(),
            name: "Fehu".into(),
            rune: "ᚠ".into(),
            description: "Archive: H.265 CRF 18, FLAC audio".into(),
            params: PresetParams {
                output_format: "mkv".into(),
                video_codec: Some("libx265".into()),
                audio_codec: Some("flac".into()),
                video_bitrate: None,
                audio_bitrate: None,
                resolution: None,
                framerate: None,
                crf: Some(18),
                preset: Some("medium".into()),
                quality: None,
            },
        },
        RuneTag {
            id: Uuid::new_v4().to_string(),
            name: "Kaunan".into(),
            rune: "ᚲ".into(),
            description: "Web: H.264 CRF 23, AAC 128k, 1080p cap".into(),
            params: PresetParams {
                output_format: "mp4".into(),
                video_codec: Some("libx264".into()),
                audio_codec: Some("aac".into()),
                video_bitrate: None,
                audio_bitrate: Some("128k".into()),
                resolution: Some((1920, 1080)),
                framerate: None,
                crf: Some(23),
                preset: Some("medium".into()),
                quality: None,
            },
        },
        RuneTag {
            id: Uuid::new_v4().to_string(),
            name: "Tiwaz".into(),
            rune: "ᛏ".into(),
            description: "YouTube: H.264 CRF 21, AAC 192k, 60fps".into(),
            params: PresetParams {
                output_format: "mp4".into(),
                video_codec: Some("libx264".into()),
                audio_codec: Some("aac".into()),
                video_bitrate: None,
                audio_bitrate: Some("192k".into()),
                resolution: None,
                framerate: Some(60.0),
                crf: Some(21),
                preset: Some("medium".into()),
                quality: None,
            },
        },
        RuneTag {
            id: Uuid::new_v4().to_string(),
            name: "Dagaz".into(),
            rune: "ᛞ".into(),
            description: "Discord: H.264 CRF 28, AAC 96k, 720p".into(),
            params: PresetParams {
                output_format: "mp4".into(),
                video_codec: Some("libx264".into()),
                audio_codec: Some("aac".into()),
                video_bitrate: None,
                audio_bitrate: Some("96k".into()),
                resolution: Some((1280, 720)),
                framerate: None,
                crf: Some(28),
                preset: Some("fast".into()),
                quality: None,
            },
        },
    ]
}

pub fn seed_defaults() {
    let dir = store_dir();
    if let Ok(entries) = fs::read_dir(&dir) {
        if entries.count() > 0 {
            return;
        }
    }
    for tag in bundled_presets() {
        let _ = save_tag(&tag);
    }
}

fn save_tag(tag: &RuneTag) -> Result<(), String> {
    let dir = store_dir();
    let path = dir.join(format!("{}.json", tag.id));
    let json = serde_json::to_string_pretty(tag).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_rune_tags() -> Result<Vec<RuneTag>, String> {
    let dir = store_dir();
    let mut tags = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let tag: RuneTag = serde_json::from_str(&content).map_err(|e| e.to_string())?;
            tags.push(tag);
        }
    }
    Ok(tags)
}

#[tauri::command]
pub fn save_rune_tag(tag: RuneTag) -> Result<RuneTag, String> {
    let mut t = tag;
    if t.id.is_empty() {
        t.id = Uuid::new_v4().to_string();
    }
    save_tag(&t)?;
    Ok(t)
}

#[tauri::command]
pub fn delete_rune_tag(id: String) -> Result<(), String> {
    let dir = store_dir();
    let path = dir.join(format!("{}.json", id));
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn apply_rune_tag(id: String) -> Result<PresetParams, String> {
    let dir = store_dir();
    let path = dir.join(format!("{}.json", id));
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let tag: RuneTag = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(tag.params)
}