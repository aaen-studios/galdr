//! Plays and scrubs the timeline. Every frame shown is the source's exact frame, at the viewer's
//! size in device pixels: the frame engine decodes ahead into its cache, and this asks it for what
//! the playhead is about to need (ahead of it while playing, around it and in the direction it's
//! moving while scrubbing) and shows what's there.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{App, RenderImage, Window};
use media::{Engine, Settings, Want};

use crate::audio::{Audio, Segment};
use crate::project::Project;

/// How far ahead of the playhead to decode while playing.
const PLAY_AHEAD: f64 = 2.0;
/// How far around a resting or scrubbed playhead to decode, ahead in the direction it last moved
/// first.
const SCRUB_AHEAD: f64 = 1.0;
const AROUND: f64 = 2.0;
/// How long the viewer's size has to hold before frames are decoded at it.
const RESIZE_SETTLE: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Paused,
    Playing,
    Scrubbing,
}

#[derive(Default)]
pub struct Stats {
    /// Gaps between frames while playing, in ms, and the gaps at cuts between clips.
    pub gaps: Vec<f64>,
    pub cut_gaps: Vec<f64>,
    /// Time from a seek to the first frame shown for it, and to its exact frame, in ms.
    pub first: Vec<f64>,
    pub exact: Vec<f64>,
    /// Seeks whose exact frame was already decoded.
    pub cache_hits: u64,
    /// Ticks while playing whose frame wasn't ready.
    pub late: u64,
    /// Ticks while scrubbing, and those that showed the exact frame under the playhead.
    pub scrub_ticks: u64,
    pub scrub_exact: u64,
}

struct Seek {
    asked: Instant,
    shown: bool,
}

pub struct Preview {
    engine: Engine<Arc<RenderImage>>,
    pub mode: Mode,
    /// Where the playhead is on the timeline.
    pub playhead: f64,
    pub image: Option<Arc<RenderImage>>,
    /// Whether `image` is the exact frame at the playhead.
    exact: bool,
    /// The sound, whose samples played are the clock while playing.
    audio: Audio,
    /// While playing: the sound has started, once the first frame and enough sound were in.
    running: bool,
    /// +1 or -1: which way the playhead last moved, to decode that way first.
    direction: f64,
    seek: Option<Seek>,
    /// The clip on screen and when its last new frame went up, for the gap stats.
    last_frame: Option<(usize, Instant)>,
    viewer: (u32, u32),
    pending_viewer: Option<((u32, u32), Instant)>,
    pub stats: Stats,
}

fn make_image(width: u32, height: u32, bgra: Vec<u8>) -> Arc<RenderImage> {
    let buffer = image::RgbaImage::from_raw(width, height, bgra).unwrap_or_else(|| image::RgbaImage::new(1, 1));
    Arc::new(RenderImage::new([image::Frame::new(buffer)]))
}

/// Each clip's sound, in timeline order.
fn segments(project: &Project) -> Vec<Segment> {
    project
        .clips
        .iter()
        .map(|c| {
            let m = project.media(c.media);
            Segment {
                path: m.map(|m| m.path.clone()).unwrap_or_default(),
                has_audio: m.and_then(|m| m.info.as_ref()).is_some_and(|i| i.has_audio),
                source_in: c.source_in,
                source_out: c.source_out,
            }
        })
        .collect()
}

/// The parts of the sources under timeline times `t0` to `t1`, in timeline order.
fn span(project: &Project, t0: f64, t1: f64) -> Vec<Want> {
    let mut out = Vec::new();
    let mut start = 0.0;
    for c in &project.clips {
        let end = start + c.length();
        let (a, b) = (t0.max(start), t1.min(end));
        if a < b {
            out.push(Want {
                source: c.media,
                from: c.source_in + (a - start),
                to: (c.source_in + (b - start)).min(c.source_out - 0.001),
            });
        }
        start = end;
        if start >= t1 {
            break;
        }
    }
    out
}

impl Preview {
    /// `silent` plays nothing out loud, with the same timing.
    pub fn new(settings: Settings, silent: bool) -> Self {
        Self {
            engine: Engine::new(settings, make_image),
            mode: Mode::Paused,
            playhead: 0.0,
            image: None,
            exact: false,
            audio: Audio::new(silent),
            running: false,
            direction: 1.0,
            seek: None,
            last_frame: None,
            viewer: (0, 0),
            pending_viewer: None,
            stats: Stats::default(),
        }
    }

    pub fn audio(&self) -> &Audio {
        &self.audio
    }

    pub fn engine(&self) -> &Engine<Arc<RenderImage>> {
        &self.engine
    }

    /// The viewer's size in device pixels, every frame. A new size is used once it holds still.
    pub fn viewer_size(&mut self, w: u32, h: u32) {
        if (w, h) == self.viewer || w == 0 || h == 0 {
            self.pending_viewer = None;
            return;
        }
        match self.pending_viewer {
            Some((size, since)) if size == (w, h) => {
                if since.elapsed() >= RESIZE_SETTLE || self.viewer == (0, 0) {
                    self.viewer = (w, h);
                    self.pending_viewer = None;
                    self.engine.set_viewer(w, h);
                    self.exact = false;
                }
            }
            _ => self.pending_viewer = Some(((w, h), Instant::now())),
        }
    }

    pub fn resizing(&self) -> bool {
        self.pending_viewer.is_some()
    }

    pub fn play(&mut self, project: &Project, t: f64) {
        if project.clips.is_empty() {
            return;
        }
        let t = if t >= project.duration() - 0.05 { 0.0 } else { t };
        self.playhead = t;
        self.mode = Mode::Playing;
        self.direction = 1.0;
        self.last_frame = None;
        self.running = false;
        self.audio.start(segments(project), t, None);
    }

    pub fn pause(&mut self, _project: &Project) {
        self.mode = Mode::Paused;
        self.running = false;
        self.audio.stop();
    }

    pub fn toggle(&mut self, project: &Project) {
        if self.mode == Mode::Playing {
            self.pause(project);
        } else {
            self.play(project, self.playhead);
        }
    }

    /// Moves the playhead to `t`. `dragging` keeps it in scrubbing mode until the drag ends.
    pub fn seek(&mut self, project: &Project, t: f64, dragging: bool) {
        let t = t.clamp(0.0, project.duration());
        let moved = t != self.playhead;
        if moved {
            self.direction = if t > self.playhead { 1.0 } else { -1.0 };
        }
        self.playhead = t;
        if self.mode == Mode::Playing || !dragging {
            self.audio.stop();
        }
        self.running = false;
        if dragging && moved && t < project.duration() {
            // A short sound from under the playhead, as it moves.
            self.audio.snippet(segments(project), t);
        }
        self.mode = if dragging { Mode::Scrubbing } else { Mode::Paused };
        self.exact = false;
        self.seek = Some(Seek { asked: Instant::now(), shown: false });
    }

    /// The timeline changed: redraw where the playhead is.
    pub fn timeline_changed(&mut self, project: &Project) {
        if project.clips.is_empty() {
            self.mode = Mode::Paused;
            self.running = false;
            self.audio.stop();
            self.image = None;
            self.engine.want(Vec::new());
            return;
        }
        self.playhead = self.playhead.min(project.duration());
        self.exact = false;
        if self.mode == Mode::Playing {
            // Carry on from here on the new timeline.
            self.running = false;
            self.audio.start(segments(project), self.playhead, None);
        }
    }

    fn wants(&self, project: &Project, t: f64, place: &crate::project::Place, source_t: f64) -> Vec<Want> {
        let mut wants = vec![Want { source: place.media, from: source_t, to: source_t }];
        if self.mode == Mode::Playing {
            wants.extend(span(project, t, t + PLAY_AHEAD));
            return wants;
        }
        // Ahead in the direction of travel first, nearest first. Backward goes in short steps,
        // since each range is filled from its start.
        if self.direction > 0.0 {
            wants.extend(span(project, t, t + SCRUB_AHEAD));
        } else {
            let mut to = t;
            while to > t - SCRUB_AHEAD && to > 0.0 {
                let from = (to - 0.25).max(0.0);
                let mut s = span(project, from, to);
                s.reverse();
                wants.extend(s);
                to = from;
            }
        }
        wants.extend(span(project, (t - AROUND).max(0.0), t + AROUND));
        wants
    }

    fn set_image(&mut self, image: Arc<RenderImage>, window: &mut Window, cx: &mut App) {
        if self.image.as_ref().is_some_and(|i| Arc::ptr_eq(i, &image)) {
            return;
        }
        // Off the GPU; the engine's cache still has it if it's needed again.
        if let Some(old) = self.image.replace(image) {
            cx.drop_image(old, Some(window));
        }
    }

    /// Called every frame: shows the right frame and asks for the next ones. Gives whether it
    /// needs another frame drawn soon.
    pub fn tick(&mut self, project: &Project, window: &mut Window, cx: &mut App) -> bool {
        let now = Instant::now();
        let duration = project.duration();
        if self.mode == Mode::Playing && self.running {
            self.playhead = self.audio.clock().min(duration);
            if self.playhead >= duration || (self.audio.finished() && self.audio.flowing()) {
                self.mode = Mode::Paused;
                self.running = false;
                self.audio.stop();
            }
        }
        let t = self.playhead;
        let Some(place) = project.place(t.min(duration - 0.001)) else {
            self.image = None;
            return false;
        };
        let clip = &project.clips[place.index];
        let source_t = place.source_t.min(clip.source_out - 0.001);
        self.engine.want(self.wants(project, t, &place, source_t));

        match self.engine.get(place.media, source_t) {
            Some(frame) => {
                let new = !self.image.as_ref().is_some_and(|i| Arc::ptr_eq(i, &frame));
                if self.mode == Mode::Playing {
                    if !self.running && self.audio.ready() {
                        self.audio.resume();
                        self.running = true;
                    }
                    if new {
                        if let Some((index, last)) = self.last_frame {
                            let gap = (now - last).as_secs_f64() * 1000.0;
                            self.stats.gaps.push(gap);
                            if index != place.index {
                                self.stats.cut_gaps.push(gap);
                            }
                        }
                        self.last_frame = Some((place.index, now));
                    }
                }
                if let Some(seek) = self.seek.take() {
                    let ms = seek.asked.elapsed().as_secs_f64() * 1000.0;
                    if !seek.shown {
                        self.stats.first.push(ms);
                        if ms < 1.0 {
                            self.stats.cache_hits += 1;
                        }
                    }
                    self.stats.exact.push(ms);
                }
                self.set_image(frame, window, cx);
                self.exact = true;
            }
            None => {
                if self.mode == Mode::Playing {
                    if self.running {
                        self.stats.late += 1;
                    }
                } else if let Some((frame, _)) =
                    // While the exact frame decodes, the nearest one that's in, from this clip.
                    self.engine.nearest(
                        place.media,
                        source_t,
                        (source_t - 1.0).max(clip.source_in),
                        (source_t + 1.0).min(clip.source_out - 0.001),
                    )
                {
                    if let Some(seek) = self.seek.as_mut()
                        && !seek.shown
                    {
                        seek.shown = true;
                        self.stats.first.push(seek.asked.elapsed().as_secs_f64() * 1000.0);
                    }
                    self.set_image(frame, window, cx);
                }
                self.exact = false;
            }
        }
        if self.mode == Mode::Scrubbing {
            self.stats.scrub_ticks += 1;
            self.stats.scrub_exact += self.exact as u64;
        }
        self.mode == Mode::Playing || !self.exact || self.resizing()
    }

    /// Frames and bytes the engine holds.
    pub fn usage(&self) -> (usize, u64) {
        self.engine.usage()
    }
}
