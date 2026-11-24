//! Audio decoding through the ffmpeg CLI as f32 PCM on stdout. Video decodes in-process (video.rs);
//! audio stays on the CLI until it moves to libav too.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;

use anyhow::{Context, Result};

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

/// Kills the process when the reading thread stops, however it stops.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
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
