//! `--auto`: a scripted edit, to test the app without anyone at the keyboard. It needs two files:
//! the first at least 6 s long, the second at least 6 s.

use std::time::Instant;

use gpui::Context;

use crate::Galdr;
use crate::preview::Mode;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    WaitForMedia,
    Edit,
    Play,
    Scrub,
    Jog,
    Export,
    Done,
}

pub struct Script {
    step: Step,
    step_started: Instant,
    started: Instant,
    seeks: usize,
    rng: u64,
    report: Vec<String>,
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
                        "played 3.0s → {:.2}s across the cut: {} frames, gap avg {:.1} p99 {:.1} max {:.1} ms, at the cut {:.1} ms",
                        app.preview.playhead,
                        st.gaps.len() + 1,
                        avg(&st.gaps),
                        pct(&st.gaps, 0.99),
                        pct(&st.gaps, 1.0),
                        st.cut_gaps.first().copied().unwrap_or(f64::NAN),
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
                app.preview.scrub(&app.project, t, true);
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
            app.preview.scrub(&app.project, 4.0 + 2.0 * along, false);
            if in_step > 3.0 {
                let st = &app.preview.stats;
                say(
                    &mut s,
                    format!(
                        "jogged over the cut for 3s: {} positions, first frame avg {:.1} p95 {:.1} ms, cache hits {}",
                        st.first.len(),
                        avg(&st.first),
                        pct(&st.first, 0.95),
                        st.cache_hits
                    ),
                );
                let out = std::env::temp_dir().join("galdr-auto-export.mp4");
                let _ = std::fs::remove_file(&out);
                app.start_export(out);
                s.next(Step::Export);
            }
        }
        Step::Export => {
            let state = app.export.as_ref().map(|e| e.state.clone()).unwrap_or_default();
            if state == "finished" || state == "failed" || in_step > 60.0 {
                let e = app.export.as_ref();
                let output = e.map(|e| e.output.clone()).unwrap_or_default();
                let probe = std::process::Command::new("ffprobe")
                    .args(["-v", "error", "-show_entries", "format=duration:stream=width,height,codec_type", "-of", "compact=p=0"])
                    .arg(&output)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).replace('\n', " ").trim().to_string())
                    .unwrap_or_default();
                say(
                    &mut s,
                    format!(
                        "export {state} in {:.1}s{}: {probe}",
                        in_step,
                        e.and_then(|e| e.error.clone()).map(|x| format!(" ({x})")).unwrap_or_default()
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
