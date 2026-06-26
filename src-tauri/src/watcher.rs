//! Watch-folder daemon.
//!
//! One `notify` watcher per configured folder. On a create/modify event the
//! file is debounced (settle check), filtered by glob pattern + file age,
//! then either auto-converted (Phase 3) or pushed to the in-memory review
//! queue and surfaced to the UI via `watch://` events.
//!
//! Designed to keep running while the main window is hidden (close-to-tray):
//! it lives entirely in Tauri managed state and emits events that the UI
//! re-subscribes to whenever it's visible.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use globset::GlobSet;
use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use uuid::Uuid;

use crate::commands::settings::{load_settings, save_settings};
use crate::models::watch_folder::{
    ConflictPolicy, QueuedFile, WatchAction, WatchFolderConfig, WatchLogEntry, WatchLogStatus,
    WatchOutputFormat, MAX_LOG_ENTRIES,
};
use crate::tray;

/// How often the settle-sweep loop runs.
const SWEEP_INTERVAL_MS: u64 = 500;

/// Managed state: active notify watchers + the review queue.
pub struct WatcherState {
    /// Active watchers keyed by folder id. Dropping a watcher stops it.
    watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    /// Files seen but not yet settled, keyed by (folder_id, path).
    /// Value: (first_seen, folder_id, path)
    pending: Mutex<HashMap<String, (Instant, String, String)>>,
    /// The manual-review queue (Queue-action folders).
    queue: Mutex<VecDeque<QueuedFile>>,
}

impl Default for WatcherState {
    fn default() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
        }
    }
}

impl WatcherState {
    pub fn queued_files(&self) -> Vec<QueuedFile> {
        self.queue.lock().map(|q| q.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn take_queue(&self) -> Vec<QueuedFile> {
        self.queue.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
    }

    pub fn remove_queued(&self, id: &str) -> Option<QueuedFile> {
        self.queue
            .lock()
            .ok()?
            .iter()
            .position(|f| f.id == id)
            .and_then(|i| self.queue.lock().ok()?.remove(i))
    }
}

/// Start (or replace) watchers for all enabled folders in settings. Called
/// from `setup` and whenever watch-folder config changes.
pub fn start_watcher<R: Runtime>(app: &AppHandle<R>) {
    let settings = load_settings();
    stop_watcher(app);

    let state = app.state::<WatcherState>();

    let mut watchers = HashMap::new();
    let enabled_count;

    state.pending.lock().expect("pending poisoned").clear();

    enabled_count = settings
        .watch_folders
        .iter()
        .filter(|f| f.enabled)
        .count();

    // Clone config + app handle for each watcher closure.
    for folder in settings.watch_folders.iter().filter(|f| f.enabled) {
        match spawn_folder_watcher(app.clone(), folder.clone()) {
            Ok(w) => {
                watchers.insert(folder.id.clone(), w);
            }
            Err(e) => {
                eprintln!("[watcher] failed to watch {}: {}", folder.path, e);
            }
        }
    }

    *state.watchers.lock().expect("watchers poisoned") = watchers;

    // Reflect status in the tray tooltip.
    if enabled_count > 0 {
        tray::set_tooltip_status(app, Some(&format!("watching {} folder{}", enabled_count, if enabled_count == 1 { "" } else { "s" })));
    } else {
        tray::set_tooltip_status(app, None);
    }

    // Kick off the settle-sweep loop once.
    ensure_sweep_loop(app.clone());
}

/// Stop all watchers and clear pending detections (queue is preserved).
pub fn stop_watcher<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<WatcherState>();
    state.watchers.lock().expect("watchers poisoned").clear();
    state.pending.lock().expect("pending poisoned").clear();
}

/// Build a notify watcher for a single folder. The handler debounces events
/// via the shared `pending` map; the sweep loop promotes settled files.
fn spawn_folder_watcher<R: Runtime>(
    app: AppHandle<R>,
    folder: WatchFolderConfig,
) -> notify::Result<RecommendedWatcher> {
    let folder_id = folder.id.clone();
    let folder_path = folder.path.clone();
    let recursive = folder.recursive;
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let event = match res {
            Ok(e) => e,
            Err(_) => return,
        };
        // Only react to new/changed files, not accesses or removals.
        let is_relevant = matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_)
        );
        if !is_relevant {
            return;
        }
        if tray::WATCHING_PAUSED.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }

        for path in &event.paths {
            // Skip directories themselves; only register files.
            if path.is_dir() {
                continue;
            }
            let key = format!("{}|{}", folder_id, path.display());
            if let Some(pending) = app.try_state::<WatcherState>() {
                pending
                    .pending
                    .lock()
                    .expect("pending poisoned")
                    .insert(key, (Instant::now(), folder_id.clone(), path.display().to_string()));
            }
        }
    })?;

    watcher.watch(
        Path::new(&folder_path),
        if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive },
    )?;
    let _ = folder_id; // already moved into closure
    Ok(watcher)
}

/// Ensure the settle-sweep background loop is running. Idempotent — uses a
/// OnceCell-style guard so repeated calls don't stack loops.
static SWEEP_STARTED: std::sync::Once = std::sync::Once::new();

fn ensure_sweep_loop<R: Runtime + 'static>(app: AppHandle<R>) {
    SWEEP_STARTED.call_once(|| {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(SWEEP_INTERVAL_MS));
            let settled = take_settled(&app);
            for (folder_id, path) in settled {
                handle_settled_file(&app, &folder_id, &path);
            }
        });
    });
}

/// Pull all pending entries that have been quiet for >= their folder's settle
/// duration. Each folder can have a different debounce window.
fn take_settled<R: Runtime>(app: &AppHandle<R>) -> Vec<(String, String)> {
    let state = app.state::<WatcherState>();
    let mut pending = state.pending.lock().expect("pending poisoned");
    let now = Instant::now();

    // Collect folder settle durations so we can compare per-folder.
    let settle_map: HashMap<String, u64> = load_settings()
        .watch_folders
        .into_iter()
        .map(|f| (f.id, f.settle_ms))
        .collect();

    let mut out = Vec::new();
    pending.retain(|_key, (seen, folder_id, path)| {
        let settle_ms = settle_map.get(folder_id).copied().unwrap_or(10000);
        if now.duration_since(*seen) >= Duration::from_millis(settle_ms) {
            out.push((folder_id.clone(), path.clone()));
            false // remove settled
        } else {
            true // keep waiting
        }
    });
    out
}

/// Build a GlobSet from a folder's patterns. Returns None if patterns is
/// empty (meaning "accept all").
fn build_pattern_set(folder: &WatchFolderConfig) -> Option<GlobSet> {
    if folder.patterns.is_empty() {
        return None;
    }
    let mut builder = globset::GlobSetBuilder::new();
    for pat in &folder.patterns {
        // globset patterns match against the full path by default; we only
        // want to match the filename. We use the pattern as-is but match
        // against the file_name() component.
        if let Ok(glob) = globset::Glob::new(pat) {
            builder.add(glob);
        }
    }
    builder.build().ok()
}

/// Check whether a filename matches any of the folder's glob patterns.
fn matches_patterns(folder: &WatchFolderConfig, filename: &str) -> bool {
    let set = match build_pattern_set(folder) {
        Some(s) => s,
        None => return true, // no patterns = accept all
    };
    set.is_match(filename)
}

/// A file has settled: validate it, then route by the folder's action mode.
fn handle_settled_file<R: Runtime>(app: &AppHandle<R>, folder_id: &str, path: &str) {
    // Look up the folder config to apply filters + action.
    let settings = load_settings();
    let folder = match settings.watch_folders.iter().find(|f| f.id == folder_id) {
        Some(f) => f.clone(),
        None => return,
    };

    // File must still exist (settle may have caught a temp that was removed).
    let p = Path::new(path);
    if !p.exists() || !p.is_file() {
        return;
    }

    // ── Glob pattern filter ──
    let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !matches_patterns(&folder, filename) {
        return;
    }

    // ── File age filter ──
    if folder.ignore_older_than_minutes > 0 {
        if let Ok(metadata) = p.metadata() {
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = modified.elapsed() {
                    let max_age = Duration::from_secs(folder.ignore_older_than_minutes * 60);
                    if age > max_age {
                        // Log the skip and return.
                        let entry = WatchLogEntry {
                            input_path: path.to_string(),
                            output_paths: vec![],
                            status: WatchLogStatus::SkippedAge,
                            timestamp: Utc::now().to_rfc3339(),
                            error: None,
                        };
                        push_log_entry(&folder.id, entry);
                        return;
                    }
                }
            }
        }
    }

    match folder.action {
        WatchAction::AutoConvert => {
            // Spawn so the blocking conversion runs off the sweep thread and
            // multiple folders convert concurrently. A per-folder mutex
            // serializes conversions within a folder to avoid ffmpeg storms.
            let app2 = app.clone();
            let folder2 = folder.clone();
            let path2 = path.to_string();
            std::thread::spawn(move || {
                run_auto_convert(app2, folder2, path2);
            });
        }
        WatchAction::Queue => {
            enqueue_file(app, &folder, path);
        }
    }
}

/// Compute the output path for a given format, applying conflict resolution.
/// Returns None if the file should be skipped due to conflict policy.
fn resolve_output_path(
    folder: &WatchFolderConfig,
    fmt: &WatchOutputFormat,
    input_stem: &str,
) -> Option<std::path::PathBuf> {
    let output_dir = if fmt.output_dir.is_empty() {
        std::path::PathBuf::from(&folder.output_dir)
    } else {
        std::path::PathBuf::from(&fmt.output_dir)
    };

    let mut out_path = output_dir.join(format!("{}.{}", input_stem, fmt.output_format));

    if out_path.exists() {
        match folder.conflict_policy {
            ConflictPolicy::Skip => return None,
            ConflictPolicy::Overwrite => {} // proceed — ffmpeg -y handles it
            ConflictPolicy::Rename => {
                let mut counter = 1;
                loop {
                    let candidate = output_dir.join(format!(
                        "{}_{}.{}",
                        input_stem, counter, fmt.output_format
                    ));
                    if !candidate.exists() {
                        out_path = candidate;
                        break;
                    }
                    counter += 1;
                    // Safety valve — avoid infinite loops on pathological cases.
                    if counter > 9999 {
                        return None;
                    }
                }
            }
        }
    }

    Some(out_path)
}

/// Run auto-conversion for a detected file, producing one output per format.
/// Serialized per folder by a shared mutex so one folder never runs two
/// ffmpeg processes at once.
fn run_auto_convert<R: Runtime>(app: AppHandle<R>, folder: WatchFolderConfig, path: String) {
    let job_id = format!("watch:{}", folder.id);

    // Per-folder serialization lock.
    let lock = folder_lock(&folder.id);
    let _guard = lock.lock().expect("folder lock poisoned");

    let input_stem = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string();

    // Determine the effective output formats. Fall back to legacy params
    // if output_formats is empty (shouldn't happen after migration, but safe).
    let formats: Vec<WatchOutputFormat> = if folder.output_formats.is_empty() {
        vec![WatchOutputFormat {
            output_format: folder.params.output_format.clone(),
            quality: folder.params.quality,
            output_dir: folder.output_dir.clone(),
        }]
    } else {
        folder.output_formats.clone()
    };

    // Compute output paths with conflict resolution. If ALL formats would be
    // skipped, log and return early.
    let mut output_paths: Vec<(WatchOutputFormat, std::path::PathBuf)> = Vec::new();
    let mut all_skipped = true;
    for fmt in &formats {
        match resolve_output_path(&folder, fmt, &input_stem) {
            Some(p) => {
                output_paths.push((fmt.clone(), p));
                all_skipped = false;
            }
            None => {
                // This format is skipped due to conflict policy.
                continue;
            }
        }
    }

    if all_skipped {
        // All outputs already exist and policy is Skip.
        let entry = WatchLogEntry {
            input_path: path.clone(),
            output_paths: output_paths.iter().map(|(_, p)| p.to_string_lossy().to_string()).collect(),
            status: WatchLogStatus::SkippedConflict,
            timestamp: Utc::now().to_rfc3339(),
            error: None,
        };
        push_log_entry(&folder.id, entry);
        let _ = app.emit(
            "watch://convert-error",
            serde_json::json!({ "folderId": folder.id, "path": path, "error": "skipped: output already exists" }),
        );
        return;
    }

    let _ = app.emit(
        "watch://convert-started",
        serde_json::json!({ "folderId": folder.id, "path": path, "jobId": job_id }),
    );

    // Run conversion for each format sequentially (within the folder lock).
    let mut all_output_paths: Vec<String> = Vec::new();
    let mut any_failed = false;
    let mut last_error = String::new();

    for (fmt, output_path) in &output_paths {
        let mut params = folder.params.clone();
        params.input_path = std::path::PathBuf::from(&path);
        params.output_dir = output_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        params.output_format = fmt.output_format.clone();
        if let Some(q) = fmt.quality {
            params.quality = Some(q);
        }

        // When watching recursively with path preservation, mirror the
        // relative subfolder path under the output directory.
        if folder.recursive && folder.preserve_path {
            if let Ok(relative) = std::path::Path::new(&path).strip_prefix(&folder.path) {
                if let Some(parent) = relative.parent() {
                    if !parent.as_os_str().is_empty() {
                        params.output_dir = params.output_dir.join(parent);
                    }
                }
            }
        }

        match crate::commands::run_single_conversion(&app, params, &job_id) {
            Ok(done) => {
                all_output_paths.push(done.output_path);
            }
            Err(e) => {
                any_failed = true;
                last_error = e;
                // Continue with remaining formats — partial success is still useful.
            }
        }
    }

    if any_failed {
        let entry = WatchLogEntry {
            input_path: path.clone(),
            output_paths: all_output_paths.clone(),
            status: WatchLogStatus::Failed,
            timestamp: Utc::now().to_rfc3339(),
            error: Some(last_error.clone()),
        };
        push_log_entry(&folder.id, entry);

        let _ = app.emit(
            "watch://convert-error",
            serde_json::json!({ "folderId": folder.id, "path": path, "error": last_error }),
        );
    } else {
        let entry = WatchLogEntry {
            input_path: path.clone(),
            output_paths: all_output_paths.clone(),
            status: WatchLogStatus::Success,
            timestamp: Utc::now().to_rfc3339(),
            error: None,
        };
        push_log_entry(&folder.id, entry);

        let _ = app.emit(
            "watch://convert-done",
            serde_json::json!({
                "folderId": folder.id,
                "path": path,
                "outputPath": all_output_paths.first(),
                "outputPaths": all_output_paths,
            }),
        );

        // Optional: remove the source after a successful conversion.
        if folder.delete_source {
            let _ = std::fs::remove_file(&path);
        }
    }

    // Rebuild a concise tray status reflecting active folders.
    crate::tray::set_tooltip_status(&app, Some("converted"));
}

/// Append a log entry to a folder's processing_log, trim to MAX_LOG_ENTRIES,
/// and persist settings. Log entries are most-recent-first.
fn push_log_entry(folder_id: &str, entry: WatchLogEntry) {
    let mut settings = load_settings();
    if let Some(folder) = settings.watch_folders.iter_mut().find(|f| f.id == folder_id) {
        folder.processing_log.insert(0, entry);
        if folder.processing_log.len() > MAX_LOG_ENTRIES {
            folder.processing_log.truncate(MAX_LOG_ENTRIES);
        }
    }
    // Best-effort save — don't crash the watcher if persistence fails.
    let _ = save_settings(settings);
}

/// A shared, lazily-created mutex per folder id. Keeps concurrent
/// conversions for the same folder from overlapping.
static FOLDER_LOCKS: once_cell::sync::Lazy<std::sync::Mutex<HashMap<String, std::sync::Arc<std::sync::Mutex<()>>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

fn folder_lock(folder_id: &str) -> std::sync::Arc<std::sync::Mutex<()>> {
    let mut map = FOLDER_LOCKS.lock().expect("folder locks poisoned");
    map.entry(folder_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(std::sync::Mutex::new(())))
        .clone()
}

/// Add a file to the manual-review queue and notify the UI.
fn enqueue_file<R: Runtime>(app: &AppHandle<R>, folder: &WatchFolderConfig, path: &str) {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let entry = QueuedFile {
        id: Uuid::new_v4().to_string(),
        folder_id: folder.id.clone(),
        folder_path: folder.path.clone(),
        path: path.to_string(),
        name,
        queued_at: Utc::now().to_rfc3339(),
    };
    let _ = app.emit("watch://file-queued", entry.clone());
    let state = app.state::<WatcherState>();
    state.queue.lock().expect("queue poisoned").push_back(entry);
}
