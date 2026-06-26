//! Per-job process & cancellation registry.
//!
//! The queue stores one [`JobEntry`] per background job, but the actual
//! work (ffmpeg / whisper-cli / forge export) runs in a child OS process
//! spawned from inside the relevant command. To cancel a *single* job we
//! need to kill exactly that child — not every `ffmpeg.exe` on the machine
//! (which is what the legacy `taskkill /IM ffmpeg.exe /F` did and is the
//! root cause of the "cancel one cancels all" bug).
//!
//! Two maps are kept here, both keyed by job id:
//!
//! * [`PIDS`] — the OS pid of the spawned child, so [`kill_job`] can target
//!   it precisely with `taskkill /PID <pid> /F /T` (Windows) or
//!   `kill -9 <pid>` (Unix).
//! * [`TOKENS`] — a per-job cancellation flag. Long-running loops (the batch
//!   converter, the whisper download) poll [`is_cancelled`] between iterations
//!   to bail out cleanly without waiting for the process to be reaped.
//!
//! Entries auto-expire: when a job's owning command finishes it calls
//! [`unregister`], removing both maps' entries. Cancelling a job that has
//! already finished is a harmless no-op.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

// ── Process registry ──

/// OS process id for each running job's child process.
static PIDS: Lazy<Mutex<HashMap<String, u32>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Register the OS pid of the child process belonging to `job_id`.
///
/// Called by the ffmpeg / whisper runners right after `Command::spawn()`
/// succeeds. The runner is responsible for calling [`unregister`] when the
/// child exits so the map doesn't grow without bound.
pub fn register_pid(job_id: &str, pid: u32) {
    if let Ok(mut map) = PIDS.lock() {
        map.insert(job_id.to_string(), pid);
    }
}

/// Drop the stored pid for `job_id` (the child has exited).
pub fn unregister_pid(job_id: &str) {
    if let Ok(mut map) = PIDS.lock() {
        map.remove(job_id);
    }
}

/// Kill *only* the child process belonging to `job_id`.
///
/// Unlike the legacy `kill_ffmpeg()` / `kill_whisper()` helpers this targets
/// a specific pid, so cancelling one job leaves every other running job
/// untouched. Returns `Ok(())` if there was nothing to kill (job already
/// finished, or the pid has been reaped) so callers can always ignore the
/// `Result` when they don't care.
pub fn kill_job(job_id: &str) -> Result<(), String> {
    let pid = {
        let mut map = PIDS.lock().map_err(|e| format!("pid lock poisoned: {}", e))?;
        map.remove(job_id)
    };

    let Some(pid) = pid else {
        // No registered child for this job — either it already finished or
        // it never spawned one (e.g. a Queued job that was cancelled before
        // being picked up by the worker). Nothing to kill.
        return Ok(());
    };

    #[cfg(windows)]
    {
        // /T takes down any child processes the pid spawned (ffmpeg sometimes
        // forks helper processes during two-pass encoding).
        let out = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .output()
            .map_err(|e| format!("failed to run taskkill: {}", e))?;
        // taskkill prints a non-zero exit when the pid isn't running anymore;
        // treat that as success since the desired end state is "process gone".
        if !out.status.success()
            && !String::from_utf8_lossy(&out.stderr).contains("not running")
            && !String::from_utf8_lossy(&out.stderr).contains("could not be found")
        {
            return Err(format!(
                "taskkill failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }
    #[cfg(not(windows))]
    {
        // Ignore "no such process" (ESRCH) — the child already exited.
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }

    Ok(())
}

// ── Cancellation-token registry ──

/// Per-job cancellation flags. Each running job gets an [`Arc<AtomicBool>`]
/// the instant it starts; polling loops check [`is_cancelled`] between
/// iterations, and [`set`] flips exactly one job's flag from `cancel_job`.
///
/// This replaces the two process-wide `AtomicBool`s
/// (`convert::CANCELLED`, `subtitles::CANCELLED_TRANSCRIPTION`) that
/// previously broadcast a cancel to *every* in-flight conversion or
/// transcription.
static TOKENS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Create (or reset) the cancellation token for `job_id` and return a clone
/// the caller can poll. Called by a command just before it starts its work.
pub fn acquire_token(job_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut map) = TOKENS.lock() {
        map.insert(job_id.to_string(), Arc::clone(&flag));
    }
    flag
}

/// Flip the cancellation flag for `job_id` (if it exists). Called by
/// `cancel_job`. No-op for jobs that have already finished and unregistered.
pub fn set(job_id: &str) {
    if let Ok(map) = TOKENS.lock() {
        if let Some(flag) = map.get(job_id) {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

/// True if `job_id`'s token has been flipped. A job with no registered token
/// (e.g. one that already finished and called [`unregister`]) is never
/// considered cancelled.
pub fn is_cancelled(job_id: &str) -> bool {
    if let Ok(map) = TOKENS.lock() {
        if let Some(flag) = map.get(job_id) {
            return flag.load(Ordering::SeqCst);
        }
    }
    false
}

/// Drop the token for `job_id`. Called by a command when it finishes (success
/// or failure) so the map doesn't accumulate stale entries. Safe to call for
/// a job that was never registered.
pub fn unregister(job_id: &str) {
    unregister_pid(job_id);
    if let Ok(mut map) = TOKENS.lock() {
        map.remove(job_id);
    }
}

// ── Download-cancellation registry ──

/// Per-model cancellation flags for in-flight whisper model downloads.
/// Mirrors the per-job token API above but keyed by model id.
static DOWNLOAD_CANCEL: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register a cancellation flag for a download and return a clone the
/// download thread can poll.
pub fn acquire_download_cancel(model_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut map) = DOWNLOAD_CANCEL.lock() {
        map.insert(model_id.to_string(), Arc::clone(&flag));
    }
    flag
}

/// Flip the cancellation flag for `model_id`. No-op if the download already
/// finished and unregistered.
pub fn cancel_download(model_id: &str) {
    if let Ok(map) = DOWNLOAD_CANCEL.lock() {
        if let Some(flag) = map.get(model_id) {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

/// Drop the cancellation flag for `model_id` after the download finishes
/// (success or failure).
pub fn unregister_download(model_id: &str) {
    if let Ok(mut map) = DOWNLOAD_CANCEL.lock() {
        map.remove(model_id);
    }
}

/// True if `model_id`'s download has been cancelled.
pub fn is_download_cancelled(model_id: &str) -> bool {
    if let Ok(map) = DOWNLOAD_CANCEL.lock() {
        if let Some(flag) = map.get(model_id) {
            return flag.load(Ordering::SeqCst);
        }
    }
    false
}
