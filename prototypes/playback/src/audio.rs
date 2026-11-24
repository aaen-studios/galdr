//! Audio output through cpal. The samples the device has taken are the playback clock.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample, Stream, StreamConfig};

use crate::decode;

struct Shared {
    buf: Mutex<VecDeque<f32>>,
    space: Condvar,
    /// Bumped on every seek; a feeder with an older number stops.
    generation: AtomicU64,
    /// Frames (one sample per channel) the device has taken since the last seek.
    played: AtomicU64,
    paused: AtomicBool,
    underruns: AtomicU64,
}

pub struct AudioOut {
    shared: Arc<Shared>,
    _stream: Stream,
    pub rate: u32,
    pub channels: u16,
    base: f64,
    path: PathBuf,
}

impl AudioOut {
    pub fn new(path: PathBuf) -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().context("no audio output device")?;
        let supported = device.default_output_config()?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let shared = Arc::new(Shared {
            buf: Mutex::new(VecDeque::new()),
            space: Condvar::new(),
            generation: AtomicU64::new(0),
            played: AtomicU64::new(0),
            paused: AtomicBool::new(true),
            underruns: AtomicU64::new(0),
        });
        let stream = match format {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config, shared.clone())?,
            cpal::SampleFormat::I16 => build::<i16>(&device, &config, shared.clone())?,
            cpal::SampleFormat::I32 => build::<i32>(&device, &config, shared.clone())?,
            cpal::SampleFormat::U16 => build::<u16>(&device, &config, shared.clone())?,
            other => anyhow::bail!("unsupported sample format {other:?}"),
        };
        stream.play()?;
        Ok(Self {
            shared,
            _stream: stream,
            rate: config.sample_rate,
            channels: config.channels,
            base: 0.0,
            path,
        })
    }

    /// Seconds into the file the listener is hearing.
    pub fn clock(&self) -> f64 {
        self.base + self.shared.played.load(Ordering::Relaxed) as f64 / self.rate as f64
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// Drops what's buffered and starts decoding from `t`. With `feed` false the buffer stays
    /// empty, for scrubbing.
    pub fn seek(&mut self, t: f64, feed: bool) {
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut buf = self.shared.buf.lock().unwrap();
            buf.clear();
            self.shared.played.store(0, Ordering::SeqCst);
        }
        self.shared.space.notify_all();
        self.base = t;
        if !feed {
            return;
        }
        let shared = self.shared.clone();
        let max = self.rate as usize * self.channels as usize / 2; // half a second ahead
        decode::start_audio(self.path.clone(), t, self.rate, self.channels, move |chunk| {
            let mut buf = shared.buf.lock().unwrap();
            while buf.len() > max && shared.generation.load(Ordering::SeqCst) == generation {
                buf = shared.space.wait(buf).unwrap();
            }
            if shared.generation.load(Ordering::SeqCst) != generation {
                return false;
            }
            buf.extend(chunk.iter().copied());
            true
        });
    }
}

fn build<T: SizedSample + FromSample<f32> + Send + 'static>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<Shared>,
) -> Result<Stream> {
    let channels = config.channels as usize;
    let stream = device.build_output_stream(
        *config,
        move |out: &mut [T], _| {
            if shared.paused.load(Ordering::Relaxed) {
                out.fill(T::from_sample(0.0));
                return;
            }
            let mut buf = shared.buf.lock().unwrap();
            let available = buf.len().min(out.len()) / channels * channels;
            for (o, s) in out.iter_mut().zip(buf.drain(..available)) {
                *o = T::from_sample(s);
            }
            for o in &mut out[available..] {
                *o = T::from_sample(0.0);
            }
            drop(buf);
            if available < out.len() {
                shared.underruns.fetch_add(1, Ordering::Relaxed);
            }
            shared.played.fetch_add((available / channels) as u64, Ordering::Relaxed);
            shared.space.notify_all();
        },
        |e| eprintln!("audio stream: {e}"),
        None,
    )?;
    Ok(stream)
}
