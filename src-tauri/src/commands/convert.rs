use std::path::PathBuf;
use tauri::Emitter;

use crate::discord_rpc;
use crate::ffmpeg::{build_args, build_two_pass_args, probe_file, run_conversion};
use crate::models::{BatchConversionParams, ConversionParams, ScannedFile};

#[derive(Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct ConversionStartedPayload {
    pub job_id: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ConversionProgressPayload {
    pub job_id: String,
    pub progress: f64,
}

#[derive(Clone, serde::Serialize)]
pub struct ConversionDonePayload {
    pub job_id: String,
    pub output_path: String,
}

#[derive(Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct ConversionErrorPayload {
    pub job_id: String,
    pub error: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ConversionLogPayload {
    pub message: String,
}

#[tauri::command]
pub async fn start_conversion(
    app_handle: tauri::AppHandle,
    params: ConversionParams,
) -> Result<ConversionDonePayload, String> {
    let job_id = crate::queue::register(
        &app_handle,
        crate::models::JobType::Conversion,
        crate::queue::make_label(
            &crate::models::JobType::Conversion,
            &params.input_path.to_string_lossy(),
        ),
        params.input_path.to_string_lossy().to_string(),
        None,
    );
    let _ = crate::queue::cancel_token::acquire(&job_id);

    // `run_single_conversion` blocks for the entire ffmpeg run (it reads
    // stderr line-by-line until the child exits). Run it on a blocking thread
    // so we don't stall Tauri's async executor — and other IPC — for the
    // duration of the conversion.
    let convert_app = app_handle.clone();
    let convert_jid = job_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_single_conversion(&convert_app, params, &convert_jid)
    })
    .await
    .map_err(|e| format!("Conversion task panicked: {}", e))?;

    match &result {
        Ok(done) => {
            crate::queue::complete(
                &app_handle,
                &job_id,
                Some(done.output_path.clone()),
                None,
            );
        }
        Err(e) => {
            crate::queue::fail(&app_handle, &job_id, e.clone());
        }
    }

    result
}

/// Shared conversion core. Runs one file through the ffmpeg pipeline and
/// emits `conversion-progress` / `conversion-log` events tagged with the
/// given `job_id`. Used by the manual Convert command (`job_id = "default"`)
/// and by the watch-folder auto-converter (`job_id = "watch:<folderId>"`).
///
/// When `params.target_size_bytes` is set, uses two-pass encoding for video
/// (single-pass bitrate targeting for audio) instead of the quality slider.
pub fn run_single_conversion<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    params: ConversionParams,
    job_id: &str,
) -> Result<ConversionDonePayload, String> {
    std::fs::create_dir_all(&params.output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    let probe_info = probe_file(&params.input_path);
    let full_duration = probe_info
        .as_ref()
        .map(|info| info.duration)
        .unwrap_or(0.0);

    let file_name = std::path::Path::new(&params.input_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let source_format = std::path::Path::new(&params.input_path)
        .extension()
        .and_then(|s| s.to_str());

    // Compute effective duration accounting for trim
    let trim_start = params.trim_start.unwrap_or(0.0);
    let trim_end = params.trim_end.filter(|&e| e > 0.0).unwrap_or(full_duration);
    let effective_duration = (trim_end - trim_start).max(1.0);

    discord_rpc::set_converting(&file_name, 0.0, &params.output_format, source_format);

    let is_target_size = params.target_size_bytes.is_some();

    let (events, _) = if is_target_size {
        // ── Target-size mode (two-pass for video, bitrate for audio) ──
        let (pass1, pass2) = build_two_pass_args(&params, effective_duration)?;

        if !pass1.is_empty() {
            // Run pass 1 — analysis only
            let _ = app_handle.emit(
                "conversion-log",
                ConversionLogPayload {
                    message: "pass 1/2 analysis".to_string(),
                },
            );
            let pass1_events = run_conversion(&pass1, effective_duration, |_| {}, job_id)?;
            for ev in &pass1_events {
                match ev {
                    crate::ffmpeg::FfmpegEvent::Error(msg) => {
                        // Clean up pass log files on error
                        let _ = std::fs::remove_file("ffmpeg2pass-0.log");
                        let _ = std::fs::remove_file("ffmpeg2pass-0.log.mbtree");
                        discord_rpc::set_idle();
                        return Err(msg.clone());
                    }
                    _ => {}
                }
            }

            let _ = app_handle.emit(
                "conversion-log",
                ConversionLogPayload {
                    message: "pass 2/2 encoding".to_string(),
                },
            );
        }

        // Run pass 2 (or the only pass for audio/GIF)
        let pass2_app = app_handle.clone();
        let pass2_fname = file_name.clone();
        let pass2_fmt = params.output_format.clone();
        let pass2_jid = job_id.to_string();
        let pass2_src = source_format.map(|s| s.to_string());
        let pass2_events = run_conversion(&pass2, effective_duration, move |ev| {
            match ev {
                crate::ffmpeg::FfmpegEvent::Progress(p) => {
                    discord_rpc::set_converting(&pass2_fname, *p, &pass2_fmt, pass2_src.as_deref());
                    let _ = pass2_app.emit(
                        "conversion-progress",
                        ConversionProgressPayload {
                            job_id: pass2_jid.clone(),
                            progress: *p,
                        },
                    );
                    crate::queue::update_progress(&pass2_app, &pass2_jid, *p);
                }
                crate::ffmpeg::FfmpegEvent::Log(msg) => {
                    let _ = pass2_app.emit(
                        "conversion-log",
                        ConversionLogPayload {
                            message: msg.clone(),
                        },
                    );
                }
                _ => {}
            }
        }, job_id)?;

        // Clean up pass log files
        let _ = std::fs::remove_file("ffmpeg2pass-0.log");
        let _ = std::fs::remove_file("ffmpeg2pass-0.log.mbtree");

        // Handle Done/Error from events
        for ev in &pass2_events {
            match ev {
                crate::ffmpeg::FfmpegEvent::Done(path) => {
                    discord_rpc::track_conversion();
                    discord_rpc::set_idle();
                    return Ok(ConversionDonePayload {
                        job_id: job_id.to_string(),
                        output_path: path.clone(),
                    });
                }
                crate::ffmpeg::FfmpegEvent::Error(msg) => {
                    discord_rpc::set_idle();
                    return Err(msg.clone());
                }
                _ => {}
            }
        }

        (pass2_events, None)
    } else {
        // ── Normal quality mode (existing behaviour) ──
        let args = build_args(&params);
        let norm_app = app_handle.clone();
        let norm_fname = file_name.clone();
        let norm_fmt = params.output_format.clone();
        let norm_jid = job_id.to_string();
        let norm_src = source_format.map(|s| s.to_string());
        let events = run_conversion(&args, effective_duration, move |ev| {
            match ev {
                crate::ffmpeg::FfmpegEvent::Progress(p) => {
                    discord_rpc::set_converting(&norm_fname, *p, &norm_fmt, norm_src.as_deref());
                    let _ = norm_app.emit(
                        "conversion-progress",
                        ConversionProgressPayload {
                            job_id: norm_jid.clone(),
                            progress: *p,
                        },
                    );
                    crate::queue::update_progress(&norm_app, &norm_jid, *p);
                }
                crate::ffmpeg::FfmpegEvent::Log(msg) => {
                    let _ = norm_app.emit(
                        "conversion-log",
                        ConversionLogPayload {
                            message: msg.clone(),
                        },
                    );
                }
                _ => {}
            }
        }, job_id)?;
        let output = events.iter().find_map(|ev| {
            if let crate::ffmpeg::FfmpegEvent::Done(path) = ev {
                Some(path.clone())
            } else {
                None
            }
        });
        (events, output)
    };

    // Handle Done/Error from events (Progress/Log already emitted via callback)
    for event in &events {
        match event {
            crate::ffmpeg::FfmpegEvent::Done(path) => {
                discord_rpc::track_conversion();
                discord_rpc::set_idle();
                return Ok(ConversionDonePayload {
                    job_id: job_id.to_string(),
                    output_path: path.clone(),
                });
            }
            crate::ffmpeg::FfmpegEvent::Error(msg) => {
                discord_rpc::set_idle();
                return Err(msg.clone());
            }
            _ => {}
        }
    }

    discord_rpc::set_idle();
    Err("Conversion produced no output".to_string())
}

#[tauri::command]
pub async fn detect_ffmpeg() -> bool {
    crate::ffmpeg::runner::detect_ffmpeg()
}

/// Concatenate multiple video clips into one. Uses the concat demuxer with
/// stream copy (-c copy) for a fast, re-encode-free join. All inputs must
/// share the same codecs/parameters; mismatches are surfaced as an error.
#[tauri::command]
pub async fn concat_videos(
    app_handle: tauri::AppHandle,
    inputs: Vec<String>,
    output_path: String,
    with_audio: bool,
) -> Result<ConversionDonePayload, String> {
    if inputs.len() < 2 {
        return Err("Need at least two clips to concatenate".to_string());
    }

    let job_id = crate::queue::register(
        &app_handle,
        crate::models::JobType::Concatenation,
        format!("Concatenating {} clips", inputs.len()),
        inputs.first().cloned().unwrap_or_default(),
        None,
    );

    // The concat pipeline (list-file build + ffmpeg run + result handling) is
    // fully blocking — run it on a blocking thread so the async executor stays
    // free for other IPC.
    let concat_app = app_handle.clone();
    let concat_jid = job_id.clone();
    let payload = tokio::task::spawn_blocking(move || -> Result<ConversionDonePayload, String> {
        std::fs::create_dir_all(
            std::path::Path::new(&output_path).parent().unwrap_or(std::path::Path::new(".")),
        )
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

        // Build the concat demuxer list file. Entries are single-quoted; any
        // embedded single quotes in the path are escaped per ffmpeg's convention.
        let list_path = std::env::temp_dir().join(format!("galdr_concat_{}.txt", uuid()));
        let mut list = String::new();
        for input in &inputs {
            let escaped = input.replace('\'', "'\\''");
            list.push_str(&format!("file '{}'\n", escaped));
        }
        std::fs::write(&list_path, &list)
            .map_err(|e| format!("Failed to write concat list: {}", e))?;

        // Total duration drives progress reporting.
        let total_duration: f64 = inputs
            .iter()
            .map(|p| probe_file(std::path::Path::new(p)).map(|i| i.duration).unwrap_or(0.0))
            .sum();

        let file_name = std::path::Path::new(&inputs[0])
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("concat")
            .to_string();
        discord_rpc::set_converting(&file_name, 0.0, "concat", None);

        let mut args: Vec<String> = vec![
            "-f".into(),
            "concat".into(),
            "-safe".into(),
            "0".into(),
            "-i".into(),
            list_path.to_string_lossy().to_string(),
            "-c".into(),
            "copy".into(),
        ];
        if !with_audio {
            args.push("-an".into());
        }
        args.push(output_path.clone());

        let cb_app = concat_app.clone();
        let cb_fname = file_name.clone();
        let cb_jid = concat_jid.clone();
        let events_res = run_conversion(&args, total_duration, move |ev| {
            match ev {
                crate::ffmpeg::FfmpegEvent::Progress(p) => {
                    discord_rpc::set_converting(&cb_fname, *p, "concat", None);
                    let _ = cb_app.emit(
                        "conversion-progress",
                        ConversionProgressPayload {
                            job_id: "concat".to_string(),
                            progress: *p,
                        },
                    );
                    crate::queue::update_progress(&cb_app, &cb_jid, *p);
                }
                crate::ffmpeg::FfmpegEvent::Log(msg) => {
                    let _ = cb_app.emit(
                        "conversion-log",
                        ConversionLogPayload { message: msg.clone() },
                    );
                }
                _ => {}
            }
        }, &concat_jid);
        let _ = std::fs::remove_file(&list_path);
        let events = events_res?;

        for event in &events {
            match event {
                crate::ffmpeg::FfmpegEvent::Done(path) => {
                    discord_rpc::track_conversion();
                    discord_rpc::set_idle();
                    crate::queue::complete(&concat_app, &concat_jid, Some(path.clone()), None);
                    return Ok(ConversionDonePayload {
                        job_id: "concat".to_string(),
                        output_path: path.clone(),
                    });
                }
                crate::ffmpeg::FfmpegEvent::Error(msg) => {
                    discord_rpc::set_idle();
                    crate::queue::fail(&concat_app, &concat_jid, msg.clone());
                    return Err(msg.clone());
                }
                _ => {}
            }
        }

        discord_rpc::set_idle();
        let err = "Concatenation produced no output".to_string();
        crate::queue::fail(&concat_app, &concat_jid, err.clone());
        Err(err)
    })
    .await
    .map_err(|e| format!("Concatenation task panicked: {}", e))?;

    payload
}

/// Extract the audio track from a media file into a standalone audio file.
#[tauri::command]
pub async fn extract_audio(
    app_handle: tauri::AppHandle,
    input_path: String,
    output_path: String,
    audio_format: String,
    bitrate: Option<String>,
) -> Result<ConversionDonePayload, String> {
    let job_id = crate::queue::register(
        &app_handle,
        crate::models::JobType::AudioExtraction,
        crate::queue::make_label(
            &crate::models::JobType::AudioExtraction,
            &input_path,
        ),
        input_path.clone(),
        None,
    );

    // The audio-extraction ffmpeg run is blocking; move it off the async
    // executor so other IPC stays responsive while it runs.
    let audio_app = app_handle.clone();
    let audio_jid = job_id.clone();
    tokio::task::spawn_blocking(move || -> Result<ConversionDonePayload, String> {
        std::fs::create_dir_all(
            std::path::Path::new(&output_path).parent().unwrap_or(std::path::Path::new(".")),
        )
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

        let duration = probe_file(std::path::Path::new(&input_path))
            .map(|info| info.duration)
            .unwrap_or(0.0);

        let file_name = std::path::Path::new(&input_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        discord_rpc::set_converting(&file_name, 0.0, &audio_format, None);

        // Pick the codec for the requested container.
        let codec = match audio_format.to_lowercase().as_str() {
            "mp3" => "libmp3lame",
            "aac" | "m4a" => "aac",
            "ogg" => "libvorbis",
            "opus" => "libopus",
            "flac" => "flac",
            "wav" => "pcm_s16le",
            _ => "libmp3lame",
        };

        let mut args: Vec<String> = vec![
            "-vn".into(),
            "-c:a".into(),
            codec.into(),
        ];
        if let Some(br) = &bitrate {
            args.push("-b:a".into());
            args.push(br.clone());
        }
        args.push(output_path.clone());

        let cb_app = audio_app.clone();
        let cb_fname = file_name.clone();
        let cb_fmt = audio_format.clone();
        let cb_jid = audio_jid.clone();
        let events = run_conversion(&args, duration, move |ev| {
            match ev {
                crate::ffmpeg::FfmpegEvent::Progress(p) => {
                    discord_rpc::set_converting(&cb_fname, *p, &cb_fmt, None);
                    let _ = cb_app.emit(
                        "conversion-progress",
                        ConversionProgressPayload {
                            job_id: "extract-audio".to_string(),
                            progress: *p,
                        },
                    );
                    crate::queue::update_progress(&cb_app, &cb_jid, *p);
                }
                crate::ffmpeg::FfmpegEvent::Log(msg) => {
                    let _ = cb_app.emit(
                        "conversion-log",
                        ConversionLogPayload { message: msg.clone() },
                    );
                }
                _ => {}
            }
        }, &audio_jid)?;

        for event in &events {
            match event {
                crate::ffmpeg::FfmpegEvent::Done(path) => {
                    discord_rpc::track_conversion();
                    discord_rpc::set_idle();
                    crate::queue::complete(&audio_app, &audio_jid, Some(path.clone()), None);
                    return Ok(ConversionDonePayload {
                        job_id: "extract-audio".to_string(),
                        output_path: path.clone(),
                    });
                }
                crate::ffmpeg::FfmpegEvent::Error(msg) => {
                    discord_rpc::set_idle();
                    crate::queue::fail(&audio_app, &audio_jid, msg.clone());
                    return Err(msg.clone());
                }
                _ => {}
            }
        }

        discord_rpc::set_idle();
        let err = "Audio extraction produced no output".to_string();
        crate::queue::fail(&audio_app, &audio_jid, err.clone());
        Err(err)
    })
    .await
    .map_err(|e| format!("Audio extraction task panicked: {}", e))?
}

/// Cheap unique suffix for the temp concat list filename.
fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

#[tauri::command]
pub fn get_default_output_dir() -> Result<String, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Could not find home directory".to_string())?;
    let dir = std::path::Path::new(&home)
        .join("Desktop")
        .join("galdr-output");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn is_directory(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

#[tauri::command]
pub fn scan_directory(dir: PathBuf, extension: String) -> Result<Vec<ScannedFile>, String> {
    let ext_filter = if extension.is_empty() {
        None
    } else {
        Some(extension.trim_start_matches('.').to_lowercase())
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());
            let match_ext = match (&ext_filter, &ext) {
                (None, _) => true,
                (Some(ref filter), Some(ref e)) => e == filter,
                _ => false,
            };
            if match_ext {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let size = std::fs::metadata(&path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                files.push(ScannedFile {
                    path: path.to_string_lossy().to_string(),
                    name,
                    size,
                });
            }
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

#[derive(Clone, serde::Serialize)]
pub struct BatchProgressPayload {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub current_file: String,
    pub file_progress: f64,
}

#[tauri::command]
pub async fn start_batch_conversion(
    app_handle: tauri::AppHandle,
    params: BatchConversionParams,
) -> Result<String, String> {
    let extension = params.input_extension.trim_start_matches('.').to_lowercase();
    let mut entries: Vec<PathBuf> = Vec::new();

    for entry in std::fs::read_dir(&params.input_dir)
        .map_err(|e| format!("Failed to read input directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                if ext == extension {
                    entries.push(path);
                }
            }
        }
    }
    entries.sort();

    let total = entries.len();
    std::fs::create_dir_all(&params.output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    let job_id = crate::queue::register(
        &app_handle,
        crate::models::JobType::BatchConversion,
        format!("Batch: {} files (extension: {})", entries.len() - params.skip, extension),
        params.input_dir.to_string_lossy().to_string(),
        None,
    );

    // Acquire a per-job cancel token so `cancel_conversion` can flip exactly
    // this batch's flag instead of the old process-wide CANCELLED AtomicBool.
    let _ = crate::queue::cancel_token::acquire(&job_id);

    // The per-file ffmpeg loop blocks for the whole batch. Run it on a blocking
    // thread so the async executor (and other IPC) stays responsive throughout.
    let batch_app = app_handle.clone();
    let batch_jid = job_id.clone();
    let (done, failed) = tokio::task::spawn_blocking(move || -> Result<(usize, usize), String> {
        let mut done = 0usize;
        let mut failed = 0usize;
        let done_offset = params.skip;

        for input_path in entries.iter().skip(params.skip) {
            if crate::queue::pids::is_cancelled(&batch_jid) {
                break;
            }
            let file_name = input_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Update queue progress for the batch job
            let batch_progress = (done + failed) as f64 / total as f64;
            crate::queue::update_progress(&batch_app, &batch_jid, batch_progress);

            let _ = batch_app.emit(
                "batch-progress",
                BatchProgressPayload {
                    total,
                    done: done + done_offset,
                    failed,
                    current_file: file_name.clone(),
                    file_progress: 0.0,
                },
            );

            discord_rpc::set_batch(&file_name, done + done_offset + 1, total, 0.0);

            let duration = probe_file(input_path)
                .map(|info| info.duration)
                .unwrap_or(0.0);

            let single_params = ConversionParams {
                input_path: input_path.clone(),
                output_dir: params.output_dir.clone(),
                output_format: params.output_format.clone(),
                target_size_bytes: params.target_size_bytes,
                ..Default::default()
            };

            let is_target_size = single_params.target_size_bytes.is_some();

            let (events, _output_path) = if is_target_size {
                // ── Target-size: two-pass per file ──
                // Compute effective duration for this file
                let trim_start = 0.0;
                let trim_end = duration;
                let eff_dur = (trim_end - trim_start).max(1.0);

                let (pass1, pass2) = build_two_pass_args(&single_params, eff_dur)?;

                if !pass1.is_empty() {
                    let _ = batch_app.emit(
                        "batch-log",
                        ConversionLogPayload {
                            message: format!("  {} — pass 1/2 analysis", file_name),
                        },
                    );
                    let pass1_result: Result<(), String> = (|| {
                        let evts = run_conversion(&pass1, eff_dur, |_| {}, &batch_jid)?;
                        for e in &evts {
                            if let crate::ffmpeg::FfmpegEvent::Error(msg) = e {
                                return Err(msg.clone());
                            }
                        }
                        Ok(())
                    })();
                    if let Err(e) = pass1_result {
                        // Clean up pass log files
                        let _ = std::fs::remove_file("ffmpeg2pass-0.log");
                        let _ = std::fs::remove_file("ffmpeg2pass-0.log.mbtree");
                        return Err(e);
                    }
                }

                let result: Result<(Vec<crate::ffmpeg::FfmpegEvent>, Option<String>), String> = (|| {
                    let pass2_cb_app = batch_app.clone();
                    let pass2_cb_fname = file_name.clone();
                    let evts = run_conversion(&pass2, eff_dur, move |ev| {
                        if let crate::ffmpeg::FfmpegEvent::Progress(p) = ev {
                            let _ = pass2_cb_app.emit(
                                "batch-progress",
                                BatchProgressPayload {
                                    total,
                                    done: done + done_offset,
                                    failed,
                                    current_file: pass2_cb_fname.clone(),
                                    file_progress: *p,
                                },
                            );
                        }
                    }, &batch_jid)?;
                    let out = evts.iter().find_map(|e| {
                        if let crate::ffmpeg::FfmpegEvent::Done(p) = e {
                            Some(p.clone())
                        } else {
                            None
                        }
                    });
                    Ok((evts, out))
                })();

                // Clean up pass log files
                let _ = std::fs::remove_file("ffmpeg2pass-0.log");
                let _ = std::fs::remove_file("ffmpeg2pass-0.log.mbtree");

                result.unwrap_or_else(|e| (vec![crate::ffmpeg::FfmpegEvent::Error(e.clone())], None))
            } else {
                // ── Normal quality mode ──
                let args = build_args(&single_params);
                let result: Result<(Vec<crate::ffmpeg::FfmpegEvent>, Option<String>), String> = (|| {
                    let norm_cb_app = batch_app.clone();
                    let norm_cb_fname = file_name.clone();
                    let evts = run_conversion(&args, duration, move |ev| {
                        if let crate::ffmpeg::FfmpegEvent::Progress(p) = ev {
                            let _ = norm_cb_app.emit(
                                "batch-progress",
                                BatchProgressPayload {
                                    total,
                                    done: done + done_offset,
                                    failed,
                                    current_file: norm_cb_fname.clone(),
                                    file_progress: *p,
                                },
                            );
                        }
                    }, &batch_jid)?;
                    let out = evts.iter().find_map(|e| {
                        if let crate::ffmpeg::FfmpegEvent::Done(p) = e {
                            Some(p.clone())
                        } else {
                            None
                        }
                    });
                    Ok((evts, out))
                })();
                result.unwrap_or_else(|e| (vec![crate::ffmpeg::FfmpegEvent::Error(e.clone())], None))
            };

            let result: Result<(), String> = (|| {
                for event in &events {
                    match event {
                        crate::ffmpeg::FfmpegEvent::Done(_) => return Ok(()),
                        crate::ffmpeg::FfmpegEvent::Error(msg) => return Err(msg.clone()),
                        _ => {}
                    }
                }
                Ok(())
            })();

            match result {
                Ok(_) => {
                    done += 1;
                    discord_rpc::track_conversion();
                }
                Err(e) => {
                    failed += 1;
                    let _ = batch_app.emit(
                        "batch-progress",
                        BatchProgressPayload {
                            total,
                            done: done + done_offset,
                            failed,
                            current_file: format!("{} — {}", file_name, e),
                            file_progress: 0.0,
                        },
                    );
                }
            }
        }

        discord_rpc::set_idle();

        Ok((done, failed))
    })
    .await
    .map_err(|e| format!("Batch task panicked: {}", e))??;

    let done_offset = params.skip;

    let _ = app_handle.emit(
        "batch-progress",
        BatchProgressPayload {
            total,
            done: done + done_offset,
            failed,
            current_file: String::new(),
            file_progress: 1.0,
        },
    );

    // Complete the batch job with result summary
    if failed > 0 && done == 0 {
        crate::queue::fail(
            &app_handle,
            &job_id,
            format!("All {} files failed", failed),
        );
    } else {
        let summary = serde_json::json!({
            "total": total,
            "done": done,
            "failed": failed,
            "skipped": done_offset,
        });
        crate::queue::complete(&app_handle, &job_id, None, Some(summary));
    }

    Ok(job_id)
}

#[tauri::command]
pub fn update_discord_presence(page: String, forge_clips: Option<usize>, forge_duration: Option<f64>) {
    match page.as_str() {
        "home" => discord_rpc::set_idle(),
        "forge" => {
            if let (Some(clips), Some(dur)) = (forge_clips, forge_duration) {
                discord_rpc::set_forge_editing(clips, dur);
            } else {
                discord_rpc::set_browsing("forge");
            }
        }
        _ => discord_rpc::set_browsing(&page),
    }
}

#[tauri::command]
pub fn update_forge_presence(clips: usize, duration_secs: f64) {
    discord_rpc::set_forge_editing(clips, duration_secs);
}

#[tauri::command]
pub fn set_discord_enabled(enabled: bool) {
    discord_rpc::set_enabled(enabled);
}

#[tauri::command]
pub fn cancel_conversion(job_id: String) -> Result<(), String> {
    // Kill only this conversion's ffmpeg child (pid-scoped, not image-scoped)
    // and flip only this job's cancellation token so polling loops bail out.
    let _ = crate::queue::pids::kill_job(&job_id);
    crate::queue::cancel_token::set(&job_id);
    Ok(())
}
