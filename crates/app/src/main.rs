//! galdr: a video editor with media tools built in.
//!
//!   galdr [files...] [--background] [--auto] [--dark] [--snapshot FILE.png [--view VIEW]]
//!
//! Files given are imported. `--background` opens the window off-screen without taking focus.
//! `--auto` runs a scripted edit of the first two files (split, delete, trim, play across the cut,
//! scrub, export), prints what it measured, and quits. `--snapshot` saves a picture of the window
//! with the files on the timeline, and quits.

mod assets;
mod audio;
mod auto;
mod file;
mod jobs;
mod layout;
mod preview;
mod project;
mod snapshot;
mod theme;
mod tools;
mod view;

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use galdr_core::{Core, Message};
use gpui::{
    App, Bounds, Context, FocusHandle, KeyDownEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathPromptOptions,
    Pixels, RenderImage, TitlebarOptions, Window, WindowBounds, WindowOptions, point, prelude::*, px, size,
};
use gpui_platform::application;
use media::Settings;
use serde_json::json;

use crate::jobs::{Jobs, Kind};
use crate::layout::{INSPECTOR_W, Layout, MEDIA_W, TIMELINE_H};
use crate::preview::Preview;
use crate::project::{ClipId, MediaId, Project};
use crate::theme::{Mode, Theme};

/// Clip edges within this many pixels of the mouse are grabbed to trim.
pub const EDGE: f32 = 7.0;
/// The timeline's tool row, its ruler, and the gap above the track.
pub const TOOLS_H: f32 = 42.0;
pub const RULER_H: f32 = 28.0;
pub const TRACK_TOP: f32 = RULER_H + 8.0;
/// Room left of time zero, so the first clip's edge can be grabbed.
pub const LANE_PAD: f32 = 12.0;
/// The filmstrips' height in pixels.
const STRIP_HEIGHT: u32 = 160;
/// The waveforms' detail.
pub const PEAKS_PER_SECOND: u32 = 100;
/// The gap between the video and audio lanes.
pub const LANE_GAP: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Divider {
    Media,
    Inspector,
    Timeline,
}

#[derive(Clone, Copy, Debug)]
pub enum Drag {
    Playhead,
    TrimIn { clip: ClipId, from_x: f32, from_source: f64 },
    TrimOut { clip: ClipId, from_x: f32, from_source: f64 },
    Move { clip: ClipId, from_x: f32, x: f32, moved: bool },
    /// A panel's edge: where the mouse started, and the panel's size then.
    Resize { divider: Divider, from: f32, size: f32 },
}

/// What fills the window under the title bar.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Edit,
    /// One of the tools in the rail, by index: transcribe, convert, compress, download.
    Tool(usize),
    Settings,
}

pub struct Galdr {
    pub project: Project,
    pub preview: Preview,
    core: Core,
    core_events: Receiver<Message>,
    pub jobs: Jobs,
    /// The tool pages' files and choices.
    pub tools: tools::Tools,
    /// The project file, once saved or opened, and what it held then.
    pub project_path: Option<PathBuf>,
    saved: file::Saved,
    /// Saves and opens the core is doing.
    pending: HashMap<u64, file::Pending>,
    /// A line shown in the title bar for a few seconds.
    pub notice: Option<(String, std::time::Instant)>,
    /// The project menu under the name in the title bar.
    pub menu: bool,
    /// Where a file dragged from the media would go in the track: its x in the lane.
    pub media_drop: Option<f32>,
    /// Quitting: once the user said not to save, or once the save they asked for is done.
    closing: bool,
    after_save: Option<file::Then>,
    pub thumbnails: HashMap<MediaId, Arc<RenderImage>>,
    /// Frames along each file, for the clips' filmstrips: source time and image.
    pub strips: HashMap<MediaId, Vec<(f64, Arc<RenderImage>)>>,
    /// How loud each file is along its length, `PEAKS_PER_SECOND` values a second.
    pub waveforms: HashMap<MediaId, Arc<Vec<f32>>>,
    pub selected: Option<ClipId>,
    pub selected_media: Option<MediaId>,
    focus: FocusHandle,
    pub theme: Theme,
    pub layout: Layout,
    pub page: Page,
    /// Only the viewer and its transport.
    pub focus_view: bool,
    /// Pixels per second on the timeline, and how far it's scrolled, in pixels.
    pub zoom: f32,
    pub fit: bool,
    pub scroll: f32,
    pub timeline_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub viewer_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub drag: Option<Drag>,
    pub auto: Option<auto::Script>,
    pub snapshot: Option<snapshot::Snapshot>,
}

/// Hours, minutes, seconds and frames.
pub fn timecode(t: f64, fps: f64) -> String {
    let fps = fps.round().max(1.0);
    let frames = (t.max(0.0) * fps + 1e-6).floor() as u64;
    let per_second = fps as u64;
    let s = frames / per_second;
    format!("{:02}:{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60, frames % per_second)
}

impl Galdr {
    fn new(
        files: Vec<PathBuf>,
        auto: bool,
        snapshot: Option<snapshot::Snapshot>,
        dark: bool,
        silent: bool,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, core_events) = mpsc::channel();
        let mut layout = if snapshot.is_some() || auto { Layout::default() } else { Layout::load() };
        if dark {
            layout.theme = Mode::Dark;
        }
        let mut app = Self {
            project: Project::default(),
            preview: Preview::new(Settings::default(), silent),
            core: Core::start(tx),
            core_events,
            jobs: Jobs::default(),
            tools: tools::Tools::default(),
            project_path: None,
            saved: file::Saved::default(),
            pending: HashMap::new(),
            notice: None,
            menu: false,
            media_drop: None,
            closing: false,
            after_save: None,
            thumbnails: HashMap::new(),
            strips: HashMap::new(),
            waveforms: HashMap::new(),
            selected: None,
            selected_media: None,
            focus,
            theme: Theme::of(layout.theme),
            layout,
            page: Page::Edit,
            focus_view: false,
            zoom: 40.0,
            fit: true,
            scroll: 0.0,
            timeline_bounds: Rc::new(Cell::new(Bounds::default())),
            viewer_bounds: Rc::new(Cell::new(Bounds::default())),
            drag: None,
            auto: auto.then(auto::Script::new),
            snapshot,
        };
        app.import(files, cx);
        app
    }

    /// The frame rate the timeline counts in: the first clip's, or the first file's.
    pub fn fps(&self) -> f64 {
        let media = self.project.clips.first().map(|c| c.media).or(self.project.media.first().map(|m| m.id));
        media.and_then(|m| self.project.media(m)).and_then(|m| m.info.as_ref()).map_or(30.0, |i| i.fps)
    }

    /// The tracks' height: whatever the timeline has room for, video and audio together.
    fn tracks_h(&self) -> f32 {
        (self.layout.timeline_h - TOOLS_H - TRACK_TOP - 20.0).clamp(80.0, 320.0)
    }

    /// The video lane's height.
    pub fn track_h(&self) -> f32 {
        self.tracks_h() - LANE_GAP - self.audio_h()
    }

    /// The audio lane's height, under the video.
    pub fn audio_h(&self) -> f32 {
        (self.tracks_h() * 0.32).clamp(30.0, 72.0)
    }

    /// Where the audio lane starts, down from the top of the lane area.
    pub fn audio_top(&self) -> f32 {
        TRACK_TOP + self.track_h() + LANE_GAP
    }

    pub fn set_layout(&mut self, f: impl FnOnce(&mut Layout)) {
        f(&mut self.layout);
        self.theme = Theme::of(self.layout.theme);
        if self.snapshot.is_none() && self.auto.is_none() {
            self.layout.save();
        }
    }

    pub fn import(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("galdr")) {
                self.open_project(path);
            } else {
                self.import_one(path, cx);
            }
        }
    }

    /// Adds a file to the media and reads it. Its info shows as soon as it's read; the thumbnail,
    /// sound and filmstrip then load side by side.
    pub fn import_one(&mut self, path: PathBuf, cx: &mut Context<Self>) -> MediaId {
        // Saved in projects, which open from anywhere.
        let path = std::path::absolute(&path).unwrap_or(path);
        let id = self.project.add_media(path.clone());
        let engine = self.preview.engine().clone();
        cx.spawn(async move |this, cx| {
            let p = path.clone();
            let read = cx.background_executor().spawn(async move { engine.add_source(id, &p) }).await;
            let _ = this.update(cx, |app: &mut Galdr, cx| {
                match read {
                    Ok(info) => {
                        let duration = info.duration;
                        if let Some(m) = app.project.media_mut(id) {
                            m.info = Some(info);
                        }
                        if duration > 0.0 {
                            app.load_pictures(id, path, duration, cx);
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
        id
    }

    /// A file's thumbnail, waveform and filmstrip, each read on its own and shown when it's in.
    fn load_pictures(&mut self, id: MediaId, path: PathBuf, duration: f64, cx: &mut Context<Self>) {
        let p = path.clone();
        cx.spawn(async move |this, cx| {
            let thumb = cx
                .background_executor()
                .spawn(async move { media::thumbnail(&p, (duration * 0.1).min(1.0), 180) })
                .await;
            if let Ok((_, frame)) = thumb
                && let Some(buffer) = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
            {
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    app.thumbnails.insert(id, Arc::new(RenderImage::new([image::Frame::new(buffer)])));
                    cx.notify();
                });
            }
        })
        .detach();

        let p = path.clone();
        cx.spawn(async move |this, cx| {
            let peaks = cx.background_executor().spawn(async move { media::AudioReader::peaks(&p, PEAKS_PER_SECOND) }).await;
            if let Ok(Some(peaks)) = peaks {
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    app.waveforms.insert(id, Arc::new(peaks));
                    cx.notify();
                });
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            // One frame a second along the file, up to 240.
            let count = (duration.ceil() as usize).clamp(2, 240);
            let times: Vec<f64> = (0..count).map(|i| duration * (i as f64 + 0.5) / count as f64).collect();
            let strip = cx
                .background_executor()
                .spawn(async move {
                    let frames = media::filmstrip(&path, &times, STRIP_HEIGHT)?;
                    let images: Vec<_> = times
                        .into_iter()
                        .zip(frames)
                        .filter_map(|(t, f)| {
                            let buffer = image::RgbaImage::from_raw(f.width, f.height, f.bgra)?;
                            Some((t, Arc::new(RenderImage::new([image::Frame::new(buffer)]))))
                        })
                        .collect();
                    anyhow::Ok(images)
                })
                .await;
            if let Ok(images) = strip {
                let _ = this.update(cx, |app: &mut Galdr, cx| {
                    app.strips.insert(id, images);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn prompt_import(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |app: &mut Galdr, cx| app.import(paths, cx));
            }
        })
        .detach();
    }

    /// Makes an edit that can be undone.
    fn edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.project.checkpoint();
        let r = f(&mut self.project);
        self.project.settle();
        self.edited();
        r
    }

    pub fn append(&mut self, media: MediaId) {
        if let Some(id) = self.edit(|p| p.append(media)) {
            self.selected = Some(id);
        }
    }

    /// After any change to the timeline.
    pub fn edited(&mut self) {
        self.preview.timeline_changed(&self.project);
    }

    pub fn split(&mut self) {
        let t = self.preview.playhead;
        if let Some(right) = self.edit(|p| p.split(t)) {
            self.selected = Some(right);
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.edit(|p| p.remove(id));
        }
    }

    pub fn undo(&mut self) {
        if self.project.undo() {
            self.edited();
        }
    }

    pub fn redo(&mut self) {
        if self.project.redo() {
            self.edited();
        }
    }

    pub fn start_export(&mut self, output: PathBuf) {
        let Some((w, h, fps)) = self.export_size() else { return };
        let clips: Vec<_> = self
            .project
            .clips
            .iter()
            .filter_map(|c| {
                let path = &self.project.media(c.media)?.path;
                Some(json!({ "path": path.display().to_string(), "start": c.source_in, "end": c.source_out }))
            })
            .collect();
        let id = self.jobs.add(Kind::Export, "Timeline".into(), output.clone(), self.project.duration());
        self.send_job(
            id,
            json!({
                "cmd": "export", "id": id, "output": output.display().to_string(),
                "width": w, "height": h, "fps": fps, "clips": clips,
            }),
        );
    }

    /// Sends a job's request to the core, which tells how it goes from then on.
    pub fn send_job(&mut self, id: u64, request: serde_json::Value) {
        if !self.core.send(&request) {
            self.jobs.failed(id, "the core refused the job");
        }
    }

    pub fn cancel_job(&mut self, id: u64) {
        self.core.send(&json!({ "cmd": "cancel", "id": id }));
    }

    /// What an export comes out as: the first clip's size, no larger than 1080p for now.
    pub fn export_size(&self) -> Option<(u32, u32, f64)> {
        let first = self.project.clips.first()?;
        let info = self.project.media(first.media)?.info.as_ref()?;
        let (mut w, mut h) = (info.width, info.height);
        if h > 1080 {
            w = ((w as f64 * 1080.0 / h as f64).round() as u32) & !1;
            h = 1080;
        }
        Some((w, h, info.fps))
    }

    pub fn exporting(&self) -> bool {
        self.jobs.any_active(Kind::Export)
    }

    pub fn prompt_export(&mut self, cx: &mut Context<Self>) {
        if self.project.clips.is_empty() || self.exporting() {
            return;
        }
        let dir = std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Videos")).unwrap_or_else(std::env::temp_dir);
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

    /// Shows a file in Explorer.
    pub fn reveal(path: &std::path::Path) {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }

    fn drain_core(&mut self, cx: &mut Context<Self>) {
        // A notice goes after a few seconds.
        let mut changed = self.notice.take_if(|(_, at)| at.elapsed().as_secs_f64() > file::NOTICE_SECS).is_some();
        while let Ok(message) = self.core_events.try_recv() {
            changed = true;
            match message {
                Message::Event(v) => {
                    if !self.core_reply(&v, cx) {
                        self.jobs.apply(&v);
                    }
                }
                Message::Panic { message, location, .. } => {
                    self.core.mark_dead();
                    self.jobs.crashed(&format!("the core crashed: {message} at {location}"));
                    self.core.restart();
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    // Timeline geometry: x is pixels from the lane's left edge.

    pub fn fit_zoom(&mut self) {
        let width = f32::from(self.timeline_bounds.get().size.width);
        if self.fit && width > 0.0 {
            let d = self.project.duration().max(10.0) as f32;
            self.zoom = (width - LANE_PAD * 2.0).max(100.0) / d;
            self.scroll = 0.0;
        }
    }

    pub fn x_of(&self, t: f64) -> f32 {
        LANE_PAD + t as f32 * self.zoom - self.scroll
    }

    pub fn t_of(&self, x: f32) -> f64 {
        ((x - LANE_PAD + self.scroll) / self.zoom).max(0.0) as f64
    }

    fn local_x(&self, window_x: Pixels) -> f32 {
        f32::from(window_x) - f32::from(self.timeline_bounds.get().origin.x)
    }

    /// The gap between clips nearest to `x`: its index in the track, and its x.
    pub fn gap_at(&self, x: f32) -> (usize, f32) {
        let t = self.t_of(x);
        let mut to = self.project.clips.len();
        for (i, c) in self.project.clips.iter().enumerate() {
            if t < self.project.clip_start(i) + c.length() / 2.0 {
                to = i;
                break;
            }
        }
        let gap_x = if to < self.project.clips.len() {
            self.x_of(self.project.clip_start(to))
        } else {
            self.x_of(self.project.duration())
        };
        (to, gap_x)
    }

    /// Where a clip being moved would land: its index once dropped, and the x of that gap.
    pub fn drop_slot(&self, clip: ClipId, x: f32) -> (usize, f32) {
        let (to, gap_x) = self.gap_at(x);
        let from = self.project.index_of(clip).unwrap_or(0);
        (if to > from { to - 1 } else { to }, gap_x)
    }

    /// A file dragged from the media and dropped on the track at `x`.
    pub fn drop_media(&mut self, media: MediaId, x: f32) {
        let (at, _) = self.gap_at(x);
        if let Some(id) = self.edit(|p| p.insert(media, at)) {
            self.selected = Some(id);
        }
    }

    pub fn timeline_mouse_down(&mut self, e: &MouseDownEvent) {
        let x = self.local_x(e.position.x);
        let y = f32::from(e.position.y) - f32::from(self.timeline_bounds.get().origin.y);
        if (TRACK_TOP..self.audio_top() + self.audio_h()).contains(&y) {
            for (i, c) in self.project.clips.iter().enumerate() {
                let x0 = self.x_of(self.project.clip_start(i));
                let x1 = x0 + c.length() as f32 * self.zoom;
                let drag = if (x - x0).abs() <= EDGE && x < x0 + (x1 - x0) / 2.0 {
                    Some(Drag::TrimIn { clip: c.id, from_x: x, from_source: c.source_in })
                } else if (x - x1).abs() <= EDGE {
                    Some(Drag::TrimOut { clip: c.id, from_x: x, from_source: c.source_out })
                } else if x > x0 && x < x1 {
                    Some(Drag::Move { clip: c.id, from_x: x, x, moved: false })
                } else {
                    None
                };
                if let Some(d) = drag {
                    let id = c.id;
                    // Undone as one edit, from here to where the drag ends.
                    self.project.checkpoint();
                    self.selected = Some(id);
                    self.drag = Some(d);
                    self.fit = false;
                    return;
                }
            }
            self.selected = None;
        }
        self.drag = Some(Drag::Playhead);
        self.preview.seek(&self.project, self.t_of(x), true);
    }

    pub fn start_resize(&mut self, divider: Divider, e: &MouseDownEvent) {
        let (from, size) = match divider {
            Divider::Media => (f32::from(e.position.x), self.layout.media_w),
            Divider::Inspector => (f32::from(e.position.x), self.layout.inspector_w),
            Divider::Timeline => (f32::from(e.position.y), self.layout.timeline_h),
        };
        self.drag = Some(Drag::Resize { divider, from, size });
    }

    pub fn mouse_move(&mut self, e: &MouseMoveEvent) {
        let Some(drag) = self.drag else { return };
        let x = self.local_x(e.position.x);
        match drag {
            Drag::Playhead => self.preview.seek(&self.project, self.t_of(x), true),
            Drag::TrimIn { clip, from_x, from_source } => {
                self.project.trim_in(clip, from_source + ((x - from_x) / self.zoom) as f64);
                // Show the clip's new first frame.
                if let Some(i) = self.project.index_of(clip) {
                    self.preview.seek(&self.project, self.project.clip_start(i), true);
                }
            }
            Drag::TrimOut { clip, from_x, from_source } => {
                self.project.trim_out(clip, from_source + ((x - from_x) / self.zoom) as f64);
                if let Some(i) = self.project.index_of(clip) {
                    let end = self.project.clip_start(i) + self.project.clips[i].length();
                    self.preview.seek(&self.project, end - 0.001, true);
                }
            }
            Drag::Move { clip, from_x, moved, .. } => {
                self.drag = Some(Drag::Move { clip, from_x, x, moved: moved || (x - from_x).abs() > 4.0 });
            }
            Drag::Resize { divider, from, size } => {
                // Sizes move live; they're saved when the drag ends.
                match divider {
                    Divider::Media => {
                        self.layout.media_w = (size + f32::from(e.position.x) - from).clamp(MEDIA_W.0, MEDIA_W.1)
                    }
                    Divider::Inspector => {
                        self.layout.inspector_w =
                            (size - (f32::from(e.position.x) - from)).clamp(INSPECTOR_W.0, INSPECTOR_W.1)
                    }
                    Divider::Timeline => {
                        self.layout.timeline_h =
                            (size - (f32::from(e.position.y) - from)).clamp(TIMELINE_H.0, TIMELINE_H.1)
                    }
                }
            }
        }
    }

    pub fn mouse_up(&mut self, e: &MouseUpEvent) {
        let Some(drag) = self.drag.take() else { return };
        let x = self.local_x(e.position.x);
        match drag {
            Drag::Playhead => self.preview.seek(&self.project, self.t_of(x), false),
            Drag::TrimIn { .. } | Drag::TrimOut { .. } => {
                self.project.settle();
                let t = self.preview.playhead;
                self.edited();
                self.preview.seek(&self.project, t, false);
            }
            Drag::Move { clip, moved: true, .. } => {
                let (to, _) = self.drop_slot(clip, x);
                self.project.move_clip(clip, to);
                self.project.settle();
                self.edited();
            }
            Drag::Move { .. } => self.project.settle(),
            Drag::Resize { .. } => self.set_layout(|_| {}),
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        // Keep the playhead where it is on screen.
        let anchor = self.x_of(self.preview.playhead);
        self.fit = false;
        self.zoom = (self.zoom * factor).clamp(2.0, 2000.0);
        self.scroll = (LANE_PAD + self.preview.playhead as f32 * self.zoom - anchor).max(0.0);
    }

    pub fn step_frames(&mut self, n: f64) {
        let t = self.preview.playhead + n / self.fps();
        self.preview.seek(&self.project, t, false);
    }

    pub fn toggle_focus_view(&mut self) {
        self.focus_view = !self.focus_view;
        self.page = Page::Edit;
    }

    pub fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = e.keystroke.modifiers;
        if m.control {
            match e.keystroke.key.as_str() {
                "[" => self.set_layout(|l| l.media = !l.media),
                "]" => self.set_layout(|l| l.inspector = !l.inspector),
                "z" if m.shift => self.redo(),
                "z" => self.undo(),
                "y" => self.redo(),
                "s" if m.shift => self.save_as(cx),
                "s" => self.save(cx),
                "o" => self.guard(file::Then::Open, window, cx),
                "n" => self.guard(file::Then::New, window, cx),
                _ => return false,
            }
            return true;
        }
        match e.keystroke.key.as_str() {
            "space" => self.preview.toggle(&self.project),
            "s" => self.split(),
            "delete" | "backspace" => self.delete_selected(),
            "left" => self.step_frames(-1.0),
            "right" => self.step_frames(1.0),
            "home" => self.preview.seek(&self.project, 0.0, false),
            "end" => self.preview.seek(&self.project, self.project.duration(), false),
            "=" | "+" => self.zoom_by(1.25),
            "-" => self.zoom_by(0.8),
            "0" => self.fit = true,
            "f" => self.toggle_focus_view(),
            "f11" => window.toggle_fullscreen(),
            "escape" if self.menu => self.menu = false,
            "escape" if self.focus_view => self.focus_view = false,
            "escape" if window.is_fullscreen() => window.toggle_fullscreen(),
            _ => return false,
        }
        true
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }
}

fn main() {
    let mut files = Vec::new();
    let (mut background, mut auto, mut dark) = (false, false, false);
    let mut snapshot = None;
    let mut view = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--background" => background = true,
            "--auto" => auto = true,
            "--dark" => dark = true,
            "--snapshot" => {
                snapshot = args.next().map(PathBuf::from);
                background = true;
            }
            "--view" => view = args.next(),
            _ => files.push(PathBuf::from(a)),
        }
    }
    let mut start = Some(files);
    application().with_assets(assets::Assets).run(move |cx: &mut App| {
        if let Err(e) = cx.text_system().add_fonts(assets::fonts()) {
            eprintln!("loading fonts: {e:#}");
        }
        let files = start.take().unwrap();
        let window_size = size(px(1600.), px(960.));
        let bounds = if background {
            Bounds { origin: point(px(-6000.), px(200.)), size: window_size }
        } else {
            Bounds::centered(None, window_size, cx)
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // galdr draws its own title bar (view.rs), as loom does.
                titlebar: Some(TitlebarOptions {
                    title: Some("galdr".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.), px(16.))),
                }),
                app_owns_titlebar_drag: true,
                window_min_size: Some(size(px(960.), px(620.))),
                focus: !background,
                inactive_frame_interval: None,
                ..Default::default()
            },
            |window, cx| {
                let app = cx.new(|cx| {
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
                    Galdr::new(files, auto, snapshot.clone().map(|p| snapshot::Snapshot::new(p, view.clone())), dark, background, focus, cx)
                });
                // Unsaved changes: ask first.
                let handle = app.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    handle.upgrade().is_none_or(|app| app.update(cx, |g, cx| g.should_close(window, cx)))
                });
                app
            },
        )
        .expect("open window");
        if !background {
            cx.activate(true);
        }
    });
}
