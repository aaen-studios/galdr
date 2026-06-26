use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareEncoderInfo {
    pub name: String,
    pub codec: String,
    pub vendor: String,
    pub available: bool,
    pub description: String,
}

/// Known hardware encoder suffixes and their vendors.
const HW_PATTERNS: &[(&str, &str)] = &[
    ("nvenc", "nvidia"),
    ("amf", "amd"),
    ("qsv", "intel"),
    ("videotoolbox", "apple"),
    ("vaapi", "vaapi"),
];

/// Run `ffmpeg -hide_banner -encoders` and return all detected hardware
/// encoders.  Returns an empty vec if ffmpeg cannot be run or produces no
/// output.
pub fn detect_hardware_encoders() -> Vec<HardwareEncoderInfo> {
    let ffmpeg = crate::ffmpeg::ffmpeg_path();
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut encoders: Vec<HardwareEncoderInfo> = Vec::new();

    for line in stdout.lines() {
        // ffmpeg -encoders lines look like:
        //  V....D h264_nvenc           NVIDIA NVENC H.264 encoder (codec h264)
        // Skip the leading 8 flag characters
        if line.len() < 9 {
            continue;
        }
        let rest = line[8..].trim();

        for (suffix, vendor) in HW_PATTERNS {
            if !rest.contains(suffix) {
                continue;
            }

            // Extract the encoder name (first whitespace-delimited token)
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }

            // Determine the video codec family from the encoder name
            let codec = if name.contains("hevc") || name.contains("h265") {
                "hevc".to_string()
            } else if name.contains("av1") {
                "av1".to_string()
            } else {
                // Default to h264 for everything else (h264_nvenc, h264_amf, etc.)
                "h264".to_string()
            };

            // Clean up description: remove leading encoder name and trailing
            // "(codec xxx)" info so the user sees only the human-readable part,
            // e.g. "NVIDIA NVENC H.264 encoder".
            let desc_trimmed = if rest.starts_with(&name) {
                rest[name.len()..].trim()
            } else {
                rest
            };
            let description = match desc_trimmed.rsplit_once("(codec ") {
                Some((before, _)) => before.trim().to_string(),
                None => desc_trimmed.to_string(),
            };

            encoders.push(HardwareEncoderInfo {
                name,
                codec,
                vendor: (*vendor).to_string(),
                available: true,
                description,
            });
            break; // matched one pattern, don't check others for this line
        }
    }

    encoders
}

/// Quickly check whether a specific encoder name is available in ffmpeg.
/// This re-runs `ffmpeg -encoders` and greps for the name — it is not cached,
/// so call sparingly (or use `detect_hardware_encoders` once and cache).
pub fn has_encoder(name: &str) -> bool {
    let ffmpeg = crate::ffmpeg::ffmpeg_path();
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().any(|line| {
        if line.len() < 9 {
            return false;
        }
        line[8..].trim().split_whitespace().next() == Some(name)
    })
}
