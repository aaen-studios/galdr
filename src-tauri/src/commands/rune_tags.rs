use crate::models::{ConversionParams, PresetParams, RuneTag};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Strip the job-specific path fields before a rune is persisted. A preset
/// must never carry a particular file's input/output paths.
fn strip_paths(mut params: PresetParams) -> PresetParams {
    params.input_path = PathBuf::new();
    params.output_dir = PathBuf::new();
    params
}

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

/// Fields a bundled example rune may set. Any field left `None` is simply not
/// part of the preset, so applying the rune won't clobber a conversion's
/// existing value for it.
#[derive(Default)]
struct PresetSpec {
    output_format: &'static str,
    video_codec: Option<&'static str>,
    audio_codec: Option<&'static str>,
    video_bitrate: Option<&'static str>,
    audio_bitrate: Option<&'static str>,
    resolution: Option<(u32, u32)>,
    framerate: Option<f64>,
    crf: Option<u8>,
    preset: Option<&'static str>,
    quality: Option<f64>,
    trim_start: Option<f64>,
    trim_end: Option<f64>,
    speed_video: Option<f64>,
    speed_audio: Option<f64>,
    rotate: Option<u32>,
    flip: Option<&'static str>,
    sample_rate: Option<u32>,
    channels: Option<u8>,
    audio_normalize: Option<&'static str>,
    fade_in: Option<f64>,
    fade_out: Option<f64>,
}

fn preset(spec: PresetSpec) -> PresetParams {
    let mut p = ConversionParams::default();
    p.output_format = spec.output_format.into();
    p.video_codec = spec.video_codec.map(Into::into);
    p.audio_codec = spec.audio_codec.map(Into::into);
    p.video_bitrate = spec.video_bitrate.map(Into::into);
    p.audio_bitrate = spec.audio_bitrate.map(Into::into);
    p.resolution = spec.resolution;
    p.framerate = spec.framerate;
    p.crf = spec.crf;
    p.preset = spec.preset.map(Into::into);
    p.quality = spec.quality;
    p.trim_start = spec.trim_start;
    p.trim_end = spec.trim_end;
    p.speed_video = spec.speed_video;
    p.speed_audio = spec.speed_audio;
    p.rotate = spec.rotate;
    p.flip = spec.flip.map(Into::into);
    p.sample_rate = spec.sample_rate;
    p.channels = spec.channels;
    p.audio_normalize = spec.audio_normalize.map(Into::into);
    p.fade_in = spec.fade_in;
    p.fade_out = spec.fade_out;
    p
}

/// Bundled starter runes. These double as examples of what a rune can capture:
/// encode settings, audio extraction, image conversion, vertical/social video,
/// loudness normalization, fades, and more. Seeded only on a fresh install.
fn bundled_presets() -> Vec<RuneTag> {
    let mut id = 0;
    let mut mk = |name: &'static str, rune: &'static str, desc: &'static str, spec: PresetSpec| {
        id += 1;
        RuneTag {
            id: format!("starter-{}", id),
            name: name.into(),
            rune: rune.into(),
            description: desc.into(),
            params: preset(spec),
        }
    };

    vec![
        // ── Encoding presets ──
        mk(
            "Fehu",
            "ᚠ",
            "High-quality archive. H.265 CRF 18 with lossless FLAC audio in MKV.",
            PresetSpec {
                output_format: "mkv",
                video_codec: Some("libx265"),
                audio_codec: Some("flac"),
                crf: Some(18),
                preset: Some("medium"),
                ..Default::default()
            },
        ),
        mk(
            "Kaunan",
            "ᚲ",
            "Web-ready H.264 at CRF 23, AAC 128k, capped at 1080p.",
            PresetSpec {
                output_format: "mp4",
                video_codec: Some("libx264"),
                audio_codec: Some("aac"),
                audio_bitrate: Some("128k"),
                resolution: Some((1920, 1080)),
                crf: Some(23),
                preset: Some("medium"),
                ..Default::default()
            },
        ),
        mk(
            "Tiwaz",
            "ᛏ",
            "YouTube upload: H.264 CRF 21, AAC 192k, smooth 60fps.",
            PresetSpec {
                output_format: "mp4",
                video_codec: Some("libx264"),
                audio_codec: Some("aac"),
                audio_bitrate: Some("192k"),
                framerate: Some(60.0),
                crf: Some(21),
                preset: Some("medium"),
                ..Default::default()
            },
        ),
        mk(
            "Dagaz",
            "ᛞ",
            "Tiny clips for chat. H.264 CRF 28, AAC 96k, downscaled to 720p.",
            PresetSpec {
                output_format: "mp4",
                video_codec: Some("libx264"),
                audio_codec: Some("aac"),
                audio_bitrate: Some("96k"),
                resolution: Some((1280, 720)),
                crf: Some(28),
                preset: Some("fast"),
                ..Default::default()
            },
        ),
        mk(
            "Jera",
            "ᛃ",
            "Smallest watchable file. VP9 in WebM, CRF 36, opus audio.",
            PresetSpec {
                output_format: "webm",
                video_codec: Some("libvpx-vp9"),
                audio_codec: Some("libopus"),
                crf: Some(36),
                preset: Some("fast"),
                audio_bitrate: Some("96k"),
                ..Default::default()
            },
        ),
        // ── Social / vertical video ──
        mk(
            "Mannaz",
            "ᛗ",
            "Vertical 9:16 for reels & shorts. 1080x1920 H.264, AAC 128k.",
            PresetSpec {
                output_format: "mp4",
                video_codec: Some("libx264"),
                audio_codec: Some("aac"),
                audio_bitrate: Some("128k"),
                resolution: Some((1080, 1920)),
                crf: Some(22),
                preset: Some("medium"),
                ..Default::default()
            },
        ),
        mk(
            "Sowilo",
            "ᛊ",
            "Square 1:1 for feeds. 1080x1080 H.264, AAC 128k.",
            PresetSpec {
                output_format: "mp4",
                video_codec: Some("libx264"),
                audio_codec: Some("aac"),
                audio_bitrate: Some("128k"),
                resolution: Some((1080, 1080)),
                crf: Some(22),
                preset: Some("medium"),
                ..Default::default()
            },
        ),
        // ── Audio extraction & mastering ──
        mk(
            "Ansuz",
            "ᚨ",
            "Extract clean MP3 audio at 320k for music.",
            PresetSpec {
                output_format: "mp3",
                audio_codec: Some("libmp3lame"),
                audio_bitrate: Some("320k"),
                sample_rate: Some(44100),
                ..Default::default()
            },
        ),
        mk(
            "Berkano",
            "ᛒ",
            "Podcast voice: MP3 128k mono, EBU R128 loudness normalization.",
            PresetSpec {
                output_format: "mp3",
                audio_codec: Some("libmp3lame"),
                audio_bitrate: Some("128k"),
                channels: Some(1),
                sample_rate: Some(44100),
                audio_normalize: Some("loudnorm"),
                ..Default::default()
            },
        ),
        mk(
            "Laguz",
            "ᛚ",
            "Fade in 1s / out 2s on extracted FLAC — great for clips & transitions.",
            PresetSpec {
                output_format: "flac",
                audio_codec: Some("flac"),
                fade_in: Some(1.0),
                fade_out: Some(2.0),
                ..Default::default()
            },
        ),
        // ── Image conversion ──
        mk(
            "Kenaz",
            "ᛜ",
            "WebP photo export, quality 82 — small files, sharp images.",
            PresetSpec {
                output_format: "webp",
                quality: Some(0.82),
                ..Default::default()
            },
        ),
        // ── Animated GIF ──
        mk(
            "Ehwaz",
            "ᚺ",
            "Looping GIF at 15fps, optimized palette for chat reactions.",
            PresetSpec {
                output_format: "gif",
                framerate: Some(15.0),
                quality: Some(0.6),
                ..Default::default()
            },
        ),
        // ── Time effects ──
        mk(
            "Raido",
            "ᚱ",
            "2x timelapse — double video & audio speed for condensing long footage.",
            PresetSpec {
                output_format: "mp4",
                video_codec: Some("libx264"),
                audio_codec: Some("aac"),
                speed_video: Some(2.0),
                speed_audio: Some(2.0),
                crf: Some(23),
                preset: Some("medium"),
                ..Default::default()
            },
        ),
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
pub fn save_rune_tag(mut tag: RuneTag) -> Result<RuneTag, String> {
    if tag.id.is_empty() {
        tag.id = Uuid::new_v4().to_string();
    }
    tag.params = strip_paths(tag.params);
    save_tag(&tag)?;
    Ok(tag)
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