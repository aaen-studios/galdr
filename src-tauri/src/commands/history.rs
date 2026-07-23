//! Operation history — a persistent log of completed conversions/downloads/etc.
//!
//! Each entry captures the operation type, the input/output paths, a snapshot
//! of the parameters used, and a timestamp. The frontend reads this to offer
//! "re-run" and "retry" actions without the user having to remember settings.

use crate::commands::settings::store_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Runtime;

/// Maximum entries kept. Older ones are trimmed on write.
const MAX_HISTORY: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    /// "conversion" | "batch" | "download" | "transcription" | "concat" | "extract_audio" | ...
    pub op: String,
    /// Human-readable label, e.g. "convert vacation.mp4 → h265".
    pub label: String,
    /// Original input file or URL.
    pub input_path: String,
    /// Resulting output file (if any).
    pub output_path: Option<String>,
    /// Snapshot of the parameters used — opaque JSON blob the frontend interprets.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// "completed" | "failed".
    pub status: String,
    /// ISO 8601 timestamp.
    pub created_at: String,
    /// Output size in bytes (for stats + display).
    #[serde(default)]
    pub output_size: Option<u64>,
}

fn history_path() -> PathBuf {
    store_dir().join("history.json")
}

fn read_all() -> Vec<HistoryEntry> {
    match fs::read_to_string(history_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn write_all(entries: &[HistoryEntry]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(history_path(), json).map_err(|e| e.to_string())
}

/// Append a history entry. Called by the frontend after an operation completes.
#[tauri::command]
pub fn add_history_entry(entry: HistoryEntry) -> Result<(), String> {
    let mut entries = read_all();
    entries.insert(0, entry);
    if entries.len() > MAX_HISTORY {
        entries.truncate(MAX_HISTORY);
    }
    write_all(&entries)
}

/// Return all history entries, newest first.
#[tauri::command]
pub fn list_history() -> Result<Vec<HistoryEntry>, String> {
    Ok(read_all())
}

/// Clear all history entries.
#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    write_all(&[])
}

/// Remove a single history entry by id.
#[tauri::command]
pub fn delete_history_entry(id: String) -> Result<(), String> {
    let mut entries = read_all();
    entries.retain(|e| e.id != id);
    write_all(&entries)
}

/// Aggregate usage stats derived from history. Powers the stats dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub total_ops: usize,
    pub completed: usize,
    pub failed: usize,
    /// Sum of all output sizes in bytes.
    pub total_output_bytes: u64,
    /// Count by operation type.
    pub by_op: std::collections::HashMap<String, usize>,
    /// Last 7 days of activity: [{date, count}].
    pub recent: Vec<(String, usize)>,
}

#[tauri::command]
pub fn get_usage_stats<R: Runtime>(_app: tauri::AppHandle<R>) -> Result<UsageStats, String> {
    let entries = read_all();
    let mut by_op: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut total_output_bytes: u64 = 0;
    let mut completed = 0;
    let mut failed = 0;

    // Build last-7-days buckets (oldest first).
    let mut recent_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let today = chrono::Utc::now().date_naive();
    let mut days: Vec<String> = Vec::new();
    for i in (0..7).rev() {
        let d = (today - chrono::Duration::days(i)).format("%Y-%m-%d").to_string();
        days.push(d.clone());
        recent_map.insert(d, 0);
    }

    for e in &entries {
        *by_op.entry(e.op.clone()).or_insert(0) += 1;
        if let Some(sz) = e.output_size {
            total_output_bytes += sz;
        }
        if e.status == "completed" {
            completed += 1;
        } else if e.status == "failed" {
            failed += 1;
        }
        // Bucket by day.
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&e.created_at) {
            let day = ts.format("%Y-%m-%d").to_string();
            if let Some(v) = recent_map.get_mut(&day) {
                *v += 1;
            }
        }
    }

    let recent = days.into_iter().map(|d| (d.clone(), recent_map.get(&d).copied().unwrap_or(0))).collect();

    Ok(UsageStats {
        total_ops: entries.len(),
        completed,
        failed,
        total_output_bytes,
        by_op,
        recent,
    })
}
