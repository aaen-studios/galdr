pub mod pids;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use once_cell::sync::Lazy;
use tauri::Emitter;
use uuid::Uuid;

use crate::models::{JobEntry, JobStatus, JobType};

// Re-export the cancel helpers so callers can reach them as
// `crate::queue::cancel_token::{acquire, set}` — a friendlier name than the
// `pids` module they live alongside. `is_cancelled` / `unregister` are used
// directly via `crate::queue::pids::` by callers that need them.
pub mod cancel_token {
    pub use super::pids::{acquire_token as acquire, set};
}

// ── Global queue state ──

static QUEUE: Lazy<Mutex<Vec<JobEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));
static MAX_COMPLETED: AtomicUsize = AtomicUsize::new(50);

// ── Event payload ──

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueUpdatePayload {
    pub jobs: Vec<JobEntry>,
}

// ── Helpers ──

/// Emit the full queue state on the `queue-update` channel.
fn emit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let queue = QUEUE.lock().unwrap();
    let payload = QueueUpdatePayload {
        jobs: queue.clone(),
    };
    let _ = app.emit("queue-update", payload);
}

/// Build a human-friendly label from a file path and job type.
pub(crate) fn make_label(job_type: &JobType, input_path: &str) -> String {
    let file_name = std::path::Path::new(input_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    match job_type {
        JobType::Conversion | JobType::SubtitleBurn => {
            format!("Converting {}", file_name)
        }
        JobType::BatchConversion => format!("Batch: {} files", file_name),
        JobType::Transcription => format!("Transcribing {}", file_name),
        JobType::SubtitleEmbed => format!("Embedding subs into {}", file_name),
        JobType::SubtitleExtract => format!("Extracting subs from {}", file_name),
        JobType::Concatenation => format!("Concatenating {} clips", file_name),
        JobType::AudioExtraction => format!("Extracting audio from {}", file_name),
        JobType::ForgeExport => "Exporting Forge timeline".to_string(),
        JobType::Download => format!("Downloading {}", file_name),
    }
}

// ── Public API ──

/// Register a new job in the queue and emit the update.
pub fn register<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    job_type: JobType,
    label: String,
    input_path: String,
    result_data: Option<serde_json::Value>,
) -> String {
    let id = Uuid::new_v4().to_string();
    let entry = JobEntry {
        id: id.clone(),
        job_type,
        status: JobStatus::Running,
        progress: 0.0,
        label,
        input_path,
        output_path: None,
        error: None,
        created_at: Utc::now().to_rfc3339(),
        completed_at: None,
        result_data,
    };

    {
        let mut queue = QUEUE.lock().unwrap();
        queue.push(entry);
        trim_queue(&mut queue);
    }

    emit(app);
    id
}

/// Update the progress of a job (0.0 – 1.0) and emit the update.
pub fn update_progress<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str, progress: f64) {
    let mut queue = QUEUE.lock().unwrap();
    if let Some(job) = queue.iter_mut().find(|j| j.id == id) {
        job.progress = progress;
    }
    drop(queue);
    emit(app);
}

/// Mark a job as completed with an optional output path and result data.
pub fn complete<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
    output_path: Option<String>,
    result_data: Option<serde_json::Value>,
) {
    let mut queue = QUEUE.lock().unwrap();
    if let Some(job) = queue.iter_mut().find(|j| j.id == id) {
        job.status = JobStatus::Completed;
        job.progress = 1.0;
        job.output_path = output_path;
        job.completed_at = Some(Utc::now().to_rfc3339());
        job.result_data = result_data;
    }
    drop(queue);
    emit(app);
}

/// Mark a job as failed with an error message.
pub fn fail<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str, error: String) {
    let mut queue = QUEUE.lock().unwrap();
    if let Some(job) = queue.iter_mut().find(|j| j.id == id) {
        job.status = JobStatus::Failed;
        job.error = Some(error);
        job.completed_at = Some(Utc::now().to_rfc3339());
    }
    drop(queue);
    emit(app);
}

/// Cancel a specific job.
pub fn cancel_job<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) {
    let mut queue = QUEUE.lock().unwrap();
    if let Some(job) = queue.iter_mut().find(|j| j.id == id) {
        job.status = JobStatus::Cancelled;
        job.completed_at = Some(Utc::now().to_rfc3339());
    }
    drop(queue);
    emit(app);
}

/// Return a snapshot of the full queue.
pub fn snapshot() -> Vec<JobEntry> {
    let queue = QUEUE.lock().unwrap();
    queue.clone()
}

/// Remove all completed, failed, and cancelled jobs.
pub fn clear_completed<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let mut queue = QUEUE.lock().unwrap();
    queue.retain(|j| matches!(j.status, JobStatus::Queued | JobStatus::Running));
    drop(queue);
    emit(app);
}

/// Remove a single job from the queue by id.
///
/// Used by the per-row ✕ in the queue dropdown to dismiss one
/// completed / cancelled / failed ("crashed") job at a time. Refuses to
/// touch a still-running job — that path goes through [`cancel_job`] first.
/// Returns `true` if a matching entry was found and removed.
pub fn remove_one<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) -> bool {
    let removed = {
        let mut queue = QUEUE.lock().unwrap();
        let len_before = queue.len();
        queue.retain(|j| j.id != id);
        len_before != queue.len()
    };
    if removed {
        emit(app);
    }
    removed
}

/// Trim the queue to keep at most `MAX_COMPLETED` finished jobs.
fn trim_queue(queue: &mut Vec<JobEntry>) {
    let max = MAX_COMPLETED.load(Ordering::Relaxed);
    if queue.len() <= max {
        return;
    }
    // Keep running + queued, then keep the newest completed ones up to max.
    let running: Vec<JobEntry> = queue
        .drain(..)
        .filter(|j| matches!(j.status, JobStatus::Queued | JobStatus::Running))
        .collect();
    let mut completed: Vec<JobEntry> = queue
        .drain(..)
        .filter(|j| {
            matches!(
                j.status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            )
        })
        .collect();
    completed.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    completed.truncate(max.saturating_sub(running.len()));
    queue.clear();
    queue.extend(running);
    queue.extend(completed);
}
