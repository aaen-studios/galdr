//! galdr: a video editor with media tools built in.
//!
//!   galdr [files...] [--background] [--auto]
//!
//! Files given are imported. `--background` opens the window off-screen without taking focus.
//! `--auto` runs a scripted edit of the first two files (split, delete, trim, play across the cut,
//! scrub, export), prints what it measured, and quits.

mod auto;
mod cache;
mod preview;
mod project;

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use galdr_core::{Core, Message};
use gpui::{
    App, Bounds, Context, ExternalPaths, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Pixels, RenderImage, ScrollEvent,
    SharedString, Window, WindowBounds, WindowOptions, canvas, div, img, point, prelude::*, px,
    relative, rgb, size,
};
use gpui_platform::application;
use serde_json::json;

use crate::preview::{Mode, Preview};
use crate::project::{ClipId, MediaId, Project};

const BG: u32 = 0x000000;
const BG_DIM: u32 = 0x111111;
const FG: u32 = 0xf2f2f2;
const FG_DIM: u32 = 0x8a8a8a;
const FAINT: u32 = 0x333333;
const FONT: &str = "Consolas";

/// Clip edges within this many pixels of the mouse are grabbed to trim.
const EDGE: f32 = 6.0;
const RULER_H: f32 = 22.0;
const TRACK_H: f32 = 64.0;

#[derive(Clone, Copy, Debug)]
enum Drag {
    Playhead,
    TrimIn { clip: ClipId, from_x: f32, from_source: f64 },
    TrimOut { clip: ClipId, from_x: f32, from_source: f64 },
    Move { clip: ClipId, from_x: f32, moved: bool },
}

pub struct Export {
    pub id: u64,
    pub output: PathBuf,
    pub state: String,
    pub done: f64,
    pub duration: f64,
    pub error: Option<String>,
}

pub struct Galdr {
    pub project: Project,
    pub preview: Preview,
    core: Core,
    core_events: Receiver<Message>,
    pub export: Option<Export>,
    next_job: u64,
    thumbnails: HashMap<MediaId, Arc<RenderImage>>,
    pub selected: Option<ClipId>,
    focus: FocusHandle,
    /// Pixels per second on the timeline, and how far it's scrolled, in pixels.
    zoom: f32,
    fit: bool,
    scroll: f32,
    timeline_bounds: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<Drag>,
    pub auto: Option<auto::Script>,
}

fn fmt_time(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:05.2}", (t / 60.0) as u64, t % 60.0)
}

impl Galdr {
    fn new(files: Vec<PathBuf>, auto: bool, focus: FocusHandle, cx: &mut Context<Self>) -> Self {
        let (tx, core_events) = mpsc::channel();
        let mut app = Self {
            project: Project::default(),
            preview: Preview::new(),
            core: Core::start(tx),
            core_events,
            export: None,
            next_job: 1,
            thumbnails: HashMap::new(),
            selected: None,
            focus,
            zoom: 40.0,
            fit: true,
            scroll: 0.0,
            timeline_bounds: Rc::new(Cell::new(Bounds::default())),
            drag: None,
            auto: auto.then(auto::Script::new),
        };
        app.import(files, cx);
        app
    }

    pub fn import(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            let id = self.project.add_media(path.clone());
            cx.spawn(async move |this, cx| {
                let read = cx
                    .background_executor()
                    .spawn(async move { media::thumbnail(&path, 1.0, 96) })
                    .await;
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    match read {
                        Ok((info, frame)) => {
                            if let Some(m) = app.project.media_mut(id) {
                                m.info = Some(info);
                            }
                            if let Some(buffer) = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra) {
                                app.thumbnails.insert(id, Arc::new(RenderImage::new([image::Frame::new(buffer)])));
                            }
                        }
                        Err(e) => {
                            if let Some(m) = app.project.media_mut(id) {
                                m.error = Some(format!("{e:#}"));
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    fn prompt_import(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |app: &mut Galdr, cx| app.import(paths, cx));
            }
        })
        .detach();
    }

    pub fn append(&mut self, media: MediaId) {
        if let Some(id) = self.project.append(media) {
            self.selected = Some(id);
            self.edited();
        }
    }

    /// After any change to the timeline.
    pub fn edited(&mut self) {
        self.preview.timeline_changed(&self.project);
    }

    pub fn split(&mut self) {
        if let Some(right) = self.project.split(self.preview.playhead) {
            self.selected = Some(right);
            self.edited();
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take()
            && self.project.remove(id)
        {
            self.edited();
        }
    }

    pub fn start_export(&mut self, output: PathBuf) {
        let Some(first) = self.project.clips.first() else { return };
        let Some(info) = self.project.media(first.media).and_then(|m| m.info.clone()) else { return };
        let clips: Vec<_> = self
            .project
            .clips
            .iter()
            .filter_map(|c| {
                let path = &self.project.media(c.media)?.path;
                Some(json!({ "path": path.display().to_string(), "start": c.source_in, "end": c.source_out }))
            })
            .collect();
        let id = self.next_job;
        self.next_job += 1;
        // The first clip sets the size, no larger than 1080p for now.
        let (mut w, mut h) = (info.width, info.height);
        if h > 1080 {
            w = ((w as f64 * 1080.0 / h as f64).round() as u32) & !1;
            h = 1080;
        }
        let request = json!({
            "cmd": "export", "id": id, "output": output.display().to_string(),
            "width": w, "height": h, "fps": info.fps, "clips": clips,
        });
        let sent = self.core.send(&request);
        self.export = Some(Export {
            id,
            output,
            state: if sent { "queued".into() } else { "failed".into() },
            done: 0.0,
            duration: self.project.duration(),
            error: (!sent).then(|| "the core refused the export".into()),
        });
    }

    fn prompt_export(&mut self, cx: &mut Context<Self>) {
        let dir = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let path = cx.prompt_for_new_path(&dir, Some("galdr-export.mp4"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = path.await {
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    app.start_export(path);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn drain_core(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        while let Ok(message) = self.core_events.try_recv() {
            changed = true;
            match message {
                Message::Event(v) => {
                    let Some(export) = self.export.as_mut() else { continue };
                    if v["id"].as_u64() != Some(export.id) {
                        continue;
                    }
                    let kind = v["event"].as_str().unwrap_or("?");
                    if kind != "progress" {
                        export.state = kind.to_string();
                    }
                    if let Some(d) = v["done"].as_f64() {
                        export.done = d;
                    }
                    if let Some(e) = v["error"].as_str() {
                        export.error = Some(e.to_string());
                    }
                }
                Message::Panic { message, location, .. } => {
                    self.core.mark_dead();
                    if let Some(export) = self.export.as_mut() {
                        export.state = "failed".into();
                        export.error = Some(format!("the core crashed: {message} at {location}"));
                    }
                    self.core.restart();
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    // Timeline geometry: x is pixels from the track's left edge.

    fn fit_zoom(&mut self) {
        let width = f32::from(self.timeline_bounds.get().size.width);
        if self.fit && width > 0.0 {
            let d = self.project.duration().max(5.0) as f32;
            self.zoom = (width - 24.0).max(100.0) / d;
            self.scroll = 0.0;
        }
    }

    fn x_of(&self, t: f64) -> f32 {
        t as f32 * self.zoom - self.scroll
    }

    fn t_of(&self, x: f32) -> f64 {
        ((x + self.scroll) / self.zoom) as f64
    }

    fn local_x(&self, window_x: Pixels) -> f32 {
        f32::from(window_x) - f32::from(self.timeline_bounds.get().origin.x)
    }

    fn timeline_mouse_down(&mut self, e: &MouseDownEvent) {
        let x = self.local_x(e.position.x);
        let y = f32::from(e.position.y) - f32::from(self.timeline_bounds.get().origin.y);
        if y > RULER_H {
            for (i, c) in self.project.clips.iter().enumerate() {
                let x0 = self.x_of(self.project.clip_start(i));
                let x1 = x0 + c.length() as f32 * self.zoom;
                let drag = if (x - x0).abs() <= EDGE {
                    Some(Drag::TrimIn { clip: c.id, from_x: x, from_source: c.source_in })
                } else if (x - x1).abs() <= EDGE {
                    Some(Drag::TrimOut { clip: c.id, from_x: x, from_source: c.source_out })
                } else if x > x0 && x < x1 {
                    Some(Drag::Move { clip: c.id, from_x: x, moved: false })
                } else {
                    None
                };
                if let Some(d) = drag {
                    self.selected = Some(c.id);
                    self.drag = Some(d);
                    self.fit = false;
                    return;
                }
            }
        }
        self.drag = Some(Drag::Playhead);
        self.preview.scrub(&self.project, self.t_of(x), false);
    }

    fn timeline_mouse_move(&mut self, e: &MouseMoveEvent) {
        let Some(drag) = self.drag else { return };
        let x = self.local_x(e.position.x);
        match drag {
            Drag::Playhead => self.preview.scrub(&self.project, self.t_of(x), false),
            Drag::TrimIn { clip, from_x, from_source } => {
                self.project.trim_in(clip, from_source + ((x - from_x) / self.zoom) as f64);
                // Show the clip's new first frame.
                if let Some(i) = self.project.index_of(clip) {
                    self.preview.scrub(&self.project, self.project.clip_start(i), false);
                }
            }
            Drag::TrimOut { clip, from_x, from_source } => {
                self.project.trim_out(clip, from_source + ((x - from_x) / self.zoom) as f64);
                if let Some(i) = self.project.index_of(clip) {
                    let end = self.project.clip_start(i) + self.project.clips[i].length();
                    self.preview.scrub(&self.project, end - 0.034, false);
                }
            }
            Drag::Move { clip, from_x, .. } => {
                if (x - from_x).abs() > 4.0 {
                    self.drag = Some(Drag::Move { clip, from_x, moved: true });
                }
            }
        }
    }

    fn timeline_mouse_up(&mut self, e: &MouseUpEvent) {
        let Some(drag) = self.drag.take() else { return };
        let x = self.local_x(e.position.x);
        match drag {
            Drag::Playhead => self.preview.scrub(&self.project, self.t_of(x), true),
            Drag::TrimIn { .. } | Drag::TrimOut { .. } => self.edited(),
            Drag::Move { clip, moved: true, .. } => {
                // Drop before the first clip whose middle is right of the mouse.
                let t = self.t_of(x);
                let mut to = self.project.clips.len();
                for (i, c) in self.project.clips.iter().enumerate() {
                    if t < self.project.clip_start(i) + c.length() / 2.0 {
                        to = i;
                        break;
                    }
                }
                let from = self.project.index_of(clip).unwrap_or(0);
                self.project.move_clip(clip, if to > from { to - 1 } else { to });
                self.edited();
            }
            Drag::Move { .. } => {}
        }
    }

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let frame = 1.0 / 30.0;
        match e.keystroke.key.as_str() {
            "space" => self.preview.toggle(&self.project),
            "s" => self.split(),
            "delete" | "backspace" => self.delete_selected(),
            "left" => self.preview.scrub(&self.project, self.preview.playhead - frame, true),
            "right" => self.preview.scrub(&self.project, self.preview.playhead + frame, true),
            "home" => self.preview.scrub(&self.project, 0.0, true),
            "end" => self.preview.scrub(&self.project, self.project.duration(), true),
            "=" | "+" => {
                self.fit = false;
                self.zoom *= 1.25;
            }
            "-" => {
                self.fit = false;
                self.zoom /= 1.25;
            }
            "0" => self.fit = true,
            _ => return false,
        }
        true
    }
}

fn button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py(px(3.))
        .border_1()
        .border_color(rgb(FAINT))
        .cursor_pointer()
        .hover(|s| s.border_color(rgb(FG)))
        .child(label)
}

impl Galdr {
    fn render_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let status: SharedString = match &self.export {
            Some(e) if e.state == "progress" || e.state == "started" => {
                format!("exporting {:.0}%", 100.0 * e.done / e.duration.max(0.001)).into()
            }
            Some(e) if e.state == "finished" => format!("exported {}", e.output.display()).into(),
            Some(e) if e.state == "failed" => {
                format!("export failed: {}", e.error.as_deref().unwrap_or("?")).into()
            }
            Some(e) => e.state.clone().into(),
            None => "".into(),
        };
        let progress = self
            .export
            .as_ref()
            .filter(|e| e.state == "progress")
            .map(|e| (e.done / e.duration.max(0.001)).clamp(0.0, 1.0) as f32);
        div()
            .h(px(40.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(FAINT))
            .child(div().pr_3().child("galdr"))
            .child(button("import", "Import").on_click(cx.listener(|this, _, _, cx| this.prompt_import(cx))))
            .child(button("export", "Export").on_click(cx.listener(|this, _, _, cx| this.prompt_export(cx))))
            .child(div().flex_1())
            .children(progress.map(|f| {
                div().w(px(160.)).h(px(4.)).bg(rgb(FAINT)).child(div().h_full().w(relative(f)).bg(rgb(FG)))
            }))
            .child(div().text_color(rgb(FG_DIM)).child(status))
    }

    fn render_bin(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("bin")
            .w(px(260.))
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .border_r_1()
            .border_color(rgb(FAINT))
            .overflow_y_scroll()
            .children(self.project.media.iter().map(|m| {
                let id = m.id;
                let detail: SharedString = match (&m.info, &m.error) {
                    (Some(i), _) => format!("{}  {}x{}  {}", fmt_time(i.duration), i.width, i.height, i.codec).into(),
                    (None, Some(e)) => e.clone().into(),
                    (None, None) => "reading…".into(),
                };
                div()
                    .id(("media", id as usize))
                    .flex()
                    .gap_2()
                    .p_1()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(BG_DIM)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.append(id);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(96.))
                            .h(px(54.))
                            .flex_none()
                            .bg(rgb(BG_DIM))
                            .children(
                                self.thumbnails
                                    .get(&id)
                                    .cloned()
                                    .map(|t| img(t).size_full().object_fit(ObjectFit::Cover)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().truncate().child(SharedString::from(m.name.clone())))
                            .child(div().text_color(rgb(FG_DIM)).text_size(px(11.)).child(detail)),
                    )
            }))
    }

    fn render_preview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.project.duration();
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(rgb(BG))
                    .children(self.preview.image.clone().map(|i| img(i).size_full().object_fit(ObjectFit::Contain))),
            )
            .child(
                div()
                    .h(px(34.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_t_1()
                    .border_color(rgb(FAINT))
                    .child(
                        button("play", if self.preview.mode == Mode::Playing { "Pause" } else { "Play" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.preview.toggle(&this.project);
                                cx.notify();
                            })),
                    )
                    .child(button("split", "Split").on_click(cx.listener(|this, _, _, cx| {
                        this.split();
                        cx.notify();
                    })))
                    .child(SharedString::from(format!("{} / {}", fmt_time(self.preview.playhead), fmt_time(total)))),
            )
    }

    fn render_timeline(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        self.fit_zoom();
        let bounds = self.timeline_bounds.clone();
        let width = f32::from(bounds.get().size.width);
        let total = self.project.duration();

        // Ruler ticks: a step that leaves about 80 px between labels.
        let step = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0]
            .into_iter()
            .find(|s| *s as f32 * self.zoom >= 80.0)
            .unwrap_or(600.0);
        let first = (self.scroll / self.zoom / step as f32).floor() as i64;
        let last = ((self.scroll + width) / self.zoom / step as f32).ceil() as i64;
        let ticks = (first.max(0)..=last).map(|i| {
            let t = i as f64 * step;
            div()
                .absolute()
                .left(px(self.x_of(t)))
                .top_0()
                .h(px(RULER_H))
                .border_l_1()
                .border_color(rgb(FAINT))
                .pl_1()
                .text_size(px(10.))
                .text_color(rgb(FG_DIM))
                .child(SharedString::from(fmt_time(t)))
        });

        let clips = self.project.clips.iter().enumerate().map(|(i, c)| {
            let x0 = self.x_of(self.project.clip_start(i));
            let w = (c.length() as f32 * self.zoom).max(2.0);
            let name = self.project.media(c.media).map(|m| m.name.clone()).unwrap_or_default();
            let selected = self.selected == Some(c.id);
            div()
                .absolute()
                .left(px(x0))
                .top(px(RULER_H + 6.0))
                .w(px(w))
                .h(px(TRACK_H))
                .bg(rgb(0x1c1c1c))
                .border_1()
                .border_color(rgb(if selected { FG } else { 0x555555 }))
                .overflow_hidden()
                .px_1()
                .text_size(px(11.))
                .child(div().truncate().child(SharedString::from(name)))
                .child(
                    div()
                        .text_color(rgb(FG_DIM))
                        .truncate()
                        .child(SharedString::from(format!("{:.2}s from {:.2}s", c.length(), c.source_in))),
                )
        });
        let playhead_x = self.x_of(self.preview.playhead);

        div()
            .id("timeline")
            .relative()
            .h(px(RULER_H + TRACK_H + 20.0))
            .w_full()
            .border_t_1()
            .border_color(rgb(FAINT))
            .bg(rgb(BG_DIM))
            .overflow_hidden()
            .child(
                canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| {
                this.timeline_mouse_down(e);
                cx.notify();
            }))
            .on_scroll(cx.listener(|this, e: &ScrollEvent, window, cx| {
                let dy = f32::from(e.delta.pixel_delta(window.line_height()).y);
                this.fit = false;
                this.scroll = (this.scroll - dy).max(0.0);
                cx.notify();
            }))
            .children(ticks)
            .children(clips)
            .when(total > 0.0, |d| {
                d.child(div().absolute().left(px(playhead_x)).top_0().w(px(1.)).h_full().bg(rgb(FG)))
            })
    }
}

impl Render for Galdr {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        auto::step(self, cx);
        let busy = self.preview.tick(&self.project, window, cx);
        if busy || self.auto.is_some() || self.drag.is_some() {
            window.request_animation_frame();
        }
        div()
            .id("root")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(FG))
            .font_family(FONT)
            .text_size(px(12.))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if this.key(e) {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.timeline_mouse_move(e);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, e: &MouseUpEvent, _, cx| {
                this.timeline_mouse_up(e);
                cx.notify();
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.import(paths.paths().to_vec(), cx);
            }))
            .child(self.render_bar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.render_bin(cx))
                    .child(self.render_preview(cx)),
            )
            .child(self.render_timeline(cx))
    }
}

fn main() {
    let mut files = Vec::new();
    let (mut background, mut auto) = (false, false);
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--background" => background = true,
            "--auto" => auto = true,
            _ => files.push(PathBuf::from(a)),
        }
    }
    let mut start = Some(files);
    application().run(move |cx: &mut App| {
        let files = start.take().unwrap();
        let window_size = size(px(1400.), px(860.));
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
            |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    if !background {
                        window.focus(&focus, cx);
                    }
                    // The core's events arrive on its thread; pick them up every 16 ms.
                    cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor().timer(Duration::from_millis(16)).await;
                            if this.update(cx, |app: &mut Galdr, cx| app.drain_core(cx)).is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    Galdr::new(files, auto, focus, cx)
                })
            },
        )
        .expect("open window");
        if !background {
            cx.activate(true);
        }
    });
}
