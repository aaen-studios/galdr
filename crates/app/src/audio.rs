//! The timeline's sound. A feeder thread decodes the clips in order from the playhead into a
//! buffer, with silence for clips that have none, and the output takes from it. The samples the
//! output has taken are the playback clock, so the picture follows the sound.
//!
//! The output is the default device, or, with `silent`, a thread that takes samples at the same
//! rate and plays nothing: for tests, and machines without a device.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample, Stream, StreamConfig};
use media::AudioReader;

/// A clip's sound: where it is on the timeline, and the part of the file under it.
#[derive(Clone, Debug)]
pub struct Segment {
    pub path: PathBuf,
    pub has_audio: bool,
    pub source_in: f64,
    pub source_out: f64,
}

/// How much the feeder keeps ahead of the output.
const AHEAD: f64 = 0.4;
/// How long a scrub's sound is.
pub const SNIPPET: f64 = 0.07;

struct Shared {
    buf: Mutex<VecDeque<f32>>,
    space: Condvar,
    /// Bumped on every start and stop; a feeder with an older number gives up.
    generation: AtomicU64,
    /// Frames (a sample per channel) the output has taken since the last start, and when it last
    /// took some, in ns since `epoch`.
    played: AtomicU64,
    took_at: AtomicU64,
    paused: AtomicBool,
    /// The feeder for this generation has written its last sample.
    fed: AtomicBool,
    underruns: AtomicU64,
    epoch: Instant,
    /// For the silent output's thread.
    closed: AtomicBool,
}

impl Shared {
    /// What the output calls: fills `out` from the buffer, silence for the rest.
    fn take(&self, out: &mut [f32], channels: usize) {
        if self.paused.load(Ordering::Relaxed) {
            out.fill(0.0);
            return;
        }
        let mut buf = self.buf.lock().unwrap();
        let available = buf.len().min(out.len()) / channels * channels;
        for (o, s) in out.iter_mut().zip(buf.drain(..available)) {
            *o = s;
        }
        out[available..].fill(0.0);
        drop(buf);
        if available < out.len() && !self.fed.load(Ordering::Relaxed) {
            self.underruns.fetch_add(1, Ordering::Relaxed);
        }
        if available > 0 {
            self.played.fetch_add((available / channels) as u64, Ordering::Relaxed);
            self.took_at.store(self.epoch.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        self.space.notify_all();
    }
}

enum Output {
    Device(#[allow(dead_code)] Stream),
    Silent,
}

pub struct Audio {
    shared: Arc<Shared>,
    output: Output,
    pub rate: u32,
    pub channels: u16,
    /// The timeline time of the first sample after the last start.
    base: f64,
}

impl Audio {
    pub fn new(silent: bool) -> Self {
        let shared = Arc::new(Shared {
            buf: Mutex::new(VecDeque::new()),
            space: Condvar::new(),
            generation: AtomicU64::new(0),
            played: AtomicU64::new(0),
            took_at: AtomicU64::new(0),
            paused: AtomicBool::new(true),
            fed: AtomicBool::new(true),
            underruns: AtomicU64::new(0),
            epoch: Instant::now(),
            closed: AtomicBool::new(false),
        });
        if !silent {
            match open_device(shared.clone()) {
                Ok((stream, rate, channels)) => {
                    return Self { shared, output: Output::Device(stream), rate, channels, base: 0.0 };
                }
                Err(e) => eprintln!("no sound: {e:#}"),
            }
        }
        let (rate, channels) = (48_000, 2);
        let s = shared.clone();
        thread::Builder::new()
            .name("silent-output".into())
            .spawn(move || {
                // Takes what a device would, every 5 ms.
                let mut last = Instant::now();
                let mut owed = 0.0f64;
                let mut chunk = Vec::new();
                while !s.closed.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(5));
                    let now = Instant::now();
                    owed += (now - last).as_secs_f64() * rate as f64;
                    last = now;
                    let frames = owed.floor() as usize;
                    owed -= frames as f64;
                    chunk.resize(frames * channels, 0.0);
                    s.take(&mut chunk, channels);
                }
            })
            .expect("start the silent output");
        Self { shared, output: Output::Silent, rate, channels: channels as u16, base: 0.0 }
    }

    pub fn silent(&self) -> bool {
        matches!(self.output, Output::Silent)
    }

    /// The timeline time being heard. Between the output's takes it runs on by the wall clock,
    /// at most one take's worth, so the picture moves smoothly.
    pub fn clock(&self) -> f64 {
        let played = self.shared.played.load(Ordering::Relaxed);
        let mut t = self.base + played as f64 / self.rate as f64;
        if played > 0 && !self.shared.paused.load(Ordering::Relaxed) {
            let since = self.shared.epoch.elapsed().as_nanos() as u64;
            let since = since.saturating_sub(self.shared.took_at.load(Ordering::Relaxed)) as f64 / 1e9;
            let buffered = !self.shared.buf.lock().unwrap().is_empty();
            if buffered {
                t += since.min(0.012);
            }
        }
        t
    }

    /// Whether the output has started taking samples since the last start.
    pub fn flowing(&self) -> bool {
        self.shared.played.load(Ordering::Relaxed) > 0
    }

    /// Enough is buffered to start without a gap.
    pub fn ready(&self) -> bool {
        let want = (0.05 * self.rate as f64) as usize * self.channels as usize;
        self.shared.fed.load(Ordering::Relaxed) || self.shared.buf.lock().unwrap().len() >= want
    }

    /// The feeder has written everything and the output has taken it.
    pub fn finished(&self) -> bool {
        self.shared.fed.load(Ordering::Relaxed) && self.shared.buf.lock().unwrap().is_empty()
    }

    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// Stops the sound and drops what's buffered.
    pub fn stop(&mut self) {
        self.shared.paused.store(true, Ordering::Relaxed);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.shared.buf.lock().unwrap().clear();
        self.shared.fed.store(true, Ordering::Relaxed);
        self.shared.space.notify_all();
    }

    /// Starts feeding the timeline from `t`, paused until `resume`, for `length` seconds or to the
    /// end.
    pub fn start(&mut self, segments: Vec<Segment>, t: f64, length: Option<f64>) {
        self.stop();
        let generation = self.shared.generation.load(Ordering::SeqCst);
        self.shared.played.store(0, Ordering::SeqCst);
        self.shared.fed.store(false, Ordering::SeqCst);
        self.base = t;
        let shared = self.shared.clone();
        let (rate, channels) = (self.rate, self.channels);
        thread::Builder::new()
            .name("audio-feeder".into())
            .spawn(move || {
                if let Err(e) = feed(&shared, generation, segments, t, length, rate, channels) {
                    eprintln!("audio: {e:#}");
                }
                if shared.generation.load(Ordering::SeqCst) == generation {
                    shared.fed.store(true, Ordering::SeqCst);
                }
            })
            .expect("start the audio feeder");
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Relaxed);
    }

    /// A scrub's short sound at `t`, if the last one is done.
    pub fn snippet(&mut self, segments: Vec<Segment>, t: f64) {
        if self.finished() {
            self.start(segments, t, Some(SNIPPET));
            self.resume();
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
        self.shared.closed.store(true, Ordering::Relaxed);
    }
}

/// Writes the timeline's samples from `t` into the buffer, waiting for room, until the end, or
/// `length` seconds, or a newer start.
fn feed(
    shared: &Shared,
    generation: u64,
    segments: Vec<Segment>,
    t: f64,
    length: Option<f64>,
    rate: u32,
    channels: u16,
) -> Result<()> {
    let ch = channels as usize;
    let max = (AHEAD * rate as f64) as usize * ch;
    let stop_at = length.map(|l| t + l);
    // Samples are counted on the timeline from zero, so cuts land on exact samples and rounding
    // never adds up.
    let sample = |time: f64| (time * rate as f64).round() as u64;
    let mut written = sample(t);
    let push = |chunk: &[f32]| -> bool {
        let mut buf = shared.buf.lock().unwrap();
        while buf.len() > max && shared.generation.load(Ordering::SeqCst) == generation {
            buf = shared.space.wait_timeout(buf, Duration::from_millis(50)).unwrap().0;
        }
        if shared.generation.load(Ordering::SeqCst) != generation {
            return false;
        }
        buf.extend(chunk.iter().copied());
        true
    };
    let mut start = 0.0;
    let mut decoded = Vec::new();
    for s in &segments {
        let end = start + (s.source_out - s.source_in);
        let seg_end = stop_at.map_or(end, |x| x.min(end));
        if sample(seg_end) <= written {
            start = end;
            if stop_at.is_some_and(|x| x <= end) {
                break;
            }
            continue;
        }
        let into = written as f64 / rate as f64 - start;
        let mut left = (sample(seg_end) - written) as usize * ch;
        let mut reader = if s.has_audio { AudioReader::open(&s.path, rate, channels).ok().flatten() } else { None };
        if let Some(r) = reader.as_mut() {
            r.seek(s.source_in + into).context("seeking the audio")?;
        }
        while left > 0 {
            decoded.clear();
            let more = match reader.as_mut() {
                Some(r) => r.read(&mut decoded)?,
                None => false,
            };
            if !more && decoded.is_empty() {
                // No sound here, or the file's ends early: silence to the clip's end.
                decoded.resize(left.min(4096 * ch), 0.0);
            }
            let n = decoded.len().min(left);
            if !push(&decoded[..n]) {
                return Ok(());
            }
            left -= n;
            if !more && reader.is_some() {
                reader = None;
            }
        }
        written = sample(seg_end);
        start = end;
        if stop_at.is_some_and(|x| x <= end) {
            break;
        }
    }
    Ok(())
}

fn open_device(shared: Arc<Shared>) -> Result<(Stream, u32, u16)> {
    let host = cpal::default_host();
    let device = host.default_output_device().context("no audio output device")?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, shared)?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, shared)?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, shared)?,
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, shared)?,
        other => anyhow::bail!("unsupported sample format {other:?}"),
    };
    stream.play()?;
    Ok((stream, config.sample_rate, config.channels))
}

fn build<T: SizedSample + FromSample<f32> + Send + 'static>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<Shared>,
) -> Result<Stream> {
    let channels = config.channels as usize;
    let mut scratch = Vec::new();
    let stream = device.build_output_stream(
        *config,
        move |out: &mut [T], _| {
            scratch.resize(out.len(), 0.0f32);
            shared.take(&mut scratch, channels);
            for (o, s) in out.iter_mut().zip(&scratch) {
                *o = T::from_sample(*s);
            }
        },
        |e| eprintln!("audio stream: {e}"),
        None,
    )?;
    Ok(stream)
}
