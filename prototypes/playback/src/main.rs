//! GAL-1: can GPUI show video smoothly on Windows?
//!
//! Plays a file with audio and lets you scrub it. Frames come from the ffmpeg CLI as BGRA and are
//! drawn as a new `RenderImage` each frame; the old one is dropped from the GPU atlas.
//!
//!   playback <file> [--height 720] [--no-hwaccel] [--bench]
//!
//! Space plays and pauses, left and right jump 5 s, drag the bar to scrub. `--bench` runs a fixed
//! script (play, random seeks, a drag, play) and prints timings, then quits.

mod audio;
mod decode;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use gpui::{
    App, Bounds, Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ObjectFit, RenderImage, Window, WindowBounds, WindowOptions, div, img,
    prelude::*, px, relative, rgb, size,
};
use gpui_platform::application;

use audio::AudioOut;
use decode::{Frame, Info, VideoRequest};

struct Args {
    path: PathBuf,
    height: Option<u32>,
    hwaccel: bool,
    bench: bool,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut out = Args { path: PathBuf::new(), height: None, hwaccel: true, bench: false };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--height" => out.height = Some(args.next().unwrap_or_default().parse()?),
            "--no-hwaccel" => out.hwaccel = false,
            "--bench" => out.bench = true,
            _ => out.path = PathBuf::from(a),
        }
    }
    anyhow::ensure!(!out.path.as_os_str().is_empty(), "usage: playback <file> [--height N] [--no-hwaccel] [--bench]");
    Ok(out)
}

/// Timings gathered while playing, reset each second for the overlay and kept whole for the bench.
#[derive(Default)]
struct Stats {
    shown: u64,
    dropped: u64,
    renders: u64,
    /// Time between consecutive frames shown while playing, in ms.
    gaps: Vec<f64>,
    /// Time from asking for a frame at a new position to drawing it, in ms.
    seek_latency: Vec<f64>,
    /// Time from a frame leaving ffmpeg to being drawn, in ms.
    queue_age: Vec<f64>,
    /// Time spent in `render`, in ms.
    render_ms: Vec<f64>,
}

impl Stats {
    fn summary(&self, label: &str, secs: f64) -> String {
        fn pct(v: &[f64], p: f64) -> f64 {
            if v.is_empty() {
                return 0.0;
            }
            let mut s = v.to_vec();
            s.sort_by(f64::total_cmp);
            s[((s.len() - 1) as f64 * p).round() as usize]
        }
        let avg = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
        format!(
            "{label}: {:.1} fps shown, {} dropped, render {:.1} fps | gap avg {:.1} p99 {:.1} max {:.1} ms | \
             seek n={} avg {:.0} p95 {:.0} max {:.0} ms | queue age avg {:.1} ms | render avg {:.2} p99 {:.2} ms",
            self.shown as f64 / secs,
            self.dropped,
            self.renders as f64 / secs,
            avg(&self.gaps),
            pct(&self.gaps, 0.99),
            pct(&self.gaps, 1.0),
            self.seek_latency.len(),
            avg(&self.seek_latency),
            pct(&self.seek_latency, 0.95),
            pct(&self.seek_latency, 1.0),
            avg(&self.queue_age),
            avg(&self.render_ms),
            pct(&self.render_ms, 0.99),
        )
    }
}

enum Mode {
    Playing,
    Paused,
    /// Showing single frames as the user drags.
    Scrubbing,
}

struct Player {
    args: Args,
    info: Info,
    out_w: u32,
    out_h: u32,
    focus: FocusHandle,

    video: Option<Receiver<Frame>>,
    video_done: bool,
    pending: VecDeque<Frame>,
    current: Option<Arc<RenderImage>>,
    current_pts: f64,
    last_shown: Option<Instant>,

    audio: Option<AudioOut>,
    /// Clock when there's no audio: position at `started`, and when it started.
    wall: (f64, Option<Instant>),

    mode: Mode,
    dragging: bool,
    /// A seek waiting for its first frame: when it was asked for.
    seek_started: Option<Instant>,
    /// Where the last scrub asked to go, and where the next should go once that one lands.
    scrub_target: Option<f64>,

    second: Stats,
    second_started: Instant,
    overlay: String,

    bench: Option<Bench>,
}

impl Player {
    fn new(args: Args, info: Info, focus: FocusHandle) -> Self {
        let (out_w, out_h) = match args.height {
            Some(h) if h < info.height => {
                let w = (info.width as f64 * h as f64 / info.height as f64).round() as u32 & !1;
                (w, h)
            }
            _ => (info.width, info.height),
        };
        let audio = if info.has_audio {
            match AudioOut::new(args.path.clone()) {
                Ok(a) => Some(a),
                Err(e) => {
                    eprintln!("no audio: {e:#}");
                    None
                }
            }
        } else {
            None
        };
        let bench = args.bench.then(Bench::new);
        let mut p = Self {
            args,
            info,
            out_w,
            out_h,
            focus,
            video: None,
            video_done: false,
            pending: VecDeque::new(),
            current: None,
            current_pts: 0.0,
            last_shown: None,
            audio,
            wall: (0.0, None),
            mode: Mode::Paused,
            dragging: false,
            seek_started: None,
            scrub_target: None,
            second: Stats::default(),
            second_started: Instant::now(),
            overlay: String::new(),
            bench,
        };
        p.seek(0.0, Mode::Playing);
        p
    }

    fn clock(&self) -> f64 {
        if let Some(a) = &self.audio {
            return a.clock();
        }
        match self.wall.1 {
            Some(started) => self.wall.0 + started.elapsed().as_secs_f64(),
            None => self.wall.0,
        }
    }

    fn seek(&mut self, t: f64, mode: Mode) {
        let t = t.clamp(0.0, (self.info.duration - 0.05).max(0.0));
        let single = matches!(mode, Mode::Scrubbing);
        self.video = Some(decode::start_video(VideoRequest {
            path: self.args.path.clone(),
            info: self.info.clone(),
            start: t,
            width: self.out_w,
            height: self.out_h,
            single,
            hwaccel: self.args.hwaccel,
        }));
        self.video_done = false;
        self.pending.clear();
        self.seek_started = Some(Instant::now());
        self.last_shown = None;
        let playing = matches!(mode, Mode::Playing);
        if let Some(a) = &mut self.audio {
            a.seek(t, playing);
            a.set_paused(!playing);
        }
        self.wall = (t, playing.then(Instant::now));
        self.current_pts = t;
        self.mode = mode;
    }

    fn toggle(&mut self) {
        match self.mode {
            Mode::Playing => {
                let t = self.clock();
                if let Some(a) = &self.audio {
                    a.set_paused(true);
                }
                self.wall = (t, None);
                self.mode = Mode::Paused;
            }
            _ => self.seek(self.current_pts, Mode::Playing),
        }
    }

    /// Scrub to `t`, at most one decode in flight: while one runs, remember the latest target.
    fn scrub(&mut self, t: f64) {
        if self.seek_started.is_some() && matches!(self.mode, Mode::Scrubbing) {
            self.scrub_target = Some(t);
            return;
        }
        self.scrub_target = None;
        self.seek(t, Mode::Scrubbing);
    }

    fn pull_frames(&mut self) {
        let Some(rx) = &self.video else { return };
        while self.pending.len() < 6 {
            match rx.try_recv() {
                Ok(f) => self.pending.push_back(f),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.video_done = true;
                    break;
                }
            }
        }
    }

    fn show(&mut self, frame: Frame, window: &mut Window, cx: &mut App) {
        let now = Instant::now();
        if let Some(started) = self.seek_started.take() {
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            self.second.seek_latency.push(ms);
            if let Some(b) = &mut self.bench {
                b.stats.seek_latency.push(ms);
            }
        } else if let (Mode::Playing, Some(last)) = (&self.mode, self.last_shown) {
            let gap = (now - last).as_secs_f64() * 1000.0;
            self.second.gaps.push(gap);
            if let Some(b) = &mut self.bench {
                b.stats.gaps.push(gap);
            }
        }
        let age = (now - frame.decoded_at).as_secs_f64() * 1000.0;
        self.second.queue_age.push(age);
        self.second.shown += 1;
        if let Some(b) = &mut self.bench {
            b.stats.queue_age.push(age);
            b.stats.shown += 1;
        }
        self.last_shown = Some(now);
        self.current_pts = frame.pts;

        let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
            .expect("frame size matches");
        let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
        if let Some(old) = self.current.replace(image) {
            cx.drop_image(old, Some(window));
        }
    }

    fn advance(&mut self, window: &mut Window, cx: &mut App) {
        self.pull_frames();
        match self.mode {
            Mode::Scrubbing | Mode::Paused => {
                // Show the newest frame we have; a scrub asks for one.
                if let Some(f) = self.pending.pop_back() {
                    self.pending.clear();
                    self.show(f, window, cx);
                    if let Some(t) = self.scrub_target.take() {
                        self.seek(t, Mode::Scrubbing);
                    }
                }
            }
            Mode::Playing => {
                let now = self.clock();
                let mut chosen = None;
                while self.pending.front().is_some_and(|f| f.pts <= now) {
                    if chosen.is_some() {
                        self.second.dropped += 1;
                        if let Some(b) = &mut self.bench {
                            b.stats.dropped += 1;
                        }
                    }
                    chosen = self.pending.pop_front();
                }
                // The first frame after a seek is shown as soon as it arrives.
                if chosen.is_none() && self.seek_started.is_some() {
                    chosen = self.pending.pop_front();
                }
                if let Some(f) = chosen {
                    self.show(f, window, cx);
                }
                if self.video_done && self.pending.is_empty() {
                    self.seek(0.0, Mode::Playing); // loop
                }
            }
        }
    }

    fn tick_stats(&mut self) {
        let secs = self.second_started.elapsed().as_secs_f64();
        if secs < 1.0 {
            return;
        }
        let underruns = self.audio.as_ref().map_or(0, |a| a.underruns());
        self.overlay = format!(
            "{}x{} {} {:.2} fps -> {}x{}{}\n{}\naudio underruns {}",
            self.info.width,
            self.info.height,
            self.info.codec,
            self.info.fps,
            self.out_w,
            self.out_h,
            if self.args.hwaccel { " hwaccel" } else { "" },
            self.second.summary("last 1s", secs),
            underruns,
        );
        self.second = Stats::default();
        self.second_started = Instant::now();
    }

    fn frac_at(&self, x: gpui::Pixels, window: &Window) -> f64 {
        (f32::from(x) / f32::from(window.viewport_size().width)).clamp(0.0, 1.0) as f64
    }
}

/// The scripted run behind `--bench`.
struct Bench {
    started: Instant,
    step: usize,
    step_started: Instant,
    stats: Stats,
    rng: u64,
    report: Vec<String>,
}

impl Bench {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            step: 0,
            step_started: Instant::now(),
            stats: Stats::default(),
            rng: 0x9e37_79b9_7f4a_7c15,
            report: Vec::new(),
        }
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }
}

const BENCH_STEPS: [(&str, f64); 4] = [
    ("play 12s from 0", 12.0),
    ("40 random seeks, one per 250ms", 10.0),
    ("drag across the whole file over 4s", 4.0),
    ("play 8s from the middle", 8.0),
];

impl Player {
    fn run_bench(&mut self, cx: &mut App) {
        let Some(b) = &mut self.bench else { return };
        let in_step = b.step_started.elapsed().as_secs_f64();
        let (label, length) = BENCH_STEPS[b.step];
        let duration = self.info.duration;

        if in_step >= length {
            let line = b.stats.summary(label, in_step);
            println!("{line}");
            b.report.push(line);
            b.stats = Stats::default();
            b.step += 1;
            b.step_started = Instant::now();
            let step = b.step;
            match step {
                1 => self.scrub(duration * 0.3),
                2 => self.scrub(0.0),
                3 => self.seek(duration * 0.5, Mode::Playing),
                _ => {
                    let total = b.started.elapsed().as_secs_f64();
                    println!("bench done in {total:.1}s");
                    if let Some(a) = &self.audio {
                        println!("audio underruns: {}", a.underruns());
                    }
                    cx.quit();
                }
            }
            return;
        }
        match b.step {
            1 => {
                let due = (in_step / 0.25) as usize;
                if due > b.stats.seek_latency.len() + usize::from(self.seek_started.is_some()) {
                    let t = b.random() * duration;
                    self.scrub(t);
                }
            }
            2 => self.scrub(duration * (in_step / length)),
            _ => {}
        }
    }
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = Instant::now();
        self.run_bench(cx);
        self.advance(window, cx);
        self.tick_stats();
        self.second.renders += 1;
        if let Some(b) = &mut self.bench {
            b.stats.renders += 1;
        }
        // Keep drawing every vsync; idle only when paused with nothing on the way.
        if !matches!(self.mode, Mode::Paused) || self.seek_started.is_some() || self.bench.is_some() {
            window.request_animation_frame();
        }

        let frac = if self.info.duration > 0.0 {
            (self.current_pts / self.info.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let time = format!(
            "{} / {}  {}",
            fmt_time(self.current_pts),
            fmt_time(self.info.duration),
            match self.mode {
                Mode::Playing => "playing",
                Mode::Paused => "paused",
                Mode::Scrubbing => "scrubbing",
            }
        );

        let render_ms = started.elapsed().as_secs_f64() * 1000.0;
        self.second.render_ms.push(render_ms);
        if let Some(b) = &mut self.bench {
            b.stats.render_ms.push(render_ms);
        }

        div()
            .id("root")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x000000))
            .text_color(rgb(0xf2f2f2))
            .font_family("Consolas")
            .text_size(px(12.))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                match ev.keystroke.key.as_str() {
                    "space" => this.toggle(),
                    "left" => this.seek(this.current_pts - 5.0, Mode::Playing),
                    "right" => this.seek(this.current_pts + 5.0, Mode::Playing),
                    _ => return,
                }
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                if this.dragging {
                    let t = this.frac_at(ev.position.x, window) * this.info.duration;
                    this.scrub(t);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseUpEvent, window, cx| {
                    if this.dragging {
                        this.dragging = false;
                        let t = this.frac_at(ev.position.x, window) * this.info.duration;
                        this.seek(t, Mode::Paused);
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .children(self.current.clone().map(|image| {
                        img(image).size_full().object_fit(ObjectFit::Contain)
                    }))
                    .child(
                        div()
                            .absolute()
                            .top_2()
                            .left_2()
                            .p_2()
                            .bg(gpui::black().opacity(0.7))
                            .whitespace_normal()
                            .child(self.overlay.clone()),
                    ),
            )
            .child(
                div()
                    .id("bar")
                    .h(px(28.))
                    .w_full()
                    .bg(rgb(0x1a1a1a))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                            this.dragging = true;
                            let t = this.frac_at(ev.position.x, window) * this.info.duration;
                            this.scrub(t);
                            cx.notify();
                        }),
                    )
                    .child(div().h_full().w(relative(frac)).bg(rgb(0xf2f2f2))),
            )
            .child(div().h(px(22.)).px_2().flex().items_center().child(time))
    }
}

fn fmt_time(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:05.2}", (t / 60.0) as u64, t % 60.0)
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let info = decode::probe(&args.path)?;
    println!(
        "{}: {}x{} {} {:.3} fps, {:.1}s, audio {}",
        args.path.display(),
        info.width,
        info.height,
        info.codec,
        info.fps,
        info.duration,
        info.has_audio
    );
    let mut args = Some(args);
    let mut info = Some(info);
    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let (args, info) = (args.take().unwrap(), info.take().unwrap());
        cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: true,
                    // GPUI drops unfocused windows to 30 fps; video has to keep its rate.
                    inactive_frame_interval: None,
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        Player::new(args, info, focus)
                    })
                },
            )
            .expect("open window");
        cx.activate(true);
    });
    Ok(())
}
