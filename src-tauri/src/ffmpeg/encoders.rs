use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;
use std::sync::Mutex;

/// Cache for hardware encoder detection — FFmpeg is only invoked once per
/// session. Subsequent calls (e.g. when navigating to the settings page)
/// return the cached result instantly.
static ENCODER_CACHE: Lazy<Mutex<Option<Vec<HardwareEncoderInfo>>>> =
    Lazy::new(|| Mutex::new(None));

/// Cache of every encoder name ffmpeg reports (`ffmpeg -encoders`), used so
/// [`has_encoder`] doesn't spawn a subprocess on every conversion. Populated
/// lazily on first lookup. The hardware-encoder cache above is a filtered view
/// of this same output, kept separate for the settings UI.
static ENCODER_NAMES_CACHE: Lazy<Mutex<Option<HashSet<String>>>> =
    Lazy::new(|| Mutex::new(None));

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

/// Return cached hardware encoders, or detect them via FFmpeg on the first
/// call and cache the result for the remainder of the session.
pub fn detect_hardware_encoders() -> Vec<HardwareEncoderInfo> {
    if let Some(cached) = ENCODER_CACHE.lock().ok().and_then(|guard| guard.clone()) {
        return cached;
    }
    let encoders = detect_hardware_encoders_inner();
    if let Ok(mut cache) = ENCODER_CACHE.lock() {
        *cache = Some(encoders.clone());
    }
    encoders
}

/// Run `ffmpeg -hide_banner -encoders` once and return its full stdout, or an
/// empty string if ffmpeg cannot be run. Shared by hardware detection and the
/// full-name cache so the subprocess only runs when a cache is cold.
fn run_encoders_list() -> String {
    let ffmpeg = crate::ffmpeg::ffmpeg_path();
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => String::new(),
    }
}

/// Run `ffmpeg -hide_banner -encoders` and return all detected hardware
/// encoders.  Returns an empty vec if ffmpeg cannot be run or produces no
/// output.
fn detect_hardware_encoders_inner() -> Vec<HardwareEncoderInfo> {
    let stdout = run_encoders_list();
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

/// Return the set of every encoder name ffmpeg reports, running
/// `ffmpeg -encoders` once per session and caching the result. Subsequent
/// calls (e.g. every preferred-encoder conversion) hit the cache instead of
/// spawning a subprocess.
fn all_encoder_names() -> HashSet<String> {
    // Fast path: cache warm — clone and return without spawning ffmpeg.
    if let Some(cached) = ENCODER_NAMES_CACHE.lock().ok().and_then(|g| g.clone()) {
        return cached;
    }

    let stdout = run_encoders_list();
    let mut names = HashSet::new();
    for line in stdout.lines() {
        if line.len() < 9 {
            continue;
        }
        if let Some(name) = line[8..].trim().split_whitespace().next() {
            names.insert(name.to_string());
        }
    }
    if let Ok(mut cache) = ENCODER_NAMES_CACHE.lock() {
        *cache = Some(names.clone());
    }
    names
}

/// Check whether a specific encoder name is available in ffmpeg. Cached: the
/// first call runs `ffmpeg -encoders`; later calls are an in-memory lookup.
pub fn has_encoder(name: &str) -> bool {
    all_encoder_names().contains(name)
}
