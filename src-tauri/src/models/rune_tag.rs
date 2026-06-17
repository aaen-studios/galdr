use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetParams {
    pub output_format: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub video_bitrate: Option<String>,
    pub audio_bitrate: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub framerate: Option<f64>,
    pub crf: Option<u8>,
    pub preset: Option<String>,
    pub quality: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuneTag {
    pub id: String,
    pub name: String,
    pub rune: String,
    pub description: String,
    pub params: PresetParams,
}
