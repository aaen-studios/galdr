//! Audio decoding in-process with libav: a file's first audio stream, resampled to the output's
//! rate and channels as interleaved f32, from any point in it.

use std::path::Path;
use std::ptr;

use anyhow::{Context as _, Result, bail};
use ff::ffi;
use ffmpeg_next as ff;

/// libswresample, to the output's format.
struct Resampler(*mut ffi::SwrContext);

// Only ever used by the thread that owns the reader.
unsafe impl Send for Resampler {}

impl Drop for Resampler {
    fn drop(&mut self) {
        unsafe { ffi::swr_free(&mut self.0) }
    }
}

pub struct AudioReader {
    input: ff::format::context::Input,
    stream: usize,
    decoder: ff::decoder::Audio,
    resampler: Resampler,
    /// Seconds per tick of the stream's time base, and the stream's first timestamp.
    tick: f64,
    start: i64,
    file_start: i64,
    rate: u32,
    channels: usize,
    eof: bool,
    /// After a seek: the time the next sample given should be, so earlier ones are dropped.
    skip_to: Option<f64>,
    frame: ff::frame::Audio,
    converted: Vec<f32>,
}

impl AudioReader {
    /// Opens the file's audio at `rate` and `channels`. None if it has no audio.
    pub fn open(path: &Path, rate: u32, channels: u16) -> Result<Option<Self>> {
        ff::init()?;
        // libav's warnings about seeks into AAC priming are expected here.
        ff::util::log::set_level(ff::util::log::Level::Error);
        let mut input = ff::format::input(path).with_context(|| format!("opening {}", path.display()))?;
        let Some(stream) = input.streams().best(ff::media::Type::Audio) else { return Ok(None) };
        let index = stream.index();
        let tb = stream.time_base();
        let tick = tb.numerator() as f64 / tb.denominator() as f64;
        let start = if stream.start_time() == ffi::AV_NOPTS_VALUE { 0 } else { stream.start_time() };
        let mut ctx = ff::codec::context::Context::from_parameters(stream.parameters())?;
        unsafe {
            // Plenty for audio, and it keeps the decode on this thread.
            (*ctx.as_mut_ptr()).thread_count = 1;
        }
        let decoder = ctx.decoder().audio()?;
        let file_start = unsafe { (*input.as_ptr()).start_time };
        let file_start = if file_start == ffi::AV_NOPTS_VALUE { 0 } else { file_start };
        // Don't read the video's packets at all.
        for i in 0..input.nb_streams() as usize {
            if i != index {
                unsafe {
                    let s = *(*input.as_mut_ptr()).streams.add(i);
                    (*s).discard = ffi::AVDiscard::AVDISCARD_ALL;
                }
            }
        }
        let resampler = unsafe {
            let d = decoder.as_ptr();
            let mut in_layout: ffi::AVChannelLayout = std::mem::zeroed();
            if (*d).ch_layout.nb_channels > 0 {
                ffi::av_channel_layout_copy(&mut in_layout, &(*d).ch_layout);
            } else {
                ffi::av_channel_layout_default(&mut in_layout, 2);
            }
            let mut out_layout: ffi::AVChannelLayout = std::mem::zeroed();
            ffi::av_channel_layout_default(&mut out_layout, channels as i32);
            let mut swr: *mut ffi::SwrContext = ptr::null_mut();
            let r = ffi::swr_alloc_set_opts2(
                &mut swr,
                &out_layout,
                ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                rate as i32,
                &in_layout,
                (*d).sample_fmt,
                (*d).sample_rate,
                0,
                ptr::null_mut(),
            );
            ffi::av_channel_layout_uninit(&mut in_layout);
            ffi::av_channel_layout_uninit(&mut out_layout);
            if r < 0 || swr.is_null() || ffi::swr_init(swr) < 0 {
                ffi::swr_free(&mut swr);
                bail!("can't resample this file's audio");
            }
            Resampler(swr)
        };
        Ok(Some(Self {
            input,
            stream: index,
            decoder,
            resampler,
            tick,
            start,
            file_start,
            rate,
            channels: channels as usize,
            eof: false,
            skip_to: None,
            frame: ff::frame::Audio::empty(),
            converted: Vec::new(),
        }))
    }

    /// Reads from `t` seconds into the file next.
    pub fn seek(&mut self, t: f64) -> Result<()> {
        let t = t.max(0.0);
        let ts = self.file_start + (t * ffi::AV_TIME_BASE as f64) as i64;
        self.input.seek(ts, ..ts)?;
        self.decoder.flush();
        unsafe {
            // What the resampler holds is from before the seek.
            ffi::swr_init(self.resampler.0);
        }
        self.eof = false;
        self.skip_to = Some(t);
        Ok(())
    }

    /// Appends the next samples to `out`. False at the end of the file.
    pub fn read(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        loop {
            if self.decoder.receive_frame(&mut self.frame).is_ok() {
                let pts = self.frame.timestamp().or(self.frame.pts()).map(|ts| (ts - self.start) as f64 * self.tick);
                self.convert()?;
                let mut from = 0;
                if let Some(t) = self.skip_to {
                    if let Some(pts) = pts {
                        let skip = ((t - pts) * self.rate as f64).round().max(0.0) as usize * self.channels;
                        from = skip.min(self.converted.len());
                    }
                    if from == self.converted.len() && pts.is_some() {
                        continue;
                    }
                    self.skip_to = None;
                }
                out.extend_from_slice(&self.converted[from..]);
                return Ok(true);
            }
            if self.eof {
                return Ok(false);
            }
            let mut packet = ff::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        // A broken packet is skipped, not the end of the sound.
                        let _ = self.decoder.send_packet(&packet);
                    }
                }
                Err(ff::Error::Eof) => {
                    let _ = self.decoder.send_eof();
                    self.eof = true;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// How loud the file is along its length: the loudest sample in each `1 / per_second` of it,
    /// from 0 to 1. None if it has no audio.
    pub fn peaks(path: &Path, per_second: u32) -> Result<Option<Vec<f32>>> {
        // A low rate is plenty to see the shape, and quick to read.
        let rate = 8_000;
        let Some(mut r) = Self::open(path, rate, 1)? else { return Ok(None) };
        let bucket = (rate / per_second.max(1)).max(1) as usize;
        let mut peaks = Vec::new();
        let mut samples = Vec::new();
        let (mut peak, mut n) = (0.0f32, 0);
        loop {
            samples.clear();
            let more = r.read(&mut samples)?;
            for s in &samples {
                peak = peak.max(s.abs());
                n += 1;
                if n == bucket {
                    peaks.push(peak.min(1.0));
                    (peak, n) = (0.0, 0);
                }
            }
            if !more {
                break;
            }
        }
        if n > 0 {
            peaks.push(peak.min(1.0));
        }
        Ok(Some(peaks))
    }

    fn convert(&mut self) -> Result<()> {
        unsafe {
            let f = self.frame.as_ptr();
            let in_count = (*f).nb_samples;
            let room = ffi::swr_get_out_samples(self.resampler.0, in_count).max(0) as usize + 32;
            self.converted.clear();
            self.converted.resize(room * self.channels, 0.0);
            let mut out_ptr = self.converted.as_mut_ptr() as *mut u8;
            let got = ffi::swr_convert(
                self.resampler.0,
                &mut out_ptr as *mut *mut u8 as *const *mut u8,
                room as i32,
                (*f).extended_data as *const *const u8,
                in_count,
            );
            if got < 0 {
                bail!("resampling failed");
            }
            self.converted.truncate(got as usize * self.channels);
        }
        Ok(())
    }
}
