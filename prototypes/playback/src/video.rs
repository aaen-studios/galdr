//! Video decoding in-process with libav. One thread owns the open file and the decoder, so a seek
//! costs a keyframe lookup and the frames up to the target, not a new process.
//!
//! The thread takes commands and always works on the newest one: a burst of scrubs while it's busy
//! collapses to the last, and a scrub that arrives mid-decode stops the one in progress.

use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use ffmpeg_next as ff;
use ff::ffi;
use ff::format::Pixel;
use ff::software::scaling;

#[derive(Clone, Debug)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub has_audio: bool,
    pub codec: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hw {
    None,
    D3d11,
    Cuda,
}

impl Hw {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(Hw::None),
            "d3d11" | "d3d11va" => Ok(Hw::D3d11),
            "cuda" | "nvdec" => Ok(Hw::Cuda),
            _ => Err(anyhow!("--hw takes none, d3d11 or cuda")),
        }
    }
}

pub enum Command {
    /// Decode continuously from `from`.
    Play { generation: u64, from: f64 },
    /// One frame at `t`: the exact frame, or the keyframe at or before it, which is much quicker.
    Scrub { generation: u64, t: f64, exact: bool },
    Pause,
}

pub struct Frame {
    pub generation: u64,
    pub pts: f64,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub decoded_at: Instant,
}

pub struct VideoDecoder {
    commands: Sender<Command>,
    pub frames: Receiver<Frame>,
    /// What the decoder ended up using, such as "d3d11va" or "software, 32 threads".
    pub backend: String,
}

impl VideoDecoder {
    /// Opens `path` on a decoder thread, scaling frames to `out` (or the source size).
    pub fn open(path: &Path, height: Option<u32>, hw: Hw) -> Result<(Self, Info)> {
        ff::init()?;
        let (commands, command_rx) = mpsc::channel();
        let (frame_tx, frames) = mpsc::sync_channel(4);
        let (ready_tx, ready_rx) = mpsc::channel();
        let path = path.to_path_buf();
        thread::Builder::new().name("video-decoder".into()).spawn(move || {
            match State::open(&path, height, hw) {
                Ok(state) => {
                    let _ = ready_tx.send(Ok((state.info.clone(), state.backend.clone())));
                    if let Err(e) = state.run(command_rx, frame_tx) {
                        eprintln!("video decoder: {e:#}");
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })?;
        let (info, backend) = ready_rx.recv().context("decoder thread died")??;
        Ok((Self { commands, frames, backend }, info))
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}

struct State {
    input: ff::format::context::Input,
    stream: usize,
    decoder: ff::decoder::Video,
    /// Seconds per tick of the stream's time base, and the stream's first timestamp.
    tick: f64,
    start: i64,
    /// Start of the whole file in AV_TIME_BASE units, for seeking.
    file_start: i64,
    frame_time: f64,
    out_height: Option<u32>,
    scaler: Option<(Pixel, u32, u32, scaling::Context)>,
    eof: bool,
    /// Where the decoder is: the last frame it gave, after which it can decode forward.
    position: Option<f64>,
    info: Info,
    backend: String,
}

/// Decode forward instead of seeking when the target is this little ahead.
const FORWARD_WINDOW: f64 = 1.0;

/// Picks the hardware format matching the device, which `open` stores in the context's `opaque`.
unsafe extern "C" fn pick_hw_format(
    ctx: *mut ffi::AVCodecContext,
    formats: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    unsafe {
        let wanted = (*ctx).opaque as usize as i32;
        let mut p = formats;
        while *p != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
            if *p as i32 == wanted {
                return *p;
            }
            p = p.add(1);
        }
        *formats // the device can't decode this: software
    }
}

impl State {
    fn open(path: &PathBuf, out_height: Option<u32>, hw: Hw) -> Result<Self> {
        let input = ff::format::input(path).with_context(|| format!("opening {}", path.display()))?;
        let has_audio = input.streams().best(ff::media::Type::Audio).is_some();
        let stream = input.streams().best(ff::media::Type::Video).context("no video stream")?;
        let index = stream.index();
        let tb = stream.time_base();
        let tick = tb.numerator() as f64 / tb.denominator() as f64;
        let start = if stream.start_time() == ffi::AV_NOPTS_VALUE { 0 } else { stream.start_time() };
        let rate = stream.avg_frame_rate();
        let fps = if rate.denominator() > 0 && rate.numerator() > 0 {
            rate.numerator() as f64 / rate.denominator() as f64
        } else {
            30.0
        };
        let codec = format!("{:?}", stream.parameters().id()).to_lowercase();

        let mut ctx = ff::codec::context::Context::from_parameters(stream.parameters())?;
        let mut backend = String::from("software");
        let device = match hw {
            Hw::None => None,
            Hw::D3d11 => Some((
                ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA,
                ffi::AVPixelFormat::AV_PIX_FMT_D3D11,
                "d3d11va",
            )),
            Hw::Cuda => Some((ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA, ffi::AVPixelFormat::AV_PIX_FMT_CUDA, "cuda")),
        };
        let mut hw_ready = false;
        if let Some((kind, pix, name)) = device {
            unsafe {
                let mut device_ref: *mut ffi::AVBufferRef = ptr::null_mut();
                let r = ffi::av_hwdevice_ctx_create(&mut device_ref, kind, ptr::null(), ptr::null_mut(), 0);
                if r >= 0 {
                    // The codec context owns the reference from here and frees it.
                    (*ctx.as_mut_ptr()).hw_device_ctx = device_ref;
                    (*ctx.as_mut_ptr()).get_format = Some(pick_hw_format);
                    (*ctx.as_mut_ptr()).opaque = pix as i32 as usize as *mut std::ffi::c_void;
                    backend = name.to_string();
                    hw_ready = true;
                } else {
                    eprintln!("{name} unavailable ({}), using software", ff::Error::from(r));
                }
            }
        }
        if !hw_ready {
            ctx.set_threading(ff::codec::threading::Config {
                kind: ff::codec::threading::Type::Frame,
                count: 0,
                ..Default::default()
            });
        }
        let decoder = ctx.decoder().video()?;
        if !hw_ready {
            let threads = unsafe { (*decoder.as_ptr()).thread_count };
            backend = format!("software, {threads} threads");
        }

        let info = Info {
            width: decoder.width(),
            height: decoder.height(),
            fps,
            duration: input.duration().max(0) as f64 / ffi::AV_TIME_BASE as f64,
            has_audio,
            codec,
        };
        let file_start = unsafe {
            let s = (*input.as_ptr()).start_time;
            if s == ffi::AV_NOPTS_VALUE { 0 } else { s }
        };
        Ok(Self {
            input,
            stream: index,
            decoder,
            tick,
            start,
            file_start,
            frame_time: 1.0 / fps,
            out_height,
            scaler: None,
            eof: false,
            position: None,
            info,
            backend,
        })
    }

    fn out_size(&self, w: u32, h: u32) -> (u32, u32) {
        match self.out_height {
            Some(oh) if oh < h => (((w as f64 * oh as f64 / h as f64).round() as u32) & !1, oh),
            _ => (w, h),
        }
    }

    fn seek(&mut self, t: f64) -> Result<()> {
        let ts = self.file_start + (t.max(0.0) * ffi::AV_TIME_BASE as f64) as i64;
        self.input.seek(ts, ..ts)?;
        self.decoder.flush();
        self.eof = false;
        self.position = None;
        Ok(())
    }

    /// The next decoded frame, or none at the end of the file.
    fn next(&mut self) -> Result<Option<(ff::frame::Video, f64)>> {
        loop {
            let mut frame = ff::frame::Video::empty();
            if self.decoder.receive_frame(&mut frame).is_ok() {
                let ts = frame.timestamp().or(frame.pts()).unwrap_or(self.start);
                let pts = (ts - self.start) as f64 * self.tick;
                self.position = Some(pts);
                return Ok(Some((frame, pts)));
            }
            if self.eof {
                return Ok(None);
            }
            let mut packet = ff::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        self.decoder.send_packet(&packet)?;
                    }
                }
                Err(ff::Error::Eof) => {
                    self.decoder.send_eof()?;
                    self.eof = true;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn convert(&mut self, frame: &ff::frame::Video, generation: u64, pts: f64) -> Result<Frame> {
        // Hardware frames live on the GPU; copy them down first.
        let raw_format = unsafe { (*frame.as_ptr()).format };
        let is_hw = raw_format == ffi::AVPixelFormat::AV_PIX_FMT_D3D11 as i32
            || raw_format == ffi::AVPixelFormat::AV_PIX_FMT_CUDA as i32;
        let downloaded;
        let src = if is_hw {
            let mut sw = ff::frame::Video::empty();
            let r = unsafe { ffi::av_hwframe_transfer_data(sw.as_mut_ptr(), frame.as_ptr(), 0) };
            if r < 0 {
                return Err(anyhow!("downloading a hardware frame: {}", ff::Error::from(r)));
            }
            downloaded = sw;
            &downloaded
        } else {
            frame
        };

        let (w, h, format) = (src.width(), src.height(), src.format());
        let (ow, oh) = self.out_size(w, h);
        if !matches!(&self.scaler, Some((f, sw, sh, _)) if *f == format && *sw == w && *sh == h) {
            let ctx = scaling::Context::get(format, w, h, Pixel::BGRA, ow, oh, scaling::Flags::BILINEAR)?;
            self.scaler = Some((format, w, h, ctx));
        }
        let scaler = &mut self.scaler.as_mut().unwrap().3;
        let mut out = ff::frame::Video::empty();
        scaler.run(src, &mut out)?;

        let row = ow as usize * 4;
        let stride = out.stride(0);
        let data = out.data(0);
        let bgra = if stride == row {
            data[..row * oh as usize].to_vec()
        } else {
            let mut v = Vec::with_capacity(row * oh as usize);
            for y in 0..oh as usize {
                v.extend_from_slice(&data[y * stride..y * stride + row]);
            }
            v
        };
        Ok(Frame { generation, pts, width: ow, height: oh, bgra, decoded_at: Instant::now() })
    }

    fn run(mut self, commands: Receiver<Command>, frames: SyncSender<Frame>) -> Result<()> {
        // What's being played, and frames before `from` to skip after the seek.
        let mut playing: Option<(u64, f64)> = None;
        let mut next_command: Option<Command> = None;
        loop {
            let command = match next_command.take() {
                Some(c) => Some(c),
                None if playing.is_some() => match commands.try_recv() {
                    Ok(c) => Some(c),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                },
                None => match commands.recv_timeout(Duration::from_secs(3600)) {
                    Ok(c) => Some(c),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                },
            };
            if let Some(mut command) = command {
                // Only the newest command matters.
                while let Ok(newer) = commands.try_recv() {
                    command = newer;
                }
                match command {
                    Command::Pause => playing = None,
                    Command::Play { generation, from } => {
                        if !self.position.is_some_and(|p| (p - from).abs() < self.frame_time * 0.5) {
                            self.seek(from)?;
                        }
                        playing = Some((generation, from));
                    }
                    Command::Scrub { generation, t, exact } => {
                        playing = None;
                        match self.scrub(generation, t, exact, &commands)? {
                            Scrubbed::Frame(f, queued) => {
                                if frames.send(f).is_err() {
                                    return Ok(());
                                }
                                next_command = queued;
                            }
                            Scrubbed::Interrupted(c) => next_command = Some(c),
                            Scrubbed::Nothing => {}
                        }
                    }
                }
                continue;
            }

            let Some((generation, from)) = playing else { continue };
            let Some((frame, pts)) = self.next()? else {
                playing = None; // end of file
                continue;
            };
            if pts + self.frame_time * 0.5 < from {
                continue;
            }
            let mut out = self.convert(&frame, generation, pts)?;
            // Wait for room, but keep listening: a seek shouldn't queue behind a full channel.
            loop {
                match frames.try_send(out) {
                    Ok(()) => break,
                    Err(TrySendError::Full(f)) => {
                        out = f;
                        match commands.try_recv() {
                            Ok(c) => {
                                next_command = Some(c);
                                break;
                            }
                            Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(2)),
                            Err(TryRecvError::Disconnected) => return Ok(()),
                        }
                    }
                    Err(TrySendError::Disconnected(_)) => return Ok(()),
                }
            }
        }
    }

    fn scrub(&mut self, generation: u64, t: f64, exact: bool, commands: &Receiver<Command>) -> Result<Scrubbed> {
        // A little ahead: decoding forward beats seeking back to a keyframe, so give the exact
        // frame even when a keyframe would do.
        let ahead = self.position.map(|p| t - p);
        let forward = ahead.is_some_and(|d| d > 0.0 && d < FORWARD_WINDOW);
        if !forward {
            self.seek(t)?;
        }
        let want_exact = exact || forward;
        // Any newer command cancels this one. During a drag that loses nothing: the decoder keeps
        // its position, and the next target is usually just ahead of it. (Letting a scrub finish
        // first was measured slower: 124 instead of 205 updates a second at 1080p.)
        let queued = None;
        let mut last = None;
        loop {
            if let Ok(c) = commands.try_recv() {
                return Ok(Scrubbed::Interrupted(c));
            }
            let Some((frame, pts)) = self.next()? else { break };
            if !want_exact || pts + self.frame_time * 0.5 >= t {
                return Ok(Scrubbed::Frame(self.convert(&frame, generation, pts)?, queued));
            }
            last = Some((frame, pts));
        }
        // Past the end: the last frame there is.
        match last {
            Some((frame, pts)) => Ok(Scrubbed::Frame(self.convert(&frame, generation, pts)?, queued)),
            None => Ok(queued.map_or(Scrubbed::Nothing, Scrubbed::Interrupted)),
        }
    }
}

enum Scrubbed {
    /// The frame, and a newer scrub that arrived meanwhile.
    Frame(Frame, Option<Command>),
    Interrupted(Command),
    Nothing,
}
