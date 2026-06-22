use crate::models::JobEntry;
use crate::queue;

/// Return a snapshot of every job in the queue (active first, then completed).
#[tauri::command]
pub fn get_queue() -> Vec<JobEntry> {
    let mut jobs = queue::snapshot();
    // Sort: running first, then queued, then by completed_at descending
    jobs.sort_by(|a, b| {
        let a_active = matches!(
            a.status,
            crate::models::JobStatus::Running | crate::models::JobStatus::Queued
        );
        let b_active = matches!(
            b.status,
            crate::models::JobStatus::Running | crate::models::JobStatus::Queued
        );
        if a_active != b_active {
            return if a_active {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        // Both active: running before queued
        if a_active {
            let a_run = matches!(a.status, crate::models::JobStatus::Running);
            let b_run = matches!(b.status, crate::models::JobStatus::Running);
            if a_run != b_run {
                return if a_run {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
            // Same status (both running or both queued): newer first
            return b.created_at.cmp(&a.created_at);
        }
        // Both completed: most recent first
        b.completed_at.cmp(&a.completed_at)
    });
    jobs
}

/// Cancel a specific job by id.
///
/// This marks the matching job `Cancelled` in the queue **and** kills only
/// that job's child process / flips only that job's cancellation token. It
/// does *not* touch any other running job — previously this fired the global
/// `kill_ffmpeg()` / `kill_whisper()` switches which nuked every ffmpeg /
/// whisper-cli process on the machine, so cancelling one job killed them all.
#[tauri::command]
pub fn cancel_job(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    // 1. Mark the matching entry Cancelled (no-op if it's already finished).
    queue::cancel_job(&app_handle, &id);
    // 2. Kill only this job's child process (pid-scoped, not image-scoped).
    let _ = queue::pids::kill_job(&id);
    // 3. Flip only this job's cancellation flag so polling loops bail out.
    queue::cancel_token::set(&id);
    Ok(())
}

/// Remove all completed, failed, and cancelled jobs from the queue.
#[tauri::command]
pub fn clear_completed_jobs(app_handle: tauri::AppHandle) -> Result<(), String> {
    queue::clear_completed(&app_handle);
    Ok(())
}

/// Dismiss a single job from the queue by id (the per-row ✕).
///
/// Removes exactly one finished / cancelled / failed ("crashed") entry.
/// Refuses to drop a still-running job — cancel it first via [`cancel_job`].
/// Returns `true` if a matching entry was found and removed.
#[tauri::command]
pub fn remove_job(app_handle: tauri::AppHandle, id: String) -> Result<bool, String> {
    Ok(queue::remove_one(&app_handle, &id))
}
