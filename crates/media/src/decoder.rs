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

#[derive(Clone)]
pub struct Frame {
    pub generation: u64,
    pub pts: f64,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub decoded_at: Instant,
    pub cost: Cost,
    /// The frame that was asked for, not one shown on the way to it.
    pub exact: bool,
}

/// What a frame cost the decoder: frames decoded to reach it, and time spent decoding and
/// converting them.
#[derive(Clone, Copy, Default, Debug)]
pub struct Cost {
    pub decoded: u32,
    pub decode_ms: f32,
    pub convert_ms: f32,
}

/// YUV to BGRA, and to the preview size, on as many threads as the machine has. ffmpeg-next's
/// scaler uses one.
pub(crate) struct Scaler(*mut ffi::SwsContext);

// The context is only ever used by the thread that owns the `State` holding it.
unsafe impl Send for Scaler {}

impl Scaler {
    /// `threads` 0 is one per core. Bicubic: the preview is the source scaled to fit, and must
    /// look it.
    pub(crate) fn new(threads: usize) -> Result<Self> {
        let ctx = unsafe { ffi::sws_alloc_context() };
        if ctx.is_null() {
            return Err(anyhow!("can't make a scaler"));
        }
        unsafe {
            (*ctx).threads = threads as i32;
            (*ctx).flags = ffi::SwsFlags::SWS_BICUBIC as u32;
        }
        Ok(Self(ctx))
    }

    fn run(&mut self, src: &ff::frame::Video, w: u32, h: u32) -> Result<ff::frame::Video> {
        let mut dst = ff::frame::Video::empty();
        unsafe {
            let d = dst.as_mut_ptr();
            (*d).width = w as i32;
            (*d).height = h as i32;
            (*d).format = ffi::AVPixelFormat::AV_PIX_FMT_BGRA as i32;
            let r = ffi::sws_scale_frame(self.0, d, src.as_ptr());
            if r < 0 {
                return Err(anyhow!("converting a frame: {}", ff::Error::from(r)));
            }
        }
        Ok(dst)
    }
}

impl Drop for Scaler {
    fn drop(&mut self) {
        unsafe { ffi::sws_free_context(&mut self.0) };
    }
}

pub struct VideoDecoder {
    commands: Sender<Command>,
    pub frames: Receiver<Frame>,
    /// What the decoder ended up using, such as "d3d11va" or "software, 32 threads".
    pub backend: String,
}

impl VideoDecoder {
    /// Opens `path` on a decoder thread without waiting for it. Commands sent meanwhile wait for
    /// the file to open; if it can't, the frame channel closes.
    pub fn start(path: &Path, height: Option<u32>, hw: Hw) -> Self {
        let _ = ff::init();
        let (commands, command_rx) = mpsc::channel();
        let (frame_tx, frames) = mpsc::sync_channel(4);
        let path = path.to_path_buf();
        let _ = thread::Builder::new().name("video-decoder".into()).spawn(move || {
            match State::open(&path, height, hw, 0) {
                Ok(state) => {
                    if let Err(e) = state.run(command_rx, frame_tx) {
                        eprintln!("video decoder: {e:#}");
                    }
                }
                Err(e) => eprintln!("video decoder: {e:#}"),
            }
        });
        Self { commands, frames, backend: String::new() }
    }

    /// Opens `path` on a decoder thread, scaling frames to `out` (or the source size).
    pub fn open(path: &Path, height: Option<u32>, hw: Hw) -> Result<(Self, Info)> {
        ff::init()?;
        let (commands, command_rx) = mpsc::channel();
        let (frame_tx, frames) = mpsc::sync_channel(4);
        let (ready_tx, ready_rx) = mpsc::channel();
        let path = path.to_path_buf();
        thread::Builder::new().name("video-decoder".into()).spawn(move || {
            match State::open(&path, height, hw, 0) {
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

pub(crate) struct State {
    input: ff::format::context::Input,
    stream: usize,
    decoder: ff::decoder::Video,
    /// Seconds per tick of the stream's time base, and the stream's first timestamp.
    tick: f64,
    start: i64,
    /// Start of the whole file in AV_TIME_BASE units, for seeking.
    file_start: i64,
    pub(crate) frame_time: f64,
    pub(crate) out_height: Option<u32>,
    scaler: Scaler,
    /// Keyframe times in seconds, from the file's index; empty when it has none.
    pub(crate) keyframes: Vec<f64>,
    /// Spent since the last frame handed out.
    cost: Cost,
    last_delivery: Option<Instant>,
    eof: bool,
    /// Where the decoder is: the last frame it gave, after which it can decode forward.
    pub(crate) position: Option<f64>,
    /// Frames shown before this time that no other frame needs are skipped: set while decoding
    /// toward a frame, so as not to decode ones that are passed over.
    pub(crate) skip_before: Option<f64>,
    /// Decoding keyframes only.
    stills: bool,
    pub(crate) info: Info,
    backend: String,
}

/// Without a keyframe index: decode forward instead of seeking when the target is this little ahead.
const FORWARD_WINDOW: f64 = 1.0;

/// Keyframe times from the stream's index, which mp4 and mov carry whole and mkv as cues.
fn keyframe_index(stream: &ff::format::stream::Stream, start: i64, tick: f64) -> Vec<f64> {
    let ptr = unsafe { stream.as_ptr() } as *mut ffi::AVStream;
    let count = unsafe { ffi::avformat_index_get_entries_count(ptr) };
    let mut out = Vec::new();
    for i in 0..count {
        let entry = unsafe { ffi::avformat_index_get_entry(ptr, i) };
        if entry.is_null() {
            continue;
        }
        let entry = unsafe { &*entry };
        if entry.flags() & ffi::AVINDEX_KEYFRAME != 0 {
            out.push((entry.timestamp - start) as f64 * tick);
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// Picks the hardware format matching the device, which `open` stores in the context's `opaque`.
/// A reference to the one device of a kind that every decoder shares, made the first time it's
/// asked for: making one per decoder costs time and GPU memory. None if the machine has none.
fn hw_device(kind: ffi::AVHWDeviceType) -> Option<*mut ffi::AVBufferRef> {
    // The kind, and the device's own reference as an address, kept for as long as galdr runs;
    // 0 when making it failed, so it isn't tried again.
    static DEVICES: std::sync::Mutex<Vec<(i32, usize)>> = std::sync::Mutex::new(Vec::new());
    let mut devices = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
    let master = match devices.iter().find(|(k, _)| *k == kind as i32) {
        Some(&(_, p)) => p,
        None => {
            let mut r: *mut ffi::AVBufferRef = ptr::null_mut();
            let ok = unsafe { ffi::av_hwdevice_ctx_create(&mut r, kind, ptr::null(), ptr::null_mut(), 0) } >= 0;
            let p = if ok { r as usize } else { 0 };
            devices.push((kind as i32, p));
            p
        }
    };
    (master != 0).then(|| unsafe { ffi::av_buffer_ref(master as *mut ffi::AVBufferRef) }).filter(|r| !r.is_null())
}

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
    pub(crate) fn open(path: &PathBuf, out_height: Option<u32>, hw: Hw, threads: usize) -> Result<Self> {
        Self::open_as(path, out_height, hw, threads, false)
    }

    /// Opened to give keyframes only, for thumbnails and filmstrips: each one is a frame decoded,
    /// with nothing decoded ahead of it, on slice threads.
    pub(crate) fn open_stills(path: &PathBuf, out_height: Option<u32>) -> Result<Self> {
        Self::open_as(path, out_height, Hw::None, 4, true)
    }

    fn open_as(path: &PathBuf, out_height: Option<u32>, hw: Hw, threads: usize, stills: bool) -> Result<Self> {
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
        let keyframes = keyframe_index(&stream, start, tick);

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
            match hw_device(kind) {
                Some(device_ref) => unsafe {
                    // The codec context owns this reference from here and frees it.
                    (*ctx.as_mut_ptr()).hw_device_ctx = device_ref;
                    (*ctx.as_mut_ptr()).get_format = Some(pick_hw_format);
                    (*ctx.as_mut_ptr()).opaque = pix as i32 as usize as *mut std::ffi::c_void;
                    backend = name.to_string();
                    hw_ready = true;
                },
                None => eprintln!("{name} unavailable, using software"),
            }
        }
        if !hw_ready {
            ctx.set_threading(ff::codec::threading::Config {
                kind: if stills { ff::codec::threading::Type::Slice } else { ff::codec::threading::Type::Frame },
                count: threads,
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
            scaler: Scaler::new(threads)?,
            keyframes,
            cost: Cost::default(),
            last_delivery: None,
            eof: false,
            position: None,
            skip_before: None,
            stills,
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

    fn keyframe_at_or_before(&self, t: f64) -> Option<f64> {
        if self.keyframes.is_empty() {
            return None;
        }
        let i = self.keyframes.partition_point(|&k| k <= t + self.frame_time * 0.5);
        Some(if i == 0 { self.keyframes[0] } else { self.keyframes[i - 1] })
    }

    pub(crate) fn seek(&mut self, t: f64) -> Result<()> {
        let ts = self.file_start + (t.max(0.0) * ffi::AV_TIME_BASE as f64) as i64;
        self.input.seek(ts, ..ts)?;
        self.decoder.flush();
        self.eof = false;
        self.position = None;
        Ok(())
    }

    /// The next decoded frame, or none at the end of the file.
    pub(crate) fn next(&mut self) -> Result<Option<(ff::frame::Video, f64)>> {
        let started = Instant::now();
        let result = self.next_inner();
        self.cost.decode_ms += started.elapsed().as_secs_f32() * 1000.0;
        if matches!(result, Ok(Some(_))) {
            self.cost.decoded += 1;
        }
        result
    }

    fn next_inner(&mut self) -> Result<Option<(ff::frame::Video, f64)>> {
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
                        // No frame refers to a non-reference frame (most B-frames), so the ones
                        // shown before the frame wanted can go undecoded on the way to it.
                        let before = match (self.skip_before, packet.pts()) {
                            (Some(t), Some(pts)) => ((pts - self.start) as f64 * self.tick) < t,
                            _ => false,
                        };
                        unsafe {
                            (*self.decoder.as_mut_ptr()).skip_frame = if self.stills {
                                ffi::AVDiscard::AVDISCARD_NONKEY
                            } else if before {
                                ffi::AVDiscard::AVDISCARD_NONREF
                            } else {
                                ffi::AVDiscard::AVDISCARD_DEFAULT
                            };
                        }
                        self.decoder.send_packet(&packet)?;
                        // A still is the keyframe alone: end the input there, so the decoder gives
                        // it now rather than holding it until the next keyframe comes. The next
                        // seek starts it again.
                        if self.stills && packet.is_key() {
                            self.decoder.send_eof()?;
                            self.eof = true;
                        }
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

    pub(crate) fn convert(&mut self, frame: &ff::frame::Video, generation: u64, pts: f64) -> Result<Frame> {
        let started = Instant::now();
        let mut out = self.convert_inner(frame, generation, pts)?;
        self.cost.convert_ms += started.elapsed().as_secs_f32() * 1000.0;
        out.cost = std::mem::take(&mut self.cost);
        Ok(out)
    }

    fn convert_inner(&mut self, frame: &ff::frame::Video, generation: u64, pts: f64) -> Result<Frame> {
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

        let (ow, oh) = self.out_size(src.width(), src.height());
        let out = self.scaler.run(src, ow, oh)?;

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
        Ok(Frame {
            generation,
            pts,
            width: ow,
            height: oh,
            bgra,
            decoded_at: Instant::now(),
            cost: Cost::default(),
            exact: false,
        })
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
                        match self.scrub(generation, t, exact, &commands, &frames)? {
                            Scrubbed::Frame(f) => {
                                if frames.send(f).is_err() {
                                    return Ok(());
                                }
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
            out.exact = true;
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

    /// Decodes toward `t`, showing frames on the way at most every `PROGRESS_EVERY`, so a jump
    /// shows its keyframe at once and a drag that outruns the decoder still moves.
    fn scrub(
        &mut self,
        generation: u64,
        t: f64,
        exact: bool,
        commands: &Receiver<Command>,
        frames: &SyncSender<Frame>,
    ) -> Result<Scrubbed> {
        // Decode forward from where the decoder is when no keyframe lies between it and the
        // target: seeking would land on the same or an earlier keyframe and decode more. Then the
        // exact frame costs no more than a keyframe would.
        let forward = match self.position {
            Some(p) if t > p => match self.keyframe_at_or_before(t) {
                Some(k) => k <= p + self.frame_time * 0.5,
                None => t - p < FORWARD_WINDOW,
            },
            _ => false,
        };
        if !forward {
            self.seek(t)?;
        }
        let want_exact = exact || forward;
        // Any newer command cancels this one. During a drag that loses nothing: the decoder keeps
        // its position, and the next target is usually just ahead of it. (Letting a scrub finish
        // first was measured slower: 124 instead of 205 updates a second at 1080p.)
        let mut last = None;
        loop {
            if let Ok(c) = commands.try_recv() {
                return Ok(Scrubbed::Interrupted(c));
            }
            let Some((frame, pts)) = self.next()? else { break };
            if !want_exact || pts + self.frame_time * 0.5 >= t {
                let mut out = self.convert(&frame, generation, pts)?;
                out.exact = true;
                self.last_delivery = Some(Instant::now());
                return Ok(Scrubbed::Frame(out));
            }
            if self.last_delivery.is_none_or(|d| d.elapsed() >= PROGRESS_EVERY) {
                let out = self.convert(&frame, generation, pts)?;
                self.last_delivery = Some(Instant::now());
                if frames.send(out).is_err() {
                    return Ok(Scrubbed::Nothing);
                }
            } else {
                last = Some((frame, pts));
            }
        }
        // Past the end: the last frame there is.
        match last {
            Some((frame, pts)) => {
                let mut out = self.convert(&frame, generation, pts)?;
                out.exact = true;
                Ok(Scrubbed::Frame(out))
            }
            None => Ok(Scrubbed::Nothing),
        }
    }
}

/// While decoding toward a target, show a frame on the way at least this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(33);

enum Scrubbed {
    Frame(Frame),
    Interrupted(Command),
    Nothing,
}

/// One frame at about `at` seconds (the keyframe there), `height` pixels high, with what the file
/// is. For the media bin; runs on the caller's thread.
pub fn thumbnail(path: &Path, at: f64, height: u32) -> Result<(Info, Frame)> {
    ff::init()?;
    let mut state = State::open_stills(&path.to_path_buf(), Some(height))?;
    let t = at.min((state.info.duration - 0.1).max(0.0));
    state.seek(t)?;
    let (frame, pts) = state.next()?.context("no frames")?;
    let mut out = state.convert(&frame, 0, pts)?;
    out.exact = true;
    Ok((state.info.clone(), out))
}

impl State {
    /// When the first frame is shown, from the first packet, without decoding anything. Leaves
    /// the file read past it; seek before decoding.
    pub(crate) fn first_pts(&mut self) -> Result<Option<f64>> {
        loop {
            let mut packet = ff::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.stream => {
                    return Ok(packet.pts().map(|ts| (ts - self.start) as f64 * self.tick));
                }
                Ok(()) => {}
                Err(ff::Error::Eof) => return Ok(None),
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Changes the height frames are converted to, with a fresh scaler for the new size.
    pub(crate) fn set_out_height(&mut self, height: Option<u32>, threads: usize) -> Result<()> {
        if self.out_height != height {
            self.out_height = height;
            self.scaler = Scaler::new(threads)?;
        }
        Ok(())
    }
}

/// Frames at about each of `times` (the keyframe before each), `height` pixels high. For the
/// timeline's filmstrips. A keyframe can only be decoded on one thread at a time, so the times
/// are split between several decoders, each with its own part.
pub fn filmstrip(path: &Path, times: &[f64], height: u32) -> Result<Vec<Frame>> {
    let decoders = thread::available_parallelism().map_or(2, |n| n.get() / 4).clamp(1, 8).min(times.len().max(1));
    let part = times.len().div_ceil(decoders).max(1);
    let parts: Vec<Result<Vec<Frame>>> = thread::scope(|s| {
        let running: Vec<_> = times.chunks(part).map(|chunk| s.spawn(move || filmstrip_part(path, chunk, height))).collect();
        running.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(anyhow!("a filmstrip decoder panicked")))).collect()
    });
    let mut out = Vec::with_capacity(times.len());
    for p in parts {
        out.extend(p?);
    }
    Ok(out)
}

fn filmstrip_part(path: &Path, times: &[f64], height: u32) -> Result<Vec<Frame>> {
    ff::init()?;
    let mut state = State::open_stills(&path.to_path_buf(), Some(height))?;
    let mut out: Vec<Frame> = Vec::with_capacity(times.len());
    // The keyframe each time falls after, and the frame decoded there: times in a long group of
    // pictures share its keyframe, which is decoded once.
    let mut last: Option<(f64, Frame)> = None;
    for &t in times {
        let t = t.min((state.info.duration - 0.1).max(0.0));
        let key = state.keyframe_at_or_before(t);
        if let (Some(k), Some((lk, f))) = (key, &last)
            && k == *lk
        {
            out.push(f.clone());
            continue;
        }
        state.seek(t)?;
        let Some((frame, pts)) = state.next()? else { break };
        let f = state.convert(&frame, 0, pts)?;
        if let Some(k) = key {
            last = Some((k, f.clone()));
        }
        out.push(f);
    }
    Ok(out)
}

/// What a video file is: its size, rate, length and codec.
pub fn probe(path: &Path) -> Result<Info> {
    ff::init()?;
    Ok(State::open(&path.to_path_buf(), None, Hw::None, 1)?.info)
}

/// For measuring: decodes `count` frames from `from` seconds in, and gives the milliseconds spent
/// decoding, copying hardware frames down, and scaling to `height`, in all, and what decoded.
pub fn speed_test(path: &Path, hw: Hw, threads: usize, height: u32, from: f64, count: usize) -> Result<(f64, f64, f64, usize, String)> {
    ff::init()?;
    let mut state = State::open(&path.to_path_buf(), Some(height), hw, threads)?;
    state.seek(from)?;
    let (mut decode, mut download, mut scale, mut n) = (0.0, 0.0, 0.0, 0);
    while n < count {
        let t0 = Instant::now();
        let Some((frame, _)) = state.next()? else { break };
        let t1 = Instant::now();
        let raw_format = unsafe { (*frame.as_ptr()).format };
        let is_hw = raw_format == ffi::AVPixelFormat::AV_PIX_FMT_D3D11 as i32
            || raw_format == ffi::AVPixelFormat::AV_PIX_FMT_CUDA as i32;
        let mut sw = ff::frame::Video::empty();
        let src = if is_hw {
            let r = unsafe { ffi::av_hwframe_transfer_data(sw.as_mut_ptr(), frame.as_ptr(), 0) };
            if r < 0 {
                return Err(anyhow!("downloading a hardware frame: {}", ff::Error::from(r)));
            }
            &sw
        } else {
            &frame
        };
        let t2 = Instant::now();
        let (ow, oh) = state.out_size(src.width(), src.height());
        state.scaler.run(src, ow, oh)?;
        let t3 = Instant::now();
        decode += (t1 - t0).as_secs_f64() * 1000.0;
        download += (t2 - t1).as_secs_f64() * 1000.0;
        scale += (t3 - t2).as_secs_f64() * 1000.0;
        n += 1;
    }
    Ok((decode, download, scale, n, state.backend.clone()))
}

/// For measuring a jump: seeks to the keyframe before `to` seconds, decodes up to the frame there
/// without converting any on the way, skipping passed-over frames if `skip`, and converts that
/// one. Gives the milliseconds in all, and a hash of the frame, to check skipping changed nothing.
pub fn jump_test(path: &Path, hw: Hw, threads: usize, height: u32, to: f64, skip: bool) -> Result<(f64, u64)> {
    use std::hash::{Hash, Hasher};
    ff::init()?;
    let mut state = State::open(&path.to_path_buf(), Some(height), hw, threads)?;
    // Warm the decoder up somewhere else first, as an open one would be.
    state.seek(0.0)?;
    state.next()?;
    let started = Instant::now();
    let key = state.keyframe_at_or_before(to).unwrap_or(0.0);
    state.seek(key + state.frame_time * 0.5)?;
    state.skip_before = skip.then_some(to - state.frame_time * 0.5);
    let mut hash = 0;
    loop {
        let Some((frame, pts)) = state.next()? else { break };
        if pts >= to - state.frame_time * 0.5 {
            let f = state.convert(&frame, 0, pts)?;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            f.bgra.hash(&mut h);
            hash = h.finish();
            break;
        }
    }
    Ok((started.elapsed().as_secs_f64() * 1000.0, hash))
}
