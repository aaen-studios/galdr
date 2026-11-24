//! Plays and scrubs the timeline. Each clip gets its own decoder, so two clips cut from one file
//! don't fight over a position, and the next clip's decoder starts half a second before the cut so
//! its first frame is waiting when the playhead gets there.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use gpui::{App, RenderImage, Window};
use media::{Command, Frame, Hw, VideoDecoder};

use crate::cache::FrameCache;
use crate::project::{ClipId, MediaId, Project};

/// How tall the frames the preview decodes are.
pub const PREVIEW_HEIGHT: u32 = 720;
/// How long before a cut the next clip's decoder starts.
const PREROLL: f64 = 0.5;
/// How long the playhead rests during a drag before the exact frame replaces the keyframe.
const SETTLE: Duration = Duration::from_millis(120);
/// Decoders kept open, least recently used closed first.
const MAX_DECODERS: usize = 6;

struct Slot {
    clip: ClipId,
    decoder: VideoDecoder,
    used: Instant,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Paused,
    Playing,
    Scrubbing,
}

struct Playing {
    clip: ClipId,
    generation: u64,
    /// Timeline time of the clip's start, and the source time it starts from.
    clip_start: f64,
    source_in: f64,
    source_out: f64,
    /// The clock: a timeline time and when it was there. Starts with the first frame.
    clock: Option<(f64, Instant)>,
    pending: VecDeque<Frame>,
    /// The next clip, already decoding.
    next: Option<(ClipId, u64)>,
}

struct Scrub {
    clip: ClipId,
    generation: u64,
    t: f64,
    asked: Instant,
    exact: bool,
}

#[derive(Default)]
pub struct Stats {
    /// Gaps between frames while playing, in ms, and the gaps at cuts between clips.
    pub gaps: Vec<f64>,
    pub cut_gaps: Vec<f64>,
    /// Time from a scrub to its first frame and to its exact frame, in ms.
    pub first: Vec<f64>,
    pub exact: Vec<f64>,
    pub cache_hits: u64,
}

pub struct Preview {
    slots: Vec<Slot>,
    generation: u64,
    pub mode: Mode,
    /// Where the playhead is on the timeline.
    pub playhead: f64,
    pub image: Option<Arc<RenderImage>>,
    cache: FrameCache<(MediaId, i64)>,
    playing: Option<Playing>,
    scrub: Option<Scrub>,
    /// Whether the first frame of the current scrub has been shown.
    scrub_shown: bool,
    from_cache: Option<Arc<RenderImage>>,
    last_frame_at: Option<Instant>,
    /// Set at a cut, so the gap before the next clip's first frame is counted as a cut's.
    cut_pending: Option<Instant>,
    pub stats: Stats,
}

fn fps_of(project: &Project, media: MediaId) -> f64 {
    project.media(media).and_then(|m| m.info.as_ref()).map_or(30.0, |i| i.fps)
}

impl Preview {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            generation: 0,
            mode: Mode::Paused,
            playhead: 0.0,
            image: None,
            cache: FrameCache::new(1024 * 1024 * 1024),
            playing: None,
            scrub: None,
            scrub_shown: false,
            from_cache: None,
            last_frame_at: None,
            cut_pending: None,
            stats: Stats::default(),
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// The clip's decoder, opened if it isn't yet.
    fn decoder(&mut self, project: &Project, clip: ClipId) -> Option<&VideoDecoder> {
        if let Some(i) = self.slots.iter().position(|s| s.clip == clip) {
            self.slots[i].used = Instant::now();
            return Some(&self.slots[i].decoder);
        }
        let c = project.clips.iter().find(|c| c.id == clip)?;
        let path = &project.media(c.media)?.path;
        if self.slots.len() >= MAX_DECODERS {
            let busy: Vec<ClipId> = self
                .playing
                .iter()
                .flat_map(|p| [Some(p.clip), p.next.map(|n| n.0)])
                .flatten()
                .chain(self.scrub.iter().map(|s| s.clip))
                .collect();
            if let Some(oldest) = self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| !busy.contains(&s.clip))
                .min_by_key(|(_, s)| s.used)
                .map(|(i, _)| i)
            {
                self.slots.remove(oldest);
            }
        }
        let decoder = VideoDecoder::start(path, Some(PREVIEW_HEIGHT), Hw::None);
        self.slots.push(Slot { clip, decoder, used: Instant::now() });
        self.slots.last().map(|s| &s.decoder)
    }

    fn send(&mut self, project: &Project, clip: ClipId, command: Command) {
        if let Some(d) = self.decoder(project, clip) {
            d.send(command);
        }
    }

    /// Frames from a clip's decoder, without opening one.
    fn frames_from(&mut self, clip: ClipId, generation: u64, into: &mut VecDeque<Frame>, max: usize) {
        let Some(slot) = self.slots.iter().find(|s| s.clip == clip) else { return };
        while into.len() < max {
            match slot.decoder.frames.try_recv() {
                Ok(f) if f.generation == generation => into.push_back(f),
                Ok(_) => {}
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn stop_playing(&mut self, project: &Project) {
        if let Some(p) = self.playing.take() {
            self.send(project, p.clip, Command::Pause);
            if let Some((next, _)) = p.next {
                self.send(project, next, Command::Pause);
            }
        }
    }

    pub fn play(&mut self, project: &Project, t: f64) {
        self.stop_playing(project);
        self.scrub = None;
        let t = if t >= project.duration() - 0.05 { 0.0 } else { t };
        let Some(place) = project.place(t) else { return };
        let c = &project.clips[place.index];
        let (source_in, source_out) = (c.source_in, c.source_out);
        let generation = self.next_generation();
        self.send(project, place.clip, Command::Play { generation, from: place.source_t });
        self.playing = Some(Playing {
            clip: place.clip,
            generation,
            clip_start: place.clip_start,
            source_in,
            source_out,
            clock: None,
            pending: VecDeque::new(),
            next: None,
        });
        self.mode = Mode::Playing;
        self.playhead = t;
        self.last_frame_at = None;
    }

    pub fn pause(&mut self, project: &Project) {
        self.stop_playing(project);
        self.mode = Mode::Paused;
    }

    pub fn toggle(&mut self, project: &Project) {
        if self.mode == Mode::Playing {
            self.pause(project);
        } else {
            self.play(project, self.playhead);
        }
    }

    /// Shows the timeline at `t`: a nearby keyframe at once if `exact` is false, the exact frame
    /// once the playhead rests.
    pub fn scrub(&mut self, project: &Project, t: f64, exact: bool) {
        self.stop_playing(project);
        self.mode = Mode::Scrubbing;
        let t = t.clamp(0.0, project.duration());
        self.playhead = t;
        let Some(place) = project.place(t) else {
            self.image = None;
            return;
        };
        let key = (place.media, (place.source_t * fps_of(project, place.media)).round() as i64);
        let generation = self.next_generation();
        if let Some(image) = self.cache.get(key) {
            self.from_cache = Some(image);
            self.scrub = Some(Scrub { clip: place.clip, generation, t, asked: Instant::now(), exact: true });
            self.scrub_shown = true;
            self.stats.cache_hits += 1;
            self.stats.first.push(0.0);
            self.stats.exact.push(0.0);
            return;
        }
        self.send(project, place.clip, Command::Scrub { generation, t: place.source_t, exact });
        self.scrub = Some(Scrub { clip: place.clip, generation, t, asked: Instant::now(), exact });
        self.scrub_shown = false;
    }

    /// The timeline changed: forget decoders for clips that are gone, and redraw where we are.
    pub fn timeline_changed(&mut self, project: &Project) {
        self.slots.retain(|s| project.index_of(s.clip).is_some());
        let t = self.playhead.min(project.duration());
        if project.clips.is_empty() {
            self.stop_playing(project);
            self.image = None;
            self.mode = Mode::Paused;
            return;
        }
        if self.mode == Mode::Playing {
            self.play(project, t);
        } else {
            self.scrub(project, t, true);
        }
    }

    fn set_image(&mut self, image: Arc<RenderImage>, window: &mut Window, cx: &mut App) {
        if let Some(old) = self.image.replace(image)
            && !self.cache.contains(&old)
            && !self.image.as_ref().is_some_and(|c| Arc::ptr_eq(c, &old))
        {
            cx.drop_image(old, Some(window));
        }
    }

    fn show(&mut self, project: &Project, media: MediaId, frame: Frame, window: &mut Window, cx: &mut App) {
        let key = (media, (frame.pts * fps_of(project, media)).round() as i64);
        let bytes = frame.bgra.len() as u64;
        let exact = frame.exact;
        let Some(buffer) = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra) else { return };
        let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
        self.set_image(image.clone(), window, cx);
        if exact {
            for old in self.cache.insert(key, image, bytes) {
                if !self.image.as_ref().is_some_and(|c| Arc::ptr_eq(c, &old)) {
                    cx.drop_image(old, Some(window));
                }
            }
        }
    }

    fn media_of(project: &Project, clip: ClipId) -> Option<MediaId> {
        project.clips.iter().find(|c| c.id == clip).map(|c| c.media)
    }

    /// Called every frame: picks up decoded frames and shows the right one. Gives whether it
    /// needs another frame drawn soon.
    pub fn tick(&mut self, project: &Project, window: &mut Window, cx: &mut App) -> bool {
        match self.mode {
            Mode::Playing => self.tick_playing(project, window, cx),
            Mode::Scrubbing | Mode::Paused => self.tick_scrubbing(project, window, cx),
        }
    }

    fn tick_scrubbing(&mut self, project: &Project, window: &mut Window, cx: &mut App) -> bool {
        if let Some(image) = self.from_cache.take() {
            self.set_image(image, window, cx);
        }
        let Some((clip, generation, t, asked, exact)) =
            self.scrub.as_ref().map(|s| (s.clip, s.generation, s.t, s.asked, s.exact))
        else {
            return false;
        };
        let mut got = VecDeque::new();
        self.frames_from(clip, generation, &mut got, 8);
        if let Some(frame) = got.pop_back() {
            let ms = asked.elapsed().as_secs_f64() * 1000.0;
            if !self.scrub_shown {
                self.stats.first.push(ms);
                self.scrub_shown = true;
            }
            let is_exact = frame.exact;
            if is_exact {
                self.stats.exact.push(ms);
            }
            if let Some(media) = Self::media_of(project, clip) {
                self.show(project, media, frame, window, cx);
            }
            if is_exact && exact {
                self.mode = Mode::Paused;
                return false;
            }
        }
        if !exact && asked.elapsed() >= SETTLE {
            self.scrub(project, t, true);
        }
        true
    }

    fn tick_playing(&mut self, project: &Project, window: &mut Window, cx: &mut App) -> bool {
        let Some((clip, generation)) = self.playing.as_ref().map(|p| (p.clip, p.generation)) else {
            return false;
        };
        let mut pending = std::mem::take(&mut self.playing.as_mut().unwrap().pending);
        self.frames_from(clip, generation, &mut pending, 4);
        let p = self.playing.as_mut().unwrap();
        // Frames past the clip's out point belong to nothing.
        pending.retain(|f| f.pts < p.source_out - 0.001);
        let to_timeline = |pts: f64| p.clip_start + (pts - p.source_in);

        let now = Instant::now();
        let clock = match p.clock {
            Some((t0, at)) => t0 + (now - at).as_secs_f64(),
            None => match pending.front() {
                Some(f) => {
                    let t0 = to_timeline(f.pts);
                    p.clock = Some((t0, now));
                    t0
                }
                None => {
                    p.pending = pending;
                    return true;
                }
            },
        };
        let mut chosen = None;
        while pending.front().is_some_and(|f| to_timeline(f.pts) <= clock + 0.001) {
            chosen = pending.pop_front();
        }
        let clip_end = p.clip_start + (p.source_out - p.source_in);
        let index = project.index_of(clip);
        let next_clip = index.and_then(|i| project.clips.get(i + 1)).cloned();
        let wants_preroll = p.next.is_none() && clip_end - clock < PREROLL && next_clip.is_some();
        p.pending = pending;
        self.playhead = clock.min(project.duration());

        if let Some(frame) = chosen {
            if let Some(last) = self.last_frame_at {
                let gap = (now - last).as_secs_f64() * 1000.0;
                self.stats.gaps.push(gap);
                if self.cut_pending.take().is_some() {
                    self.stats.cut_gaps.push(gap);
                }
            }
            self.last_frame_at = Some(now);
            if let Some(media) = Self::media_of(project, clip) {
                self.show(project, media, frame, window, cx);
            }
        }

        if wants_preroll && let Some(next) = &next_clip {
            let g = self.next_generation();
            self.send(project, next.id, Command::Play { generation: g, from: next.source_in });
            if let Some(p) = self.playing.as_mut() {
                p.next = Some((next.id, g));
            }
        }

        if clock >= clip_end {
            let finished = self.playing.take().unwrap();
            self.send(project, finished.clip, Command::Pause);
            match (next_clip, finished.next) {
                (Some(next), Some((_, g))) => {
                    self.cut_pending = Some(Instant::now());
                    self.playing = Some(Playing {
                        clip: next.id,
                        generation: g,
                        clip_start: clip_end,
                        source_in: next.source_in,
                        source_out: next.source_out,
                        // Keep the clock running through the cut.
                        clock: finished.clock,
                        pending: VecDeque::new(),
                        next: None,
                    });
                }
                _ => {
                    self.mode = Mode::Paused;
                    self.playhead = project.duration();
                    return false;
                }
            }
        }
        true
    }
}
