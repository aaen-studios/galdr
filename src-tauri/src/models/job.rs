use serde::{Deserialize, Serialize};

/// The type of background operation a queue entry represents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Conversion,
    BatchConversion,
    Transcription,
    SubtitleEmbed,
    SubtitleExtract,
    SubtitleBurn,
    Concatenation,
    AudioExtraction,
    ForgeExport,
}

impl std::fmt::Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobType::Conversion => write!(f, "conversion"),
            JobType::BatchConversion => write!(f, "batch_conversion"),
            JobType::Transcription => write!(f, "transcription"),
            JobType::SubtitleEmbed => write!(f, "subtitle_embed"),
            JobType::SubtitleExtract => write!(f, "subtitle_extract"),
            JobType::SubtitleBurn => write!(f, "subtitle_burn"),
            JobType::Concatenation => write!(f, "concatenation"),
            JobType::AudioExtraction => write!(f, "audio_extraction"),
            JobType::ForgeExport => write!(f, "forge_export"),
        }
    }
}

/// Current status of a queue job. The error text for `Failed` lives in the
/// `JobEntry::error` field so this enum stays a plain string when serialized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A single entry in the background job queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEntry {
    pub id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    /// 0.0 – 1.0
    pub progress: f64,
    /// Human-readable label (e.g. "Converting video.mp4")
    pub label: String,
    pub input_path: String,
    pub output_path: Option<String>,
    pub error: Option<String>,
    /// ISO 8601
    pub created_at: String,
    pub completed_at: Option<String>,
    /// Flexible result payload (e.g. output path list for batch jobs)
    pub result_data: Option<serde_json::Value>,
}
