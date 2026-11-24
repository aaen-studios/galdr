//! `--auto`: a scripted edit, to test the app without anyone at the keyboard. It needs two files:
//! the first at least 6 s long, the second at least 6 s.

use std::time::Instant;

use gpui::Context;

use crate::Galdr;
use crate::jobs::{Job, Kind, State};
use crate::preview::Mode;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    WaitForMedia,
    Edit,
    Play,
    Scrub,
    Jog,
    Export,
    Tools,
    Save,
    Open,
    Done,
}

pub struct Script {
    step: Step,
    step_started: Instant,
    started: Instant,
    seeks: usize,
    rng: u64,
    report: Vec<String>,
    /// The track when it was saved, to compare with what opens.
    saved_clips: Vec<(f64, f64)>,
}

impl Script {
    pub fn new() -> Self {
        Self {
            step: Step::WaitForMedia,
            step_started: Instant::now(),
            started: Instant::now(),
            seeks: 0,
            rng: 0x2545_f491_4f6c_dd1d,
            report: Vec::new(),
            saved_clips: Vec::new(),
        }
    }

    fn next(&mut self, step: Step) {
        self.step = step;
        self.step_started = Instant::now();
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * p).round() as usize]
}

fn avg(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

fn say(script: &mut Script, line: String) {
    println!("{:6.2}s {line}", script.started.elapsed().as_secs_f64());
    script.report.push(line);
}

pub fn step(app: &mut Galdr, cx: &mut Context<Galdr>) {
    let Some(mut s) = app.auto.take() else { return };
    let in_step = s.step_started.elapsed().as_secs_f64();
    match s.step {
        Step::WaitForMedia => {
            let ready = app.project.media.len() >= 2 && app.project.media.iter().all(|m| m.info.is_some());
            if let Some(m) = app.project.media.iter().find(|m| m.error.is_some()) {
                say(&mut s, format!("can't read {}: {}", m.name, m.error.as_deref().unwrap_or("")));
                s.next(Step::Done);
            } else if ready {
                say(&mut s, format!("read {} files in {:.0} ms", app.project.media.len(), in_step * 1000.0));
                s.next(Step::Edit);
            } else if in_step > 10.0 {
                say(&mut s, "files didn't load in 10 s; pass two video files".into());
                s.next(Step::Done);
            }
        }
        Step::Edit => {
            let (a, b) = (app.project.media[0].id, app.project.media[1].id);
            app.append(a);
            app.append(b);
            // Cut the first clip at 5 s and drop the rest of it; keep 2–6 s of the second.
            app.preview.playhead = 5.0;
            app.split();
            app.delete_selected();
            let second = app.project.clips[1].id;
            app.project.trim_in(second, 2.0);
            app.project.trim_out(second, 6.0);
            app.edited();
            // Undo back to an empty track, and redo it all.
            let before = app.project.clips.clone();
            let mut undos = 0;
            while app.project.can_undo() {
                app.undo();
                undos += 1;
            }
            let emptied = app.project.clips.is_empty();
            while app.project.can_redo() {
                app.redo();
            }
            say(
                &mut s,
                format!(
                    "undo: {undos} steps back to {}, redo {}",
                    if emptied { "an empty track" } else { "a track that ISN'T empty" },
                    if app.project.clips == before { "brings it all back" } else { "DOESN'T bring it back" }
                ),
            );
            let lengths: Vec<String> = app.project.clips.iter().map(|c| format!("{:.2}", c.length())).collect();
            say(&mut s, format!("timeline: {} clips [{}], {:.2}s", app.project.clips.len(), lengths.join(", "), app.project.duration()));
            app.preview.play(&app.project, 3.0);
            s.next(Step::Play);
        }
        Step::Play => {
            if app.preview.mode != Mode::Playing || app.preview.playhead >= 8.5 || in_step > 10.0 {
                let st = &app.preview.stats;
                say(
                    &mut s,
                    format!(
                        "played 3.0s → {:.2}s across the cut: {} frames, gap avg {:.1} p99 {:.1} max {:.1} ms, at the cut {:.1} ms, {} redraws without their frame",
                        app.preview.playhead,
                        st.gaps.len() + 1,
                        avg(&st.gaps),
                        pct(&st.gaps, 0.99),
                        pct(&st.gaps, 1.0),
                        st.cut_gaps.first().copied().unwrap_or(f64::NAN),
                        st.late,
                    ),
                );
                let audio = app.preview.audio();
                say(
                    &mut s,
                    format!(
                        "sound ({} output): {:.2}s of timeline in {:.2}s, {} underruns",
                        if audio.silent() { "silent" } else { "device" },
                        app.preview.playhead - 3.0,
                        in_step,
                        audio.underruns()
                    ),
                );
                app.preview.pause(&app.project);
                app.preview.stats = Default::default();
                s.next(Step::Scrub);
            }
        }
        Step::Scrub => {
            let due = (in_step / 0.2) as usize + 1;
            if due > s.seeks && s.seeks < 20 {
                s.seeks += 1;
                let t = s.random() * app.project.duration();
                app.preview.seek(&app.project, t, false);
            }
            if in_step > 4.5 {
                let st = &app.preview.stats;
                say(
                    &mut s,
                    format!(
                        "20 random seeks: first frame avg {:.1} p95 {:.1} ms, exact avg {:.1} p95 {:.1} max {:.1} ms, cache hits {}",
                        avg(&st.first),
                        pct(&st.first, 0.95),
                        avg(&st.exact),
                        pct(&st.exact, 0.95),
                        pct(&st.exact, 1.0),
                        st.cache_hits
                    ),
                );
                app.preview.stats = Default::default();
                s.next(Step::Jog);
            }
        }
        Step::Jog => {
            // Back and forth over the cut at 5 s, once a second.
            let phase = in_step % 1.0;
            let along = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
            app.preview.seek(&app.project, 4.0 + 2.0 * along, true);
            if in_step > 3.0 {
                let st = &app.preview.stats;
                say(
                    &mut s,
                    format!(
                        "jogged over the cut for 3s: exact frame under the playhead on {} of {} redraws",
                        st.scrub_exact,
                        st.scrub_ticks
                    ),
                );
                let out = std::env::temp_dir().join("galdr-auto-export.mp4");
                let _ = std::fs::remove_file(&out);
                app.start_export(out);
                s.next(Step::Export);
            }
        }
        Step::Export => {
            let job = app.jobs.latest(Kind::Export);
            if job.is_none_or(|j| !j.active()) || in_step > 60.0 {
                let line = job_report(job, in_step);
                say(&mut s, format!("export {line}"));
                // Then the tools, into the temp folder: a compress and a convert of the first
                // file, and with GALDR_AUTO_FORMATS set, the export in every format.
                let input = app.project.media[0].path.clone();
                let dir = std::env::temp_dir();
                let compressed = dir.join("galdr-auto-compressed.mp4");
                let converted = dir.join("galdr-auto-converted.mp3");
                let _ = std::fs::remove_file(&compressed);
                let _ = std::fs::remove_file(&converted);
                app.start_compress(&input, compressed, "small", 720, 2.0);
                app.start_convert(&input, converted, "mp3");
                if std::env::var_os("GALDR_AUTO_FORMATS").is_some()
                    && let Some(export) = app.jobs.latest(Kind::Export).map(|j| j.output.clone())
                {
                    for (format, _, ext) in crate::tools::FORMATS {
                        let out = dir.join(format!("galdr-auto-format-{format}.{ext}"));
                        let _ = std::fs::remove_file(&out);
                        app.start_convert(&export, out, format);
                    }
                }
                s.next(Step::Tools);
            }
        }
        Step::Tools => {
            let busy = app.jobs.any_active(Kind::Compress) || app.jobs.any_active(Kind::Convert);
            if !busy || in_step > 300.0 {
                let jobs: Vec<&Job> =
                    app.jobs.list.iter().filter(|j| matches!(j.kind, Kind::Compress | Kind::Convert)).collect();
                let lines: Vec<String> = jobs
                    .iter()
                    .map(|j| {
                        let size = std::fs::metadata(&j.output)
                            .map(|m| format!(", {:.2} MB", m.len() as f64 / 1_048_576.0))
                            .unwrap_or_default();
                        format!(
                            "{} → {} {}{size}",
                            format!("{:?}", j.kind).to_lowercase(),
                            j.output_name(),
                            job_report(Some(j), in_step)
                        )
                    })
                    .collect();
                for line in lines {
                    say(&mut s, line);
                }
                // Save the project, start a new one, and open it again.
                let path = std::env::temp_dir().join("galdr-auto.galdr");
                let _ = std::fs::remove_file(&path);
                s.saved_clips = app.project.clips.iter().map(|c| (c.source_in, c.source_out)).collect();
                app.write_project(path);
                s.next(Step::Save);
            }
        }
        Step::Save => {
            if !app.dirty() || in_step > 10.0 {
                let saved = !app.dirty() && app.project_path.is_some();
                let path = std::env::temp_dir().join("galdr-auto.galdr");
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                say(&mut s, format!("save: {} ({size} bytes)", if saved { "saved" } else { "FAILED" }));
                app.new_project();
                let emptied = app.project.clips.is_empty() && app.project.media.is_empty();
                say(&mut s, format!("new: {}", if emptied { "empty" } else { "NOT EMPTY" }));
                app.open_project(path);
                s.next(Step::Open);
            }
        }
        Step::Open => {
            let loaded =
                !app.project.media.is_empty() && app.project.media.iter().all(|m| m.info.is_some() || m.error.is_some());
            if loaded || in_step > 10.0 {
                let clips: Vec<(f64, f64)> = app.project.clips.iter().map(|c| (c.source_in, c.source_out)).collect();
                let same = clips == s.saved_clips;
                say(
                    &mut s,
                    format!(
                        "open: {} files, {} clips, {} the saved track, named {}, {}",
                        app.project.media.len(),
                        clips.len(),
                        if same { "same as" } else { "DIFFERENT FROM" },
                        app.project_name(),
                        if app.dirty() { "CHANGED" } else { "unchanged" }
                    ),
                );
                s.next(Step::Done);
            }
        }
        Step::Done => {
            println!("\nsummary:");
            for line in &s.report {
                println!("  {line}");
            }
            cx.quit();
            return;
        }
    }
    app.auto = Some(s);
}

/// How a job went, and what ffprobe says its output is.
fn job_report(job: Option<&Job>, seconds: f64) -> String {
    let Some(j) = job else { return "never started".into() };
    let probe = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration:stream=width,height,codec_name", "-of", "compact=p=0"])
        .arg(&j.output)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).replace('\n', " ").trim().to_string())
        .unwrap_or_default();
    let state = match j.state {
        State::Finished => "finished",
        State::Failed => "FAILED",
        State::Canceled => "canceled",
        State::Queued | State::Running => "STILL RUNNING",
    };
    let error = j.error.as_ref().map(|e| format!(" ({e})")).unwrap_or_default();
    format!("{state} in {seconds:.1}s{error}: {probe}")
}
