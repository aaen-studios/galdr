//! The frame engine: exact frames, decoded ahead of need by a pool of workers into one cache.
//!
//! The caller says what it wants as ranges of source time in priority order (the frame under the
//! playhead first, then what's around or ahead of it) and reads frames back from the cache. Each
//! worker takes the first wanted frame that nobody has or is decoding, and decodes from the
//! keyframe before it to the end of that group of pictures, so several workers fill neighbouring
//! groups at once. Frames are converted at the size they're shown at: the source fitted into the
//! viewer, never larger than the source. What the cache holds is what the screen shows.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::decoder::{Hw, Info, State};

pub type SourceId = u64;

/// Frames of a source to have ready, from `from` to `to` seconds into it, both included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Want {
    pub source: SourceId,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Decoders working at once, and the threads each decodes and converts with.
    pub workers: usize,
    pub threads: usize,
    /// Bytes of frames to keep.
    pub budget: u64,
    /// A GPU to decode the frame on screen with, besides the CPU workers, or none.
    pub hw: Hw,
}

impl Default for Settings {
    fn default() -> Self {
        // Measured on 32 threads: 4 decoders of 8 threads each reach a random frame as fast as
        // one of 32 and keep a fast drag exact; 16 each was slower at both.
        let cores = thread::available_parallelism().map_or(8, |n| n.get());
        let workers = (cores / 6).clamp(2, 4);
        Self { workers, threads: (cores / workers).max(2), budget: 3 << 30, hw: Hw::None }
    }
}

/// While decoding toward the first wanted frame of a group, keep a frame on the way at least this
/// often, so a far jump in a long group shows movement before it lands.
const PROGRESS_EVERY: Duration = Duration::from_millis(33);
/// Open decoders a worker keeps, least recently used closed first.
const OPEN_PER_WORKER: usize = 3;

struct Source {
    path: PathBuf,
    info: Info,
    frame_time: f64,
    /// Frame numbers of the keyframes, from the file's index; empty when it has none.
    keyframes: Vec<i64>,
    /// One past the last frame, once a decode has reached the end of the file.
    end: Option<i64>,
    out_height: u32,
    /// Threads its decoders run with: more for long groups, which one decoder works through alone.
    threads: usize,
    /// Groups that failed to decode, by their first frame, not to be tried again.
    failed: HashSet<i64>,
    broken: bool,
}

impl Source {
    /// The frame showing at `t`: the last one starting at or before it.
    fn index(&self, t: f64) -> i64 {
        (t / self.frame_time + 1e-3).floor().max(0.0) as i64
    }

    /// The last frame: known once a decode reached the end, guessed from the duration till then,
    /// with room for the duration being a little short.
    fn last(&self) -> i64 {
        self.end.map_or_else(|| (self.info.duration / self.frame_time).ceil() as i64 + 8, |e| e - 1)
    }

    /// The group of pictures frame `i` is in: its first frame and the next group's first frame.
    fn gop(&self, i: i64) -> (i64, i64) {
        if self.keyframes.is_empty() {
            // No index: work in two-second pieces, each seeking to the keyframe before it.
            let n = ((2.0 / self.frame_time).round() as i64).max(1);
            let k = i / n * n;
            return (k, k + n);
        }
        let p = self.keyframes.partition_point(|&k| k <= i);
        let k = if p == 0 { 0 } else { self.keyframes[p - 1] };
        (k, self.keyframes.get(p).copied().unwrap_or(self.last() + 1).max(i + 1))
    }

    fn range(&self, w: &Want) -> (i64, i64) {
        let last = self.last();
        (self.index(w.from).min(last), self.index(w.to).min(last))
    }
}

/// The source's height fitted into a `w` by `h` box, never above the source's own.
fn fit_height(info: &Info, (w, h): (u32, u32)) -> u32 {
    if w == 0 || h == 0 || info.width == 0 || info.height == 0 {
        return info.height;
    }
    let scale = (w as f64 / info.width as f64).min(h as f64 / info.height as f64).min(1.0);
    (((info.height as f64 * scale).round() as u32) & !1).max(2)
}

/// Threads for a source's decoders. Reaching a frame deep in a long group is one decoder's work
/// from the keyframe on, so more threads pay off there; with short groups the workers already
/// share the cores between them. Measured on 32 threads with 4 workers: a 10 s group reached
/// frames in 0.4 s at 24 threads against 1.1 s at 8, while 1 s groups dragged worse above 8.
fn threads_for(keyframes: &[i64], frame_time: f64, duration: f64, base: usize) -> usize {
    let group = match keyframes {
        [] => return base, // no index: decoded in 2 s pieces
        [_] => duration,
        [first, .., last] => (last - first) as f64 * frame_time / (keyframes.len() - 1) as f64,
    };
    let cores = thread::available_parallelism().map_or(8, |n| n.get());
    ((base as f64 * (group / 2.0).clamp(1.0, 3.0)).round() as usize).min(cores.max(base))
}

struct Entry<T> {
    value: T,
    bytes: u64,
    used: u64,
}

struct Cache<T> {
    map: HashMap<(SourceId, i64), Entry<T>>,
    bytes: u64,
    budget: u64,
    tick: u64,
}

impl<T: Clone> Cache<T> {
    fn get(&mut self, key: (SourceId, i64)) -> Option<T> {
        self.tick += 1;
        let e = self.map.get_mut(&key)?;
        e.used = self.tick;
        Some(e.value.clone())
    }

    fn insert(&mut self, key: (SourceId, i64), value: T, bytes: u64) {
        self.tick += 1;
        if let Some(old) = self.map.insert(key, Entry { value, bytes, used: self.tick }) {
            self.bytes -= old.bytes;
        }
        self.bytes += bytes;
        if self.bytes > self.budget {
            // Down to 95% at once, so this sort runs once every few dozen frames, not each one.
            let mut by_age: Vec<_> = self.map.iter().map(|(k, e)| (e.used, *k)).collect();
            by_age.sort_unstable_by_key(|(used, _)| *used);
            for (_, k) in by_age {
                if self.bytes <= self.budget / 20 * 19 || self.map.len() <= 1 {
                    break;
                }
                if let Some(e) = self.map.remove(&k) {
                    self.bytes -= e.bytes;
                }
            }
        }
    }

    fn clear(&mut self) {
        self.map.clear();
        self.bytes = 0;
    }
}

struct Running {
    id: u64,
    source: SourceId,
    gop_start: i64,
    from: i64,
    /// The last frame to decode, moved on while the job runs when frames after it are wanted.
    to: Arc<AtomicI64>,
    /// The last frame decoded.
    at: Arc<AtomicI64>,
    cancel: Arc<AtomicBool>,
    /// On the GPU worker.
    gpu: bool,
}

impl Running {
    fn to(&self) -> i64 {
        self.to.load(Ordering::Relaxed)
    }
}

struct Sched<T> {
    sources: HashMap<SourceId, Source>,
    cache: Cache<T>,
    wants: Vec<Want>,
    running: Vec<Running>,
    viewer: (u32, u32),
    /// Bumped when frames change size: what's decoded for an older one is thrown away.
    epoch: u64,
    next_id: u64,
    shutdown: bool,
    /// The GPU worker is waiting for a frame to decode.
    gpu_idle: bool,
}

struct Job {
    id: u64,
    source: SourceId,
    path: PathBuf,
    frame_time: f64,
    out_height: u32,
    threads: usize,
    gop_start: i64,
    from: i64,
    to: Arc<AtomicI64>,
    epoch: u64,
    at: Arc<AtomicI64>,
    cancel: Arc<AtomicBool>,
}

impl<T: Clone> Sched<T> {
    /// The first wanted frame nobody has or is decoding, as (source, group start, from, to). The
    /// GPU worker takes only the frame on screen, where reaching one frame fast is what counts;
    /// while it's free, the CPU workers leave that to it and decode ahead.
    fn next_job(&self, gpu: bool) -> Option<(SourceId, i64, i64, i64)> {
        for (n, w) in self.wants.iter().enumerate() {
            if gpu && n > 0 {
                return None;
            }
            if !gpu && n == 0 && self.gpu_idle {
                continue;
            }
            let Some(src) = self.sources.get(&w.source) else { continue };
            if src.broken || src.last() < 0 {
                continue;
            }
            let (a, b) = src.range(w);
            let mut i = a;
            let mut on_cpu = false;
            while i <= b {
                if self.cache.map.contains_key(&(w.source, i)) {
                    i += 1;
                    continue;
                }
                if let Some(r) = self.claim(w.source, i) {
                    on_cpu |= !r.gpu;
                    i = r.to() + 1;
                    continue;
                }
                let (k, next) = src.gop(i);
                if src.failed.contains(&k) {
                    i = next;
                    continue;
                }
                // On toward the end of the group: decoding up to here paid for the rest of it.
                // Stop short of frames another worker is on, and of those at the end already in;
                // ones in between are decoded through, which reaching the rest needs anyway.
                let mut j = (next - 1).min(src.last());
                for r in self.live(w.source) {
                    if r.from > i && r.from <= j {
                        j = r.from - 1;
                    }
                }
                while j > i && self.cache.map.contains_key(&(w.source, j)) {
                    j -= 1;
                }
                // A job earlier in the same group gets here sooner than a new one starting from
                // the keyframe, and the two would only slow each other down: carry it on instead.
                // That matters in long groups, where a second job could take seconds.
                if let Some(r) = self.live(w.source).filter(|r| r.gop_start == k && r.to() < i).max_by_key(|r| r.to()) {
                    r.to.store(j, Ordering::Relaxed);
                    on_cpu |= !r.gpu;
                    i = j + 1;
                    continue;
                }
                return Some((w.source, k, i, j));
            }
            // The first want is the frame on screen: until it's in, nothing else takes the CPU
            // from a CPU worker decoding it. One on the GPU needs little of it.
            if n == 0 && on_cpu {
                return None;
            }
        }
        None
    }

    fn live(&self, source: SourceId) -> impl Iterator<Item = &Running> {
        self.running.iter().filter(move |r| r.source == source && !r.cancel.load(Ordering::Relaxed))
    }

    fn claim(&self, source: SourceId, i: i64) -> Option<&Running> {
        self.live(source).find(|r| r.from <= i && i <= r.to())
    }

    /// Stops jobs whose frames nobody wants any more.
    fn cancel_unwanted(&self) {
        for r in &self.running {
            let Some(src) = self.sources.get(&r.source) else {
                r.cancel.store(true, Ordering::Relaxed);
                continue;
            };
            let from = r.from.max(r.at.load(Ordering::Relaxed) + 1);
            let wanted = self.wants.iter().any(|w| {
                if w.source != r.source {
                    return false;
                }
                let (a, b) = src.range(w);
                a <= r.to() && b >= from
            });
            if !wanted {
                r.cancel.store(true, Ordering::Relaxed);
            }
        }
    }
}

struct Shared<T> {
    sched: Mutex<Sched<T>>,
    wake: Condvar,
    make: Box<dyn Fn(u32, u32, Vec<u8>) -> T + Send + Sync>,
    settings: Settings,
    /// Frames added to the cache, ever: lets the caller see something new landed.
    landed: AtomicU64,
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, Sched<T>> {
        self.sched.lock().unwrap_or_else(|e| e.into_inner())
    }
}

struct Workers<T> {
    shared: Arc<Shared<T>>,
    handles: Vec<JoinHandle<()>>,
}

impl<T> Drop for Workers<T> {
    fn drop(&mut self) {
        {
            let mut s = self.shared.lock();
            s.shutdown = true;
            for r in &s.running {
                r.cancel.store(true, Ordering::Relaxed);
            }
        }
        self.shared.wake.notify_all();
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

/// Shared between clones; the workers stop when the last clone goes.
pub struct Engine<T> {
    shared: Arc<Shared<T>>,
    _workers: Arc<Workers<T>>,
}

impl<T> Clone for Engine<T> {
    fn clone(&self) -> Self {
        Self { shared: self.shared.clone(), _workers: self._workers.clone() }
    }
}

impl<T: Clone + Send + 'static> Engine<T> {
    /// `make` turns a converted frame (width, height, BGRA bytes) into what the cache keeps, such
    /// as an image ready for the GPU. It runs on the workers.
    pub fn new(settings: Settings, make: impl Fn(u32, u32, Vec<u8>) -> T + Send + Sync + 'static) -> Self {
        let _ = ffmpeg_next::init();
        let shared = Arc::new(Shared {
            sched: Mutex::new(Sched {
                sources: HashMap::new(),
                cache: Cache { map: HashMap::new(), bytes: 0, budget: settings.budget, tick: 0 },
                wants: Vec::new(),
                running: Vec::new(),
                viewer: (0, 0),
                epoch: 0,
                next_id: 0,
                shutdown: false,
                gpu_idle: false,
            }),
            wake: Condvar::new(),
            make: Box::new(make),
            settings,
            landed: AtomicU64::new(0),
        });
        // The CPU workers, and with a GPU, one more that decodes there.
        let gpu = settings.hw != Hw::None;
        let handles = (0..settings.workers.max(1) + gpu as usize)
            .filter_map(|i| {
                let shared = shared.clone();
                let on_gpu = gpu && i == settings.workers.max(1);
                let name = if on_gpu { "frame-worker-gpu".to_string() } else { format!("frame-worker-{i}") };
                thread::Builder::new().name(name).spawn(move || worker(shared, on_gpu)).ok()
            })
            .collect();
        Self { shared: shared.clone(), _workers: Arc::new(Workers { shared, handles }) }
    }

    /// Reads what `path` is and where its keyframes are. Blocks for as long as opening the file
    /// takes, so call it off the UI thread.
    pub fn add_source(&self, id: SourceId, path: &Path) -> Result<Info> {
        let mut state = State::open(&path.to_path_buf(), None, Hw::None, 1)?;
        let frame_time = state.frame_time;
        // The index holds decode times, which with B-frames run ahead of the presentation times
        // frames are shown at. The first packet's presentation time shows how far.
        let shift = match (state.keyframes.first().copied(), state.first_pts()?) {
            (Some(first), Some(pts)) => pts - first,
            _ => 0.0,
        };
        let keyframes: Vec<i64> = state.keyframes.iter().map(|k| ((k + shift) / frame_time).round() as i64).collect();
        let info = state.info.clone();
        drop(state);
        let mut s = self.shared.lock();
        let out_height = fit_height(&info, s.viewer);
        let threads = threads_for(&keyframes, frame_time, info.duration, self.shared.settings.threads);
        s.sources.insert(
            id,
            Source {
                path: path.to_path_buf(),
                info: info.clone(),
                frame_time,
                keyframes,
                end: None,
                out_height,
                threads,
                failed: HashSet::new(),
                broken: false,
            },
        );
        drop(s);
        self.shared.wake.notify_all();
        Ok(info)
    }

    /// The viewer's size in device pixels. Frames are converted to fit it, so a new size starts
    /// the cache over.
    pub fn set_viewer(&self, w: u32, h: u32) {
        let mut s = self.shared.lock();
        if s.viewer == (w, h) {
            return;
        }
        s.viewer = (w, h);
        let changed = s.sources.values().any(|src| fit_height(&src.info, (w, h)) != src.out_height);
        if !changed {
            return;
        }
        for src in s.sources.values_mut() {
            src.out_height = fit_height(&src.info, (w, h));
        }
        s.epoch += 1;
        s.cache.clear();
        for r in &s.running {
            r.cancel.store(true, Ordering::Relaxed);
        }
        drop(s);
        self.shared.wake.notify_all();
    }

    /// What to have ready, most wanted first. Replaces what was wanted before.
    pub fn want(&self, wants: Vec<Want>) {
        let mut s = self.shared.lock();
        if s.wants == wants {
            return;
        }
        s.wants = wants;
        s.cancel_unwanted();
        drop(s);
        self.shared.wake.notify_all();
    }

    /// The frame showing at `t` seconds into `source`, if it's ready.
    pub fn get(&self, source: SourceId, t: f64) -> Option<T> {
        let mut s = self.shared.lock();
        let i = s.sources.get(&source).map(|src| src.index(t).min(src.last()))?;
        s.cache.get((source, i))
    }

    /// The ready frame closest to `t`, looking no further than `lo` and `hi`, with its time.
    pub fn nearest(&self, source: SourceId, t: f64, lo: f64, hi: f64) -> Option<(T, f64)> {
        let mut s = self.shared.lock();
        let src = s.sources.get(&source)?;
        let (ft, i, a, b) = (src.frame_time, src.index(t).min(src.last()), src.index(lo), src.index(hi).min(src.last()));
        for d in 0..=600 {
            for j in [i - d, i + d] {
                if j >= a && j <= b && s.cache.map.contains_key(&(source, j)) {
                    return s.cache.get((source, j)).map(|v| (v, j as f64 * ft));
                }
            }
            if i - d < a && i + d > b {
                break;
            }
        }
        None
    }

    /// Whether frames are being decoded or wanted ones are missing.
    pub fn busy(&self) -> bool {
        let s = self.shared.lock();
        !s.running.is_empty() || s.next_job(false).is_some() || s.next_job(true).is_some()
    }

    /// Counts frames that have landed in the cache, ever.
    pub fn landed(&self) -> u64 {
        self.shared.landed.load(Ordering::Relaxed)
    }

    /// Frames and bytes in the cache.
    pub fn usage(&self) -> (usize, u64) {
        let s = self.shared.lock();
        (s.cache.map.len(), s.cache.bytes)
    }

    pub fn settings(&self) -> Settings {
        self.shared.settings
    }
}

fn worker<T: Clone + Send + 'static>(shared: Arc<Shared<T>>, gpu: bool) {
    // Open decoders, most recently used last.
    let mut open: Vec<(SourceId, State)> = Vec::new();
    loop {
        let job = {
            let mut s = shared.lock();
            loop {
                if s.shutdown {
                    return;
                }
                if let Some((source, gop_start, from, to)) = s.next_job(gpu) {
                    if gpu {
                        s.gpu_idle = false;
                    }
                    let src = &s.sources[&source];
                    let (path, frame_time, out_height, threads) = (src.path.clone(), src.frame_time, src.out_height, src.threads);
                    s.next_id += 1;
                    let job = Job {
                        id: s.next_id,
                        source,
                        path,
                        frame_time,
                        out_height,
                        threads,
                        gop_start,
                        from,
                        to: Arc::new(AtomicI64::new(to)),
                        epoch: s.epoch,
                        at: Arc::new(AtomicI64::new(i64::MIN)),
                        cancel: Arc::new(AtomicBool::new(false)),
                    };
                    s.running.push(Running {
                        id: job.id,
                        source,
                        gop_start,
                        from,
                        to: job.to.clone(),
                        at: job.at.clone(),
                        cancel: job.cancel.clone(),
                        gpu,
                    });
                    break job;
                }
                if gpu {
                    s.gpu_idle = true;
                    drop(s);
                    // CPU workers may be leaving the frame on screen to this one.
                    shared.wake.notify_all();
                    s = shared.lock();
                    if s.shutdown {
                        return;
                    }
                    if s.next_job(true).is_some() {
                        continue;
                    }
                }
                s = shared.wake.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };

        let state = match open.iter().position(|(id, _)| *id == job.source) {
            Some(i) => {
                let entry = open.remove(i);
                open.push(entry);
                Ok(())
            }
            None => State::open(&job.path, Some(job.out_height), if gpu { shared.settings.hw } else { Hw::None }, job.threads).map(|state| {
                if open.len() >= OPEN_PER_WORKER {
                    open.remove(0);
                }
                open.push((job.source, state));
            }),
        };
        let began = Instant::now();
        let outcome = match state {
            Ok(()) => {
                let state = &mut open.last_mut().unwrap().1;
                run(&shared, &job, state)
            }
            Err(e) => Err(e),
        };
        if std::env::var_os("GALDR_TRACE").is_some() {
            eprintln!(
                "  job {} ended after {:.1} ms at {}{}",
                job.id,
                began.elapsed().as_secs_f64() * 1000.0,
                job.at.load(Ordering::Relaxed),
                if job.cancel.load(Ordering::Relaxed) { ", canceled" } else { "" }
            );
        }

        let mut s = shared.lock();
        s.running.retain(|r| r.id != job.id);
        match outcome {
            Ok(Some(end)) => {
                if let Some(src) = s.sources.get_mut(&job.source) {
                    src.end = Some(src.end.map_or(end, |e| e.min(end)));
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("frame worker: {e:#}");
                if let Some(src) = s.sources.get_mut(&job.source) {
                    if open.iter().all(|(id, _)| *id != job.source) {
                        src.broken = true; // can't even open it
                    } else {
                        src.failed.insert(job.gop_start);
                    }
                }
                open.retain(|(id, _)| *id != job.source);
            }
        }
        drop(s);
        shared.wake.notify_all();
    }
}

/// Decodes one job's frames into the cache. Gives where the file ends, if it got there.
fn run<T: Clone + Send + 'static>(shared: &Shared<T>, job: &Job, state: &mut State) -> Result<Option<i64>> {
    state.set_out_height(Some(job.out_height), job.threads)?;
    // Frames shown before the first one wanted are passed over; those no other frame needs, most
    // B-frames, needn't be decoded at all.
    state.skip_before = Some((job.from as f64 - 0.5) * job.frame_time);
    let ft = job.frame_time;
    let index_of = |pts: f64| (pts / ft).round() as i64;
    // Carry on from where the decoder is when that's in this group, or just before it.
    let forward = state.position.map(index_of).is_some_and(|p| p < job.from && p >= job.gop_start - 1);
    if !forward {
        // Half a frame past the keyframe, so rounding can't land the seek on the one before.
        state.seek((job.gop_start as f64 + 0.5) * ft)?;
    }
    let mut last_kept = Instant::now();
    let mut prev: Option<(i64, T)> = None;
    let mut decoded_any = false;
    let trace = std::env::var_os("GALDR_TRACE").is_some();
    let started = Instant::now();
    let mut decoded = 0;
    loop {
        if trace && job.at.load(Ordering::Relaxed) == job.from {
            eprintln!(
                "job {} gop {} from {} to {}: {} {decoded} frames, {:.1} ms",
                job.id,
                job.gop_start,
                job.from,
                job.to.load(Ordering::Relaxed),
                if forward { "forward" } else { "seek" },
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        decoded += 1;
        if job.cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let Some((frame, pts)) = state.next()? else {
            // The end of the file: frames from here on don't exist.
            let end = prev.as_ref().map(|(i, _)| i + 1).or(state.position.map(|p| index_of(p) + 1));
            return Ok(Some(end.unwrap_or(if decoded_any { job.from } else { job.gop_start })));
        };
        decoded_any = true;
        let i = index_of(pts);
        job.at.store(i, Ordering::Relaxed);
        let in_range = i >= job.from && i <= job.to.load(Ordering::Relaxed);
        let on_the_way = i < job.from && last_kept.elapsed() >= PROGRESS_EVERY;
        let key = (job.source, i);
        let mut value = None;
        if in_range || on_the_way {
            let have = shared.lock().cache.get(key);
            value = match have {
                Some(v) => Some(v),
                None => {
                    let f = state.convert(&frame, 0, pts)?;
                    let bytes = f.bgra.len() as u64;
                    let v = (shared.make)(f.width, f.height, f.bgra);
                    let mut s = shared.lock();
                    if s.epoch != job.epoch {
                        return Ok(None);
                    }
                    s.cache.insert(key, v.clone(), bytes);
                    // A frame the file skips (variable frame rate) shows the one before it.
                    if let Some((p, pv)) = &prev {
                        for m in (p + 1).max(job.from)..i {
                            if !s.cache.map.contains_key(&(job.source, m)) {
                                s.cache.insert((job.source, m), pv.clone(), bytes);
                            }
                        }
                    }
                    drop(s);
                    shared.landed.fetch_add(1, Ordering::Relaxed);
                    last_kept = Instant::now();
                    Some(v)
                }
            };
        }
        if in_range {
            prev = value.map(|v| (i, v));
        }
        if i >= job.to.load(Ordering::Relaxed) {
            return Ok(None);
        }
    }
}
