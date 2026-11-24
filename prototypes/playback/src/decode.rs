//! Decoding through the ffmpeg CLI: video as raw BGRA frames, audio as f32 PCM, both on stdout.
//!
//! This is the prototype's stand-in for libav. Seeking restarts the process at the new time.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;
use std::time::Instant;

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub has_audio: bool,
    pub codec: String,
}

pub struct Frame {
    pub pts: f64,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub decoded_at: Instant,
}

fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn probe(path: &Path) -> Result<Info> {
    let out = command("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"])
        .arg(path)
        .output()
        .context("running ffprobe (is it on PATH?)")?;
    if !out.status.success() {
        bail!("ffprobe failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let streams = json["streams"].as_array().context("no streams")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .context("no video stream")?;
    let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");

    let fps = video["avg_frame_rate"]
        .as_str()
        .or(video["r_frame_rate"].as_str())
        .and_then(|r| {
            let (n, d) = r.split_once('/')?;
            let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
            (d > 0.0 && n > 0.0).then_some(n / d)
        })
        .unwrap_or(30.0);
    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse().ok())
        .unwrap_or(0.0);

    Ok(Info {
        width: video["width"].as_u64().context("no width")? as u32,
        height: video["height"].as_u64().context("no height")? as u32,
        fps,
        duration,
        has_audio,
        codec: video["codec_name"].as_str().unwrap_or("?").to_string(),
    })
}

/// Kills the process when the reading thread stops, however it stops.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub struct VideoRequest {
    pub path: PathBuf,
    pub info: Info,
    pub start: f64,
    /// Output size; the source size when it matches.
    pub width: u32,
    pub height: u32,
    /// Decode one frame and stop, for scrubbing.
    pub single: bool,
    pub hwaccel: bool,
}

/// Starts decoding video at `start`. Dropping the receiver stops the decoder.
pub fn start_video(req: VideoRequest) -> Receiver<Frame> {
    let (tx, rx) = sync_channel::<Frame>(if req.single { 1 } else { 6 });
    thread::spawn(move || {
        if let Err(e) = run_video(&req, &tx) {
            eprintln!("video decoder: {e:#}");
        }
    });
    rx
}

fn run_video(req: &VideoRequest, tx: &SyncSender<Frame>) -> Result<()> {
    let mut cmd = command("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    if req.hwaccel {
        cmd.args(["-hwaccel", "auto"]);
    }
    cmd.args(["-ss", &format!("{:.3}", req.start)]).arg("-i").arg(&req.path);
    cmd.args(["-map", "0:v:0", "-an", "-sn"]);
    if req.width != req.info.width || req.height != req.info.height {
        cmd.args(["-vf", &format!("scale={}:{}:flags=bilinear", req.width, req.height)]);
    }
    cmd.args(["-pix_fmt", "bgra", "-f", "rawvideo"]);
    if req.single {
        cmd.args(["-frames:v", "1"]);
    }
    cmd.arg("pipe:1").stdout(Stdio::piped()).stderr(Stdio::inherit());

    let mut child = KillOnDrop(cmd.spawn().context("running ffmpeg (is it on PATH?)")?);
    let mut stdout = child.0.stdout.take().context("no stdout")?;
    let size = (req.width * req.height * 4) as usize;
    let mut n: u64 = 0;
    loop {
        let mut buf = vec![0u8; size];
        if stdout.read_exact(&mut buf).is_err() {
            return Ok(()); // end of file, or killed
        }
        let frame = Frame {
            pts: req.start + n as f64 / req.info.fps,
            width: req.width,
            height: req.height,
            bgra: buf,
            decoded_at: Instant::now(),
        };
        n += 1;
        if tx.send(frame).is_err() {
            return Ok(()); // the player moved on
        }
    }
}

/// Starts decoding audio at `start` as interleaved f32 at the device's rate and channel count,
/// calling `push` with each chunk until it returns false.
pub fn start_audio(
    path: PathBuf,
    start: f64,
    rate: u32,
    channels: u16,
    mut push: impl FnMut(&[f32]) -> bool + Send + 'static,
) {
    thread::spawn(move || {
        let mut run = || -> Result<()> {
            let mut cmd = command("ffmpeg");
            cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"])
                .args(["-ss", &format!("{start:.3}")])
                .arg("-i")
                .arg(&path)
                .args(["-map", "0:a:0", "-vn", "-sn", "-f", "f32le"])
                .args(["-ac", &channels.to_string(), "-ar", &rate.to_string()])
                .arg("pipe:1")
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit());
            let mut child = KillOnDrop(cmd.spawn()?);
            let mut stdout = child.0.stdout.take().context("no stdout")?;
            let mut bytes = vec![0u8; 4096 * 4];
            let mut samples = Vec::with_capacity(4096);
            loop {
                let got = stdout.read(&mut bytes)?;
                if got == 0 {
                    return Ok(());
                }
                // A read can end mid-sample; keep whole samples only.
                let whole = got - got % 4;
                samples.clear();
                samples.extend(
                    bytes[..whole]
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                );
                if !push(&samples) {
                    return Ok(());
                }
                if whole != got {
                    // Rare with pipes; drop the partial sample rather than drift.
                    let _ = stdout.read_exact(&mut bytes[..4 - (got - whole)]);
                }
            }
        };
        if let Err(e) = run() {
            eprintln!("audio decoder: {e:#}");
        }
    });
}
