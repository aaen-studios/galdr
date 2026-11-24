//! GAL-1 / GAL-4: can GPUI show video smoothly on Windows, and scrub it fast?
//!
//! Plays a file and lets you scrub it. Frames are decoded in-process with libav (video.rs), turned
//! into BGRA and drawn as a new `RenderImage` each frame; the old one is dropped from the GPU atlas.
//!
//!   playback <file> [--height 720, 0 for source] [--hw none|d3d11|cuda] [--cache-mb 1024] [--mute] [--background] [--bench]
//!
//! Space plays and pauses, left and right jump 5 s, drag the bar to scrub. While dragging, galdr
//! shows keyframes and fills in the exact frame once the mouse rests. `--background` opens the
//! window off-screen without taking focus, and mutes. `--bench` runs a fixed script and prints
//! timings, then quits.

mod audio;
mod decode;
mod cache;


use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ObjectFit, RenderImage, Window, WindowBounds, WindowOptions, div, img, point,
    prelude::*, px, relative, rgb, size,
};
use gpui_platform::application;

use audio::AudioOut;
use cache::FrameCache;
use media::{Command, Frame, Hw, Info, VideoDecoder};

struct Args {
    path: PathBuf,
    height: Option<u32>,
    hw: Hw,
    mute: bool,
    background: bool,
    bench: bool,
    cache_mb: u64,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut out = Args {
        path: PathBuf::new(),
        // About what the preview area shows; the app will ask for its own size.
        height: Some(720),
        hw: Hw::None,
        mute: false,
        background: false,
        bench: false,
        cache_mb: 1024,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            // 0 is the source's size.
            "--height" => out.height = Some(args.next().unwrap_or_default().parse()?).filter(|h| *h > 0),
            "--hw" => out.hw = Hw::parse(&args.next().unwrap_or_default())?,
            "--mute" => out.mute = true,
            "--background" => {
                out.background = true;
                out.mute = true;
            }
            "--bench" => out.bench = true,
            "--cache-mb" => out.cache_mb = args.next().unwrap_or_default().parse()?,
            _ => out.path = PathBuf::from(a),
        }
    }
    anyhow::ensure!(
        !out.path.as_os_str().is_empty(),
        "usage: playback <file> [--height N] [--hw none|d3d11|cuda] [--mute] [--background] [--bench]"
    );
    Ok(out)
}

#[derive(Default)]
struct Stats {
    shown: u64,
    dropped: u64,
    renders: u64,
    /// Time between consecutive frames shown while playing, in ms.
    gaps: Vec<f64>,
    /// Time from asking for a position to drawing the first frame for it, in ms, and to drawing
    /// the exact frame.
    latency: Vec<f64>,
    exact_latency: Vec<f64>,
    cache_hits: u64,
    /// Time from a frame leaving the decoder to being drawn, in ms.
    queue_age: Vec<f64>,
    /// Time spent in `render`, in ms.
    render_ms: Vec<f64>,
    /// Per frame shown: frames the decoder decoded to reach it, and ms decoding and converting.
    decoded: Vec<f64>,
    decode_ms: Vec<f64>,
    convert_ms: Vec<f64>,
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

impl Stats {
    fn summary(&self, label: &str, secs: f64) -> String {
        format!(
            "{label}\n    {:.1} fps shown, {} dropped, ui {:.0} fps, gap max {:.1} ms, render p99 {:.2} ms\n    \
             first frame avg {:.1} p95 {:.1} ms | exact frame n={} avg {:.1} p95 {:.1} max {:.1} ms | cache hits {}\n    \
             per frame shown: {:.1} decoded, decode {:.1} ms, convert {:.1} ms",
            self.shown as f64 / secs,
            self.dropped,
            self.renders as f64 / secs,
            pct(&self.gaps, 1.0),
            pct(&self.render_ms, 0.99),
            avg(&self.latency),
            pct(&self.latency, 0.95),
            self.exact_latency.len(),
            avg(&self.exact_latency),
            pct(&self.exact_latency, 0.95),
            pct(&self.exact_latency, 1.0),
            self.cache_hits,
            avg(&self.decoded),
            avg(&self.decode_ms),
            avg(&self.convert_ms),
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Playing,
    Paused,
    Scrubbing,
}

/// How long the mouse rests during a drag before the exact frame replaces the keyframe.
const SETTLE: Duration = Duration::from_millis(120);

struct Player {
    info: Info,
    decoder: VideoDecoder,
    focus: FocusHandle,

    generation: u64,
    /// When each generation was asked for, to time how long it took to show, and whether its
    /// first frame has been shown.
    asked: HashMap<u64, (Instant, bool)>,
    shown_generation: u64,

    pending: VecDeque<Frame>,
    current: Option<Arc<RenderImage>>,
    cache: FrameCache,
    /// A scrub the cache answered: the image and its time, to show on the next draw.
    from_cache: Option<(Arc<RenderImage>, f64)>,
    current_pts: f64,
    last_shown: Option<Instant>,

    audio: Option<AudioOut>,
    /// Clock without audio: position at `started`, and when it started. Starts on the first frame.
    wall: (f64, Option<Instant>),

    mode: Mode,
    dragging: bool,
    /// The last scrub target and when it was asked for, and whether its exact frame is asked for.
    scrub: Option<(f64, Instant, bool)>,

    second: Stats,
    second_started: Instant,
    overlay: String,

    bench: Option<Bench>,
}

impl Player {
    fn new(args: Args, info: Info, decoder: VideoDecoder, focus: FocusHandle) -> Self {
        let audio = if info.has_audio && !args.mute {
            AudioOut::new(args.path.clone())
                .map_err(|e| eprintln!("no audio: {e:#}"))
                .ok()
        } else {
            None
        };
        let bench = args.bench.then(Bench::new);
        let mut p = Self {
            info,
            decoder,
            focus,
            generation: 0,
            asked: HashMap::new(),
            shown_generation: 0,
            pending: VecDeque::new(),
            current: None,
            cache: FrameCache::new(args.cache_mb * 1024 * 1024),
            from_cache: None,
            current_pts: 0.0,
            last_shown: None,
            audio,
            wall: (0.0, None),
            mode: Mode::Paused,
            dragging: false,
            scrub: None,
            second: Stats::default(),
            second_started: Instant::now(),
            overlay: String::new(),
            bench,
        };
        p.play(0.0);
        p
    }

    fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.asked.insert(self.generation, (Instant::now(), false));
        self.generation
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

    fn clamp(&self, t: f64) -> f64 {
        t.clamp(0.0, (self.info.duration - 0.05).max(0.0))
    }

    fn play(&mut self, t: f64) {
        let t = self.clamp(t);
        let generation = self.next_generation();
        self.decoder.send(Command::Play { generation, from: t });
        self.pending.clear();
        self.last_shown = None;
        if let Some(a) = &mut self.audio {
            a.seek(t, true);
            a.set_paused(false);
        }
        self.wall = (t, None); // the wall clock starts with the first frame
        self.mode = Mode::Playing;
        self.scrub = None;
    }

    fn pause(&mut self) {
        let t = self.current_pts;
        self.decoder.send(Command::Pause);
        if let Some(a) = &self.audio {
            a.set_paused(true);
        }
        self.wall = (t, None);
        self.mode = Mode::Paused;
    }

    fn toggle(&mut self) {
        if self.mode == Mode::Playing {
            self.pause();
        } else {
            self.play(self.current_pts);
        }
    }

    /// Scrub to `t`: a keyframe now, the exact frame once the target stops moving.
    fn scrub_to(&mut self, t: f64, exact: bool) {
        let t = self.clamp(t);
        if self.mode == Mode::Playing {
            self.pause();
        }
        let generation = self.next_generation();
        self.mode = Mode::Scrubbing;
        // Seen before: show it from the cache, and leave the decoder alone.
        if let Some(image) = self.cache.get(self.frame_index(t)) {
            self.asked.remove(&generation);
            self.shown_generation = generation;
            self.pending.clear();
            self.from_cache = Some((image, self.frame_index(t) as f64 / self.info.fps));
            self.scrub = Some((t, Instant::now(), true));
            self.record(|s| {
                s.cache_hits += 1;
                s.latency.push(0.0);
                s.exact_latency.push(0.0);
            });
            return;
        }
        self.decoder.send(Command::Scrub { generation, t, exact });
        self.scrub = Some((t, Instant::now(), exact));
    }

    fn frame_index(&self, t: f64) -> i64 {
        (t * self.info.fps).round() as i64
    }

    /// Puts `image` on screen, freeing the one it replaces unless the cache keeps it.
    fn set_image(&mut self, image: Arc<RenderImage>, window: &mut Window, cx: &mut App) {
        if let Some(old) = self.current.replace(image)
            && !self.cache.contains(&old)
            && !self.current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &old))
        {
            cx.drop_image(old, Some(window));
        }
    }

    fn pull_frames(&mut self) {
        // Take only what we can hold, so the decoder blocks instead of racing ahead.
        while self.pending.len() < 4 {
            match self.decoder.frames.try_recv() {
                Ok(f) => {
                    if f.generation >= self.shown_generation {
                        self.pending.push_back(f);
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn record(&mut self, f: impl Fn(&mut Stats)) {
        f(&mut self.second);
        if let Some(b) = &mut self.bench {
            f(&mut b.stats);
        }
    }

    fn show(&mut self, frame: Frame, window: &mut Window, cx: &mut App) {
        let now = Instant::now();
        let first_of_generation = frame.generation != self.shown_generation;
        let generation = frame.generation;
        if let Some((asked, first_done)) = self.asked.get_mut(&generation) {
            let ms = (now - *asked).as_secs_f64() * 1000.0;
            let first = !*first_done;
            *first_done = true;
            if first {
                self.record(|s| s.latency.push(ms));
            }
            if frame.exact {
                self.record(|s| s.exact_latency.push(ms));
                self.asked.remove(&generation);
            }
        }
        if first_of_generation {
            self.asked.retain(|g, _| *g >= generation);
        } else if let (Mode::Playing, Some(last)) = (self.mode, self.last_shown) {
            let gap = (now - last).as_secs_f64() * 1000.0;
            self.record(|s| s.gaps.push(gap));
        }
        let age = (now - frame.decoded_at).as_secs_f64() * 1000.0;
        let cost = frame.cost;
        self.record(|s| {
            s.queue_age.push(age);
            s.shown += 1;
            s.decoded.push(cost.decoded as f64);
            s.decode_ms.push(cost.decode_ms as f64);
            s.convert_ms.push(cost.convert_ms as f64);
        });
        self.shown_generation = frame.generation;
        self.last_shown = Some(now);
        self.current_pts = frame.pts;
        if self.mode == Mode::Playing && self.audio.is_none() && self.wall.1.is_none() {
            self.wall = (frame.pts, Some(now));
        }

        let bytes = frame.bgra.len() as u64;
        let exact = frame.exact;
        let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
            .expect("frame size matches");
        let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
        self.set_image(image.clone(), window, cx);
        if exact {
            for old in self.cache.insert(self.frame_index(frame.pts), image, bytes) {
                if !self.current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &old)) {
                    cx.drop_image(old, Some(window));
                }
            }
        }
    }

    fn advance(&mut self, window: &mut Window, cx: &mut App) {
        self.pull_frames();
        match self.mode {
            Mode::Scrubbing | Mode::Paused => {
                if let Some((image, pts)) = self.from_cache.take() {
                    self.set_image(image, window, cx);
                    self.current_pts = pts;
                    self.last_shown = Some(Instant::now());
                    self.record(|s| s.shown += 1);
                }
                // Show the newest frame that's arrived.
                if let Some(f) = self.pending.pop_back() {
                    self.pending.clear();
                    self.show(f, window, cx);
                }
                // The mouse has rested on a keyframe: ask for the exact frame.
                if let Some((t, at, false)) = self.scrub
                    && at.elapsed() >= SETTLE
                {
                    self.scrub_to(t, true);
                }
            }
            Mode::Playing => {
                self.pending.retain(|f| f.generation == self.generation);
                let waiting_for_first = self.shown_generation != self.generation;
                if waiting_for_first {
                    if let Some(f) = self.pending.pop_front() {
                        self.show(f, window, cx);
                    }
                    return;
                }
                let now = self.clock();
                let mut chosen = None;
                while self.pending.front().is_some_and(|f| f.pts <= now) {
                    if chosen.is_some() {
                        self.record(|s| s.dropped += 1);
                    }
                    chosen = self.pending.pop_front();
                }
                if let Some(f) = chosen {
                    self.show(f, window, cx);
                }
                if self.current_pts >= self.info.duration - 2.0 * (1.0 / self.info.fps)
                    && self.pending.is_empty()
                {
                    self.play(0.0); // loop
                }
            }
        }
    }

    fn tick_stats(&mut self) {
        let secs = self.second_started.elapsed().as_secs_f64();
        if secs < 1.0 {
            return;
        }
        self.overlay = format!(
            "{}x{} {} {:.2} fps, {}, cache {} frames {} MB\n{}",
            self.info.width,
            self.info.height,
            self.info.codec,
            self.info.fps,
            self.decoder.backend,
            self.cache.len(),
            self.cache.megabytes(),
            self.second.summary("last 1s", secs),
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
    seeks: usize,
}

impl Bench {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            step: 0,
            step_started: Instant::now(),
            stats: Stats::default(),
            rng: 0x9e37_79b9_7f4a_7c15,
            seeks: 0,
        }
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }
}

const BENCH_STEPS: [(&str, f64); 6] = [
    ("play 10s from 0", 10.0),
    ("40 random exact seeks, one per 250ms", 10.0),
    ("drag exact frames across the file in 4s", 4.0),
    ("drag keyframes across the file in 4s", 4.0),
    ("jog back and forth over 2s of video, once a second, for 4s", 4.0),
    ("play 6s from the middle", 6.0),
];

impl Player {
    fn run_bench(&mut self, cx: &mut App) {
        let Some(b) = &mut self.bench else { return };
        let in_step = b.step_started.elapsed().as_secs_f64();
        let (label, length) = BENCH_STEPS[b.step];
        let duration = self.info.duration;

        if in_step >= length {
            println!("{}", b.stats.summary(label, in_step));
            b.stats = Stats::default();
            b.step += 1;
            b.step_started = Instant::now();
            b.seeks = 0;
            if b.step == 5 {
                self.play(duration * 0.5);
            } else if b.step == BENCH_STEPS.len() {
                println!("bench done in {:.1}s", b.started.elapsed().as_secs_f64());
                cx.quit();
            }
            return;
        }
        match b.step {
            1 => {
                let due = (in_step / 0.25) as usize + 1;
                if due > b.seeks {
                    b.seeks += 1;
                    let t = b.random() * duration;
                    self.scrub_to(t, true);
                }
            }
            2 => self.scrub_to(duration * (in_step / length), true),
            3 => self.scrub_to(duration * (in_step / length), false),
            4 => {
                // A triangle wave: forward over 2 s of video in half a second, back in the next.
                let phase = in_step % 1.0;
                let along = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
                self.scrub_to(duration * 0.3 + 2.0 * along, false);
            }
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
        self.record(|s| s.renders += 1);
        let waiting = self.shown_generation != self.generation || matches!(self.scrub, Some((_, _, false)));
        if self.mode == Mode::Playing || waiting || self.bench.is_some() {
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
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        self.record(|s| s.render_ms.push(ms));

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
                    "left" => this.play(this.current_pts - 5.0),
                    "right" => this.play(this.current_pts + 5.0),
                    _ => return,
                }
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                if this.dragging {
                    let t = this.frac_at(ev.position.x, window) * this.info.duration;
                    this.scrub_to(t, false);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseUpEvent, window, cx| {
                    if this.dragging {
                        this.dragging = false;
                        let t = this.frac_at(ev.position.x, window) * this.info.duration;
                        this.scrub_to(t, true);
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .children(
                        self.current
                            .clone()
                            .map(|image| img(image).size_full().object_fit(ObjectFit::Contain)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_2()
                            .left_2()
                            .p_2()
                            .bg(gpui::black().opacity(0.7))
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
                            this.scrub_to(t, false);
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
    let (decoder, info) = VideoDecoder::open(&args.path, args.height, args.hw)?;
    println!(
        "{}: {}x{} {} {:.3} fps, {:.1}s, audio {}, decoder {}",
        args.path.display(),
        info.width,
        info.height,
        info.codec,
        info.fps,
        info.duration,
        info.has_audio,
        decoder.backend,
    );
    let mut start = Some((args, info, decoder));
    application().run(move |cx: &mut App| {
        let (args, info, decoder) = start.take().unwrap();
        let background = args.background;
        let window_size = size(px(1280.), px(800.));
        let bounds = if background {
            // Off every screen, so it never covers what the user is doing.
            Bounds { origin: point(px(-6000.), px(200.)), size: window_size }
        } else {
            Bounds::centered(None, window_size, cx)
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: !background,
                // GPUI drops unfocused windows to 30 fps; video has to keep its rate.
                inactive_frame_interval: None,
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    if !background {
                        window.focus(&focus, cx);
                    }
                    Player::new(args, info, decoder, focus)
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
