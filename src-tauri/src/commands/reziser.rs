use std::path::Path;

use crate::ffmpeg::probe_file;
use crate::models::MediaInfo;

#[derive(Clone, serde::Serialize)]
pub struct CompressEstimate {
    pub original_size: u64,
    pub estimated_size: u64,
    pub can_compress: bool,
    pub media_info: MediaInfo,
}

#[tauri::command]
pub fn estimate_compress_size(
    path: String,
    quality: f64,
    output_format: String,
) -> Result<CompressEstimate, String> {
    let info = probe_file(Path::new(&path))?;
    let original_size = info.size;

    let has_video = info.streams.iter().any(|s| s.kind == "video");
    let has_audio = info.streams.iter().any(|s| s.kind == "audio");
    let duration = info.duration;

    // Get source bitrate from format-level or stream-level info
    let source_bitrate = info
        .bitrate
        .or_else(|| {
            info.streams
                .iter()
                .filter_map(|s| s.bitrate)
                .reduce(|a, b| a + b)
        })
        .unwrap_or(0);

    // Quality factor: how much of the bitrate to keep
    let quality_factor = 0.05 + quality * 0.90;

    // Resolution scaling at very low quality
    let resolution_scale = if quality < 0.05 {
        0.35
    } else if quality < 0.10 {
        0.50
    } else if quality < 0.15 {
        0.60
    } else if quality < 0.20 {
        0.75
    } else if quality < 0.25 {
        0.85
    } else {
        1.0
    };
    // Resolution affects area (width × height), so factor is squared
    let resolution_factor = resolution_scale * resolution_scale;

    // Format-specific efficiency and codec factors
    let efficiency = format_efficiency(&output_format, has_video, has_audio);
    let is_lossless = is_lossless_format(&output_format);

    let estimated_size = if original_size == 0 {
        // Can't estimate without source size
        original_size
    } else if duration > 0.0 && source_bitrate > 0 {
        // ── Duration-based estimate (video/audio with known bitrate) ──
        let source_bps = source_bitrate as f64;
        let target_bps = source_bps * quality_factor * efficiency;
        let estimated = (target_bps * duration / 8.0 * resolution_factor) as u64;
        // Don't over-promise: clamp to minimum reasonable size
        estimated.max(original_size / 100).min(original_size * 2)
    } else if let Some(vs) = info.streams.iter().find(|s| s.kind == "video") {
        // ── Image or single-image video ──
        let w = vs.width.unwrap_or(1920) as f64;
        let h = vs.height.unwrap_or(1080) as f64;
        let frame_count = if has_video && duration > 0.0 {
            (duration * vs.frame_rate.unwrap_or(30.0)).max(1.0)
        } else {
            1.0
        };

        // Source bits per pixel per frame
        let source_bpp = original_size as f64 * 8.0 / (w * h * frame_count);

        // For lossless → lossless, quality only affects compression level (modest gain)
        let actual_factor = if is_lossless {
            0.70 + quality * 0.25 // compression_level gives ~5-30% variation
        } else {
            quality_factor * efficiency
        };

        let target_bpp = (source_bpp * actual_factor).max(0.1);
        let estimated = (target_bpp * w * h * frame_count / 8.0 * resolution_factor) as u64;
        estimated.max(original_size / 100).min(original_size * 2)
    } else if has_audio {
        // ── Audio-only with no known bitrate ──
        let target_bps = audio_target_bitrate(quality, &output_format);
        let estimated = if duration > 0.0 {
            (target_bps * duration / 8.0) as u64
        } else {
            (original_size as f64 * quality_factor * efficiency) as u64
        };
        estimated.max(1024).min(original_size * 2)
    } else {
        // ── Fallback: simple ratio ──
        let ratio = (quality_factor * efficiency).clamp(0.01, 3.0);
        (original_size as f64 * ratio) as u64
    };

    // Can we actually make it smaller?
    let can_compress = estimated_size < original_size;

    Ok(CompressEstimate {
        original_size,
        estimated_size,
        can_compress,
        media_info: info,
    })
}

/// Format efficiency relative to baseline (lower = better compression)
fn format_efficiency(format: &str, has_video: bool, has_audio: bool) -> f64 {
    match format {
        // Lossy image
        "jpg" | "jpeg" => 1.0,
        "webp" => 0.65,
        "avif" => 0.45,
        // Lossless image
        "png" => 1.5,
        "tiff" => 1.8,
        "bmp" => 3.0,
        // Video
        "mp4" | "m4v" | "mov" => 0.9,
        "mkv" => 0.75,
        "webm" => 0.7,
        "avi" => 1.3,
        "flv" => 1.1,
        "ogv" => 0.8,
        "wmv" => 1.0,
        // GIF
        "gif" => {
            if has_video { 1.8 } else { 1.0 }
        }
        // Audio
        "mp3" => 1.0,
        "aac" | "m4a" => 0.9,
        "ogg" => 0.8,
        "opus" => 0.7,
        "wav" | "aiff" if has_audio => 4.0,
        "flac" => 1.8,
        "wma" => 0.9,
        _ => 1.0,
    }
}

fn is_lossless_format(format: &str) -> bool {
    matches!(format, "png" | "bmp" | "tiff" | "wav" | "aiff" | "flac")
}

fn audio_target_bitrate(quality: f64, format: &str) -> f64 {
    let base: f64 = match quality {
        q if q >= 0.95 => 320_000.0,
        q if q >= 0.85 => 256_000.0,
        q if q >= 0.70 => 192_000.0,
        q if q >= 0.50 => 128_000.0,
        q if q >= 0.30 => 96_000.0,
        q if q >= 0.15 => 64_000.0,
        q if q >= 0.05 => 32_000.0,
        q if q >= 0.02 => 16_000.0,
        _ => 8_000.0,
    };
    let fmt_factor: f64 = match format {
        "opus" => 0.6,
        "ogg" => 0.8,
        "aac" | "m4a" => 0.9,
        "wma" => 0.9,
        "flac" | "wav" | "aiff" => 2.0,
        _ => 1.0,
    };
    (base * fmt_factor).max(16_000.0)
}
