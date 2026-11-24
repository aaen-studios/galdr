//! GAL-2: galdr's Calx core linked into a GPUI app.
//!
//! Convert runs one ffmpeg job in the core (a 720p H.264 copy of the input) and shows its progress;
//! Cancel stops it; Panic makes the core hit a bug, and the app starts it again.
//!
//!   core-link <input> [--background] [--auto]
//!
//! `--auto` runs a script instead of waiting for clicks: cancel a job, finish one, fail one, panic,
//! restart, finish one, then prints what happened and quits. `--background` opens the window
//! off-screen without taking focus.



use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, relative, rgb, size,
};
use gpui_platform::application;
use serde_json::{Value, json};

use galdr_core::{Core, Message};

#[derive(Clone)]
struct Job {
    id: u64,
    label: String,
    state: String,
    done: f64,
    duration: Option<f64>,
    speed: Option<f64>,
    note: String,
    sent: Instant,
}

/// Steps of `--auto`, each waiting for an event.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    CancelOne,
    FinishOne,
    FailOne,
    Panic,
    AfterRestart,
    Done,
}

struct App2 {
    input: PathBuf,
    out_dir: PathBuf,
    core: Core,
    events: Receiver<Message>,
    jobs: Vec<Job>,
    next_id: u64,
    log: Vec<String>,
    started: Instant,
    auto: Option<Step>,
    report: Vec<String>,
    frames: u64,
}

impl App2 {
    fn new(input: PathBuf, auto: bool) -> Self {
        let (tx, events) = mpsc::channel();
        let out_dir = std::env::temp_dir().join("galdr-core-link");
        let _ = std::fs::create_dir_all(&out_dir);
        let mut app = Self {
            input,
            out_dir,
            core: Core::start(tx),
            events,
            jobs: Vec::new(),
            next_id: 1,
            log: Vec::new(),
            started: Instant::now(),
            auto: auto.then_some(Step::CancelOne),
            report: Vec::new(),
            frames: 0,
        };
        if auto {
            app.convert("full file, to cancel", &[], None);
        }
        app
    }

    fn convert(&mut self, label: &str, extra: &[&str], input: Option<&str>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let output = self.out_dir.join(format!("job-{id}.mp4"));
        let mut args: Vec<&str> = extra.to_vec();
        args.extend(["-vf", "scale=-2:720", "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac"]);
        let request = json!({
            "cmd": "convert",
            "id": id,
            "input": input.map(String::from).unwrap_or_else(|| self.input.display().to_string()),
            "output": output.display().to_string(),
            "args": args,
        });
        let sent = self.core.send(&request);
        self.jobs.push(Job {
            id,
            label: label.to_string(),
            state: if sent { "sent".into() } else { "refused".into() },
            done: 0.0,
            duration: None,
            speed: None,
            note: String::new(),
            sent: Instant::now(),
        });
        id
    }

    fn cancel(&mut self, id: u64) {
        self.core.send(&json!({ "cmd": "cancel", "id": id }));
    }

    fn panic_core(&mut self) {
        self.core.send(&json!({ "cmd": "panic" }));
    }

    fn running(&self) -> Option<u64> {
        self.jobs.iter().rev().find(|j| j.state == "started" || j.state == "progress").map(|j| j.id)
    }

    fn say(&mut self, line: String) {
        let t = self.started.elapsed().as_secs_f64();
        let line = format!("{t:6.2}s {line}");
        if self.auto.is_some() {
            println!("{line}");
        }
        self.log.push(line);
        if self.log.len() > 14 {
            self.log.remove(0);
        }
    }

    fn drain(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        while let Ok(message) = self.events.try_recv() {
            changed = true;
            match message {
                Message::Event(v) => self.on_event(v),
                Message::Panic { message, location, function, trace } => {
                    self.core.mark_dead();
                    let place = function.unwrap_or_else(|| "the core's thread".into());
                    self.say(format!("core panicked in {place}: {message} at {location}"));
                    for line in trace.lines().take(3) {
                        self.say(format!("    {line}"));
                    }
                    let code = self.core.restart();
                    self.say(format!("restarted the core (calx_shutdown gave {code})"));
                    if self.auto == Some(Step::Panic) {
                        self.report.push(format!("panic on the core's thread reported, restart gave {code}"));
                        self.auto = Some(Step::AfterRestart);
                        self.convert("2s after restart", &["-t", "2"], None);
                    }
                }
            }
        }
        if changed {
            cx.notify();
        }
        if self.auto == Some(Step::Done) {
            self.auto = None;
            println!("\nsummary:");
            for line in &self.report {
                println!("  {line}");
            }
            let secs = self.started.elapsed().as_secs_f64();
            println!("  ui drew {:.0} frames a second while it ran", self.frames as f64 / secs);
            cx.quit();
        }
    }

    fn on_event(&mut self, v: Value) {
        let kind = v["event"].as_str().unwrap_or("?").to_string();
        let id = v["id"].as_u64().unwrap_or(0);
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) else {
            self.say(format!("event for unknown job: {v}"));
            return;
        };
        let since = job.sent.elapsed().as_secs_f64() * 1000.0;
        if kind == "progress" {
            job.done = v["done"].as_f64().unwrap_or(job.done);
            job.duration = v["duration"].as_f64().or(job.duration);
            job.speed = v["speed"].as_f64();
        } else {
            if let Some(d) = v["duration"].as_f64() {
                job.duration = Some(d);
            }
            if let Some(e) = v["error"].as_str() {
                job.note = e.lines().last().unwrap_or(e).to_string();
            }
        }
        job.state = kind.clone();
        let (label, done, note) = (job.label.clone(), job.done, job.note.clone());
        if kind != "progress" {
            self.say(format!("job {id} ({label}) {kind} after {since:.0} ms {note}"));
        }

        match (self.auto, kind.as_str()) {
            (Some(Step::CancelOne), "progress") if done > 2.0 => {
                self.say(format!("cancelling job {id} at {done:.1}s"));
                self.cancel(id);
                if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                    j.sent = Instant::now(); // time the cancel from here
                }
            }
            (Some(Step::CancelOne), "canceled") => {
                self.report.push(format!("job {id} canceled {since:.0} ms after asking"));
                self.auto = Some(Step::FinishOne);
                self.convert("5s clip", &["-t", "5"], None);
            }
            (Some(Step::FinishOne), "finished") => {
                let path = self.out_dir.join(format!("job-{id}.mp4"));
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                self.report.push(format!("job {id} finished in {since:.0} ms, {} KB written", size / 1024));
                self.auto = Some(Step::FailOne);
                self.convert("missing input", &[], Some("does-not-exist.mp4"));
            }
            (Some(Step::FailOne), "failed") => {
                self.report.push(format!("job {id} failed as it should: {note}"));
                self.auto = Some(Step::Panic);
                self.panic_core();
            }
            (Some(Step::AfterRestart), "finished") => {
                self.report.push(format!("job {id} finished on the restarted core in {since:.0} ms"));
                self.auto = Some(Step::Done);
            }
            (Some(_), "failed") => {
                self.report.push(format!("job {id} failed unexpectedly: {note}"));
                self.auto = Some(Step::Done);
            }
            _ => {}
        }
    }
}

fn button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .border_1()
        .border_color(rgb(0xf2f2f2))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x222222)))
        .child(label)
}

impl Render for App2 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frames += 1;
        if self.auto.is_some() || self.running().is_some() {
            window.request_animation_frame();
        }
        let alive = self.core.alive();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(0x000000))
            .text_color(rgb(0xf2f2f2))
            .font_family("Consolas")
            .text_size(px(13.))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("convert", "Convert").on_click(cx.listener(|this, _, _, cx| {
                        this.convert("720p copy", &[], None);
                        cx.notify();
                    })))
                    .child(button("cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                        if let Some(id) = this.running() {
                            this.cancel(id);
                        }
                        cx.notify();
                    })))
                    .child(button("panic", "Panic").on_click(cx.listener(|this, _, _, cx| {
                        this.panic_core();
                        cx.notify();
                    })))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_color(if alive { rgb(0x8a8a8a) } else { rgb(0xff6b6b) })
                            .child(if alive { "core running" } else { "core stopped" }),
                    ),
            )
            .children(self.jobs.iter().rev().take(6).map(|j| {
                let frac = match j.duration {
                    Some(d) if d > 0.0 => (j.done / d).clamp(0.0, 1.0) as f32,
                    _ => if j.state == "finished" { 1.0 } else { 0.0 },
                };
                let speed = j.speed.map(|s| format!(" {s:.1}x")).unwrap_or_default();
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(SharedString::from(format!(
                        "#{} {}  {}  {:.1}/{}s{}  {}",
                        j.id,
                        j.label,
                        j.state,
                        j.done,
                        j.duration.map(|d| format!("{d:.1}")).unwrap_or("?".into()),
                        speed,
                        j.note
                    )))
                    .child(
                        div()
                            .h(px(6.))
                            .w_full()
                            .bg(rgb(0x1a1a1a))
                            .child(div().h_full().w(relative(frac)).bg(rgb(0xf2f2f2))),
                    )
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_color(rgb(0x8a8a8a))
                    .children(self.log.iter().map(|l| SharedString::from(l.clone()))),
            )
    }
}

fn main() -> anyhow::Result<()> {
    let mut input = None;
    let (mut background, mut auto) = (false, false);
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--background" => background = true,
            "--auto" => auto = true,
            _ => input = Some(PathBuf::from(a)),
        }
    }
    let input = input.ok_or_else(|| anyhow::anyhow!("usage: core-link <input> [--background] [--auto]"))?;
    let mut start = Some(input);
    application().run(move |cx: &mut App| {
        let input = start.take().unwrap();
        let window_size = size(px(900.), px(560.));
        let bounds = if background {
            Bounds { origin: point(px(-6000.), px(200.)), size: window_size }
        } else {
            Bounds::centered(None, window_size, cx)
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: !background,
                inactive_frame_interval: None,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    // Events arrive on the core's thread; pick them up every 16 ms.
                    cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor().timer(Duration::from_millis(16)).await;
                            if this.update(cx, |app: &mut App2, cx| app.drain(cx)).is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    App2::new(input, auto)
                })
            },
        )
        .expect("open window");
        if !background {
            cx.activate(true);
        }
    });
    Ok(())
}
