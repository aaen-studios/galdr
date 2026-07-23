//! yt-dlp powered URL import commands.
//!
//! Provides the backend for the Import page: fetch metadata from a URL,
//! download media (video / audio / subtitles) via yt-dlp, and manage the
//! app-managed downloads folder. Progress is streamed to the frontend via
//! events, and every download is registered in the global job queue so it
//! appears in the QueueDropdown and can be cancelled with pid-scoped precision.

use std::fs;
use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use regex::Regex;
use tauri::Emitter;
use uuid::Uuid;

use crate::models::job::JobType;
use crate::queue;
use crate::yt_dlp;

// ── Event payloads ──

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpInstallProgressPayload {
    /// 0.0 – 1.0
    pub progress: f64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub job_id: String,
    pub progress: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub percent_str: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLogPayload {
    pub job_id: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCompletePayload {
    pub job_id: String,
    pub output_path: Option<String>,
}

// ── Data types (mirror the frontend types in src/types/index.ts) ──

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatOption {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>,
    pub filesize: Option<u64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub language: String,
    pub name: String,
    pub auto_generated: bool,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlMetadata {
    pub title: String,
    pub uploader: Option<String>,
    pub duration: f64,
    pub thumbnail: Option<String>,
    pub description: Option<String>,
    pub url: String,
    pub webpage_url: Option<String>,
    pub formats: Vec<FormatOption>,
    pub subtitles: Vec<SubtitleTrack>,
    pub is_playlist: bool,
    pub playlist_count: Option<u32>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpStatus {
    pub available: bool,
    pub resolved_path: String,
    pub downloads_dir: String,
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub url: String,
    pub output_dir: String,
    pub media_type: String,        // "video" | "audio" | "both"
    pub format_id: Option<String>,
    pub quality: String,           // "best" | "1080p" | "720p" | "480p" | "audio_only"
    pub download_subtitles: bool,
    pub subtitle_languages: Vec<String>,
    pub embed_subtitles: bool,
    pub extract_audio: bool,
    pub audio_format: Option<String>,
    pub playlist_items: Option<String>,
    pub output_format: Option<String>,  // "mp4" | "webm" | "mkv" | etc.
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: String,
}

// ── Commands ──

/// Report whether yt-dlp is available and where downloads are saved.
#[tauri::command]
pub fn ytdlp_status(download_dir: Option<String>) -> YtdlpStatus {
    let resolved = yt_dlp::yt_dlp_path();
    let dir = yt_dlp::resolve_downloads_dir(download_dir.as_deref().unwrap_or(""));
    YtdlpStatus {
        available: resolved.exists(),
        resolved_path: resolved.to_string_lossy().to_string(),
        downloads_dir: dir.to_string_lossy().to_string(),
    }
}

/// Download yt-dlp from GitHub if not present, streaming progress to the
/// frontend. Returns the resolved binary path on success.
#[tauri::command]
pub async fn ensure_ytdlp(app_handle: tauri::AppHandle) -> Result<String, String> {
    let dest = yt_dlp::cached_binary_path();
    if dest.exists() {
        return Ok(dest.to_string_lossy().to_string());
    }

    let url = yt_dlp::download_url();
    let app_for_thread = app_handle.clone();
    let dest_for_thread = dest.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        // On non-Windows, the downloaded file needs to be made executable.
        let mut response = reqwest::blocking::get(url)
            .map_err(|e| format!("download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("download failed: HTTP {}", response.status()));
        }

        // Try to get total size from Content-Length for progress reporting.
        let total = response.content_length().unwrap_or(0);

        if let Some(parent) = dest_for_thread.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create dir: {}", e))?;
        }

        let tmp_dest = dest_for_thread.with_extension("part");
        let mut file = fs::File::create(&tmp_dest)
            .map_err(|e| format!("failed to create temp file: {}", e))?;

        use std::io::Write;
        let mut downloaded: u64 = 0;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = Read::read(&mut response, &mut buf)
                .map_err(|e| format!("download read error: {}", e))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("write error: {}", e))?;
            downloaded = downloaded.saturating_add(n as u64);
            let progress = if total > 0 {
                (downloaded as f64 / total as f64).min(1.0)
            } else {
                0.0
            };
            let _ = app_for_thread.emit(
                "ytdlp-install-progress",
                YtdlpInstallProgressPayload {
                    progress,
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                },
            );
        }

        fs::rename(&tmp_dest, &dest_for_thread)
            .map_err(|e| format!("failed to finalize yt-dlp: {}", e))?;

        // On Unix, make the binary executable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&dest_for_thread) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&dest_for_thread, perms);
            }
        }

        Ok(dest_for_thread.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("install task panicked: {}", e))??;

    Ok(result)
}

/// Fetch metadata for a URL without downloading. Runs `yt-dlp --dump-json`.
#[tauri::command]
pub async fn fetch_metadata(url: String) -> Result<UrlMetadata, String> {
    let ytdlp = yt_dlp::yt_dlp_path();
    if !ytdlp.exists() {
        return Err("yt-dlp is not installed — install it from the Import page first".to_string());
    }

    let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let mut cmd = Command::new(&ytdlp);
        cmd.args(["--dump-json", "--no-download", "--no-warnings"])
            .arg(&url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("failed to run yt-dlp: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // yt-dlp writes extractor errors to stderr even on partial success;
            // if we have stdout, try to use it anyway.
            if output.stdout.is_empty() {
                return Err(format!("yt-dlp failed: {}", stderr.trim()));
            }
        }

        String::from_utf8(output.stdout)
            .map_err(|e| format!("yt-dlp output was not valid UTF-8: {}", e))
    })
    .await
    .map_err(|e| format!("metadata task panicked: {}", e))??;

    // yt-dlp may emit multiple JSON objects for playlists; take the first line.
    let first_line = output
        .lines()
        .next()
        .ok_or("yt-dlp returned no output")?;

    parse_metadata(first_line)
}

/// Parse a single yt-dlp JSON info dict into `UrlMetadata`.
fn parse_metadata(json_str: &str) -> Result<UrlMetadata, String> {
    let raw: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("failed to parse yt-dlp JSON: {}", e))?;

    // yt-dlp wraps playlist entries under `_type: "playlist"` with `entries`.
    let is_playlist = raw["_type"].as_str() == Some("playlist");
    let entry = if is_playlist {
        raw["entries"].as_array().and_then(|e| e.first()).unwrap_or(&raw)
    } else {
        &raw
    };

    let title = entry["title"]
        .as_str()
        .unwrap_or("untitled")
        .to_string();
    let uploader = entry["uploader"].as_str().map(String::from);
    let duration = entry["duration"].as_f64().unwrap_or(0.0);
    let thumbnail = entry["thumbnail"].as_str().map(String::from);
    let description = entry["description"].as_str().map(String::from);
    let webpage_url = entry["webpage_url"].as_str().map(String::from);

    let playlist_count = if is_playlist {
        raw["playlist_count"].as_u64().map(|c| c as u32)
    } else {
        None
    };

    // Parse available formats into a deduplicated, human-friendly list.
    let formats = parse_formats(entry["formats"].as_array());

    // Parse subtitle tracks.
    let subtitles = parse_subtitles(&raw, entry);

    Ok(UrlMetadata {
        title,
        uploader,
        duration,
        thumbnail,
        description,
        url: entry["webpage_url"]
            .as_str()
            .or_else(|| entry["url"].as_str())
            .unwrap_or("")
            .to_string(),
        webpage_url,
        formats,
        subtitles,
        is_playlist,
        playlist_count,
    })
}

/// Extract a clean format list from yt-dlp's `formats` array.
fn parse_formats(formats: Option<&Vec<serde_json::Value>>) -> Vec<FormatOption> {
    let Some(arr) = formats else {
        return Vec::new();
    };
    let mut result: Vec<FormatOption> = Vec::new();
    for f in arr {
        let format_id = f["format_id"].as_str().unwrap_or("").to_string();
        if format_id.is_empty() {
            continue;
        }
        let ext = f["ext"].as_str().unwrap_or("").to_string();
        let resolution = f["resolution"].as_str().map(String::from);
        let filesize = f["filesize"].as_u64().or_else(|| {
            f["filesize_approx"].as_f64().map(|s| s as u64)
        });
        let vcodec = f["vcodec"].as_str().filter(|s| *s != "none").map(String::from);
        let acodec = f["acodec"].as_str().filter(|s| *s != "none").map(String::from);
        let note = f["format_note"].as_str().map(String::from);

        result.push(FormatOption {
            format_id: format_id.clone(),
            ext,
            resolution,
            filesize,
            vcodec,
            acodec,
            note,
        });
    }
    result
}

/// Extract available subtitle tracks from the yt-dlp info dict.
fn parse_subtitles(raw: &serde_json::Value, entry: &serde_json::Value) -> Vec<SubtitleTrack> {
    let mut tracks: Vec<SubtitleTrack> = Vec::new();

    // `automatic_captions` holds auto-generated CC; `subtitles` holds manual subs.
    for (is_auto, key) in [(true, "automatic_captions"), (false, "subtitles")] {
        if let Some(langs) = raw[key].as_object().or_else(|| entry[key].as_object()) {
            for (lang, entries) in langs {
                let Some(arr) = entries.as_array() else { continue };
                if let Some(first) = arr.first() {
                    let name = first["name"].as_str().unwrap_or(lang).to_string();
                    tracks.push(SubtitleTrack {
                        language: lang.clone(),
                        name,
                        auto_generated: is_auto,
                    });
                }
            }
        }
    }

    tracks
}

/// Build the yt-dlp CLI arg list from `DownloadOptions`.
fn build_download_args(opts: &DownloadOptions, output_template: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // Output path template.
    args.push("-o".to_string());
    args.push(output_template.to_string());

    // Resume partial downloads, don't overwrite existing files.
    args.push("--no-overwrites".to_string());
    args.push("--continue".to_string());

    // Line-buffered progress on stderr so we can parse live updates.
    args.push("--newline".to_string());

    // Quiet down non-progress stderr noise (but keep errors).
    args.push("--no-warnings".to_string());

    // Playlist item selection.
    if let Some(ref items) = opts.playlist_items {
        if !items.trim().is_empty() {
            args.push("--playlist-items".to_string());
            args.push(items.trim().to_string());
        }
    }

    // Format / quality selection.
    if let Some(ref fid) = opts.format_id {
        if !fid.trim().is_empty() {
            args.push("-f".to_string());
            args.push(fid.trim().to_string());
        }
    } else {
        apply_quality_selector(&mut args, &opts.quality, &opts.media_type);
    }

    // Audio extraction.
    if opts.extract_audio || opts.quality == "audio_only" {
        args.push("-x".to_string());
        if let Some(ref fmt) = opts.audio_format {
            args.push("--audio-format".to_string());
            args.push(fmt.clone());
            args.push("--audio-quality".to_string());
            args.push("0".to_string());
        }
    }

    // Output container format (for video merges).
    if let Some(ref fmt) = opts.output_format {
        if !fmt.trim().is_empty() {
            args.push("--merge-output-format".to_string());
            args.push(fmt.trim().to_string());
        }
    }

    // Subtitle download.
    if opts.download_subtitles {
        args.push("--write-subs".to_string());
        if !opts.subtitle_languages.is_empty() {
            args.push("--sub-langs".to_string());
            args.push(opts.subtitle_languages.join(","));
        }
        if opts.embed_subtitles {
            args.push("--embed-subs".to_string());
        }
    }

    // The URL is always the last positional arg.
    args.push(opts.url.clone());

    args
}

/// Translate a user-facing quality preset into a yt-dlp format selector.
fn apply_quality_selector(args: &mut Vec<String>, quality: &str, media_type: &str) {
    let selector = match (quality, media_type) {
        ("audio_only", _) | (_, "audio") => {
            // Best audio-only format; yt-dlp will extract if -x is also passed.
            "bestaudio/best"
        }
        ("1080p", _) => "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
        ("720p", _) => "bestvideo[height<=720]+bestaudio/best[height<=720]",
        ("480p", _) => "bestvideo[height<=480]+bestaudio/best[height<=480]",
        // "best" or anything unrecognised → yt-dlp's default (best video+audio merge).
        _ => "bestvideo+bestaudio/best",
    };
    args.push("-f".to_string());
    args.push(selector.to_string());
}

/// Start a download job. Spawns yt-dlp, streams progress, registers in the
/// queue. Returns the job id.
#[tauri::command]
pub async fn start_download(
    app_handle: tauri::AppHandle,
    options: DownloadOptions,
) -> Result<String, String> {
    let ytdlp = yt_dlp::yt_dlp_path();
    if !ytdlp.exists() {
        return Err("yt-dlp is not installed — install it from the Import page first".to_string());
    }

    let job_id = Uuid::new_v4().to_string();
    let output_dir = yt_dlp::resolve_downloads_dir(&options.output_dir);

    // Output template: `<dir>/<title> [%(id)s].%(ext)s` — unique per video,
    // human-readable, and survives the merge-output-format step.
    let output_template = format!(
        "{}/%(title)s [%(id)s].%(ext)s",
        output_dir.to_string_lossy()
    );

    let args = build_download_args(&options, &output_template);

    // Register the job in the queue immediately so it shows up in the UI
    // before the process even spawns. Register with our own job_id so the
    // queue entry, pid registration, and progress/complete/fail calls all
    // share one id (and cancel-from-queue kills the right pid).
    let label = format!("Downloading {}", options.url);
    queue::register_with_id(
        &app_handle,
        &job_id,
        JobType::Download,
        label,
        options.url.clone(),
        None,
    );

    let app_for_thread = app_handle.clone();
    let job_id_for_thread = job_id.clone();

    tokio::task::spawn_blocking(move || {
        run_ytdlp_download(
            &ytdlp,
            &args,
            &job_id_for_thread,
            &app_for_thread,
            0.0, // duration unknown at this point; fake progress uses a generic estimate
        );
    });

    Ok(job_id)
}

/// Run yt-dlp with piped stderr, parse progress lines, and emit events.
/// Mirrors the streaming pattern in `whisper/runner.rs`.
fn run_ytdlp_download(
    ytdlp: &std::path::Path,
    args: &[String],
    job_id: &str,
    app: &tauri::AppHandle,
    duration_secs: f64,
) {
    let mut cmd = Command::new(ytdlp);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = app.emit(
                "download-log",
                DownloadLogPayload {
                    job_id: job_id.to_string(),
                    message: format!("! failed to spawn yt-dlp: {}", e),
                },
            );
            queue::fail(app, job_id, format!("failed to spawn yt-dlp: {}", e));
            return;
        }
    };

    // Register the child's pid so cancel_download can kill exactly this process.
    crate::queue::pids::register_pid(job_id, child.id());

    // Fake progress fallback: advance based on elapsed time when real progress
    // is unavailable (live streams, HLS, unknown size). Capped at 95% so it
    // never looks "done" before the real completion event.
    let job_id_fake = job_id.to_string();
    let app_fake = app.clone();
    let fake_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let fake_running_check = std::sync::Arc::clone(&fake_running);
    let estimated_secs = if duration_secs > 0.0 {
        // Rough heuristic: ~5 seconds per minute of content for a typical download
        (duration_secs / 60.0 * 5.0).max(10.0)
    } else {
        60.0 // generic 60s estimate when duration is unknown
    };
    let fake_handle = thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut last_pct = 0.0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if !fake_running_check.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let elapsed = start.elapsed().as_secs_f64();
            let fake_pct = ((elapsed / estimated_secs) * 100.0).min(95.0) / 100.0;
            if fake_pct > last_pct {
                last_pct = fake_pct;
                let _ = app_fake.emit("download-progress", DownloadProgressPayload {
                    job_id: job_id_fake.clone(),
                    progress: fake_pct,
                    speed: Some("estimating…".to_string()),
                    eta: None,
                    percent_str: Some(format!("~{:.0}%", fake_pct * 100.0)),
                });
            }
        }
    });

    let stderr = child.stderr.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    // yt-dlp default progress lines look like:
    //   [download]  12.3% of ~15.23MiB at  2.56MiB/s ETA 00:05
    let progress_re = Regex::new(
        r"^\[download\]\s+([\d.]+)%\s+of\s+~?[\d.]+\w*\s+at\s+([\d.]+)\w*/s\s+ETA\s+(\S+)"
    ).unwrap();

    let (tx, rx) = mpsc::channel::<DownloadEvent>();

    // stderr reader: progress + log lines.
    let tx_err = tx.clone();
    thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines().flatten() {
            if let Some(caps) = progress_re.captures(&line) {
                let pct: f64 = caps[1].parse().unwrap_or(0.0) / 100.0;
                let speed = Some(caps[2].to_string()).filter(|s| !s.is_empty() && s != "Unknown");
                let eta = Some(caps[3].to_string()).filter(|s| !s.is_empty() && s != "Unknown");
                let _ = tx_err.send(DownloadEvent::Progress {
                    progress: pct,
                    speed,
                    eta,
                    percent_str: Some(format!("{:.1}%", pct * 100.0)),
                });
            } else if !line.trim().is_empty() {
                let _ = tx_err.send(DownloadEvent::Log(line));
            }
        }
    });

    // stdout reader: yt-dlp writes destination paths and info here.
    let tx_out = tx.clone();
    thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if !line.trim().is_empty() {
                let _ = tx_out.send(DownloadEvent::Log(line));
            }
        }
    });

    drop(tx);

    // Drain events live.
    for ev in rx.iter() {
        match ev {
            DownloadEvent::Progress {
                progress,
                speed,
                eta,
                percent_str,
            } => {
                let _ = app.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        job_id: job_id.to_string(),
                        progress,
                        speed,
                        eta,
                        percent_str,
                    },
                );
                queue::update_progress(app, job_id, progress);
            }
            DownloadEvent::Log(msg) => {
                let _ = app.emit(
                    "download-log",
                    DownloadLogPayload {
                        job_id: job_id.to_string(),
                        message: msg,
                    },
                );
            }
        }
    }

    // Stop the fake progress fallback now that real progress has finished.
    fake_running.store(false, std::sync::atomic::Ordering::SeqCst);
    let _ = fake_handle.join();

    // Check for cancellation.
    if crate::queue::pids::is_cancelled(job_id) {
        crate::queue::pids::unregister(job_id);
        let _ = app.emit(
            "download-complete",
            DownloadCompletePayload {
                job_id: job_id.to_string(),
                output_path: None,
            },
        );
        queue::cancel_job(app, job_id);
        return;
    }

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            crate::queue::pids::unregister(job_id);
            let _ = app.emit(
                "download-log",
                DownloadLogPayload {
                    job_id: job_id.to_string(),
                    message: format!("! failed to wait on yt-dlp: {}", e),
                },
            );
            queue::fail(app, job_id, format!("failed to wait on yt-dlp: {}", e));
            return;
        }
    };

    crate::queue::pids::unregister(job_id);

    if status.success() {
        let _ = app.emit(
            "download-complete",
            DownloadCompletePayload {
                job_id: job_id.to_string(),
                output_path: None, // caller can list_downloads to find the file
            },
        );
        queue::complete(app, job_id, None, None);
    } else {
        let msg = format!("yt-dlp exited with code: {}", status);
        let _ = app.emit(
            "download-log",
            DownloadLogPayload {
                job_id: job_id.to_string(),
                message: format!("! {}", msg),
            },
        );
        queue::fail(app, job_id, msg);
    }
}

/// Events emitted by the yt-dlp reader threads.
enum DownloadEvent {
    Progress {
        progress: f64,
        speed: Option<String>,
        eta: Option<String>,
        percent_str: Option<String>,
    },
    Log(String),
}

/// Cancel a running download by job id. Kills the exact child process.
#[tauri::command]
pub fn cancel_download(job_id: String) -> Result<(), String> {
    crate::queue::pids::kill_job(&job_id)?;
    crate::queue::pids::set(&job_id);
    Ok(())
}

/// List files in the downloads folder, most-recent-first.
#[tauri::command]
pub fn list_downloads(download_dir: Option<String>) -> Vec<DownloadedFile> {
    let dir = yt_dlp::resolve_downloads_dir(download_dir.as_deref().unwrap_or(""));
    let mut files: Vec<DownloadedFile> = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return files,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = metadata.len();
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                let dt = chrono::DateTime::from_timestamp(d.as_secs() as i64, 0);
                dt.map(|d| d.to_rfc3339()).unwrap_or_default()
            })
            .unwrap_or_default();

        files.push(DownloadedFile {
            path: path.to_string_lossy().to_string(),
            name,
            size,
            modified,
        });
    }

    // Sort by modified time, newest first.
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    files
}

/// Delete a downloaded file from disk.
#[tauri::command]
pub fn delete_download(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    // Safety: refuse to delete anything outside the downloads dir to prevent
    // path-traversal via a crafted path.
    let downloads_dir = yt_dlp::default_downloads_dir();
    if !p.starts_with(&downloads_dir) {
        return Err("can only delete files inside the downloads folder".to_string());
    }
    if p.exists() {
        fs::remove_file(&p).map_err(|e| format!("failed to delete file: {}", e))?;
    }
    Ok(())
}
