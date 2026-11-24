//! Reads a file's audio with `AudioReader`: the whole of it, then a second from a few seeks, and
//! prints how long that took and how loud each part is.
//!
//!   cargo run --release -p media --example audio_check -- FILE

use std::time::Instant;

use media::AudioReader;

fn rms(s: &[f32]) -> f32 {
    if s.is_empty() { 0.0 } else { (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt() }
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("pass a file");
    let (rate, channels) = (48_000u32, 2u16);
    let started = Instant::now();
    let Some(mut r) = AudioReader::open(path.as_ref(), rate, channels)? else {
        println!("no audio");
        return Ok(());
    };
    println!("opened in {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    let started = Instant::now();
    let mut all = Vec::new();
    while r.read(&mut all)? {}
    println!(
        "whole file: {:.3} s of sound in {:.1} ms, rms {:.3}",
        all.len() as f64 / (rate as f64 * channels as f64),
        started.elapsed().as_secs_f64() * 1000.0,
        rms(&all)
    );
    for t in [0.0, 2.0, 5.5, 1.25] {
        let started = Instant::now();
        r.seek(t)?;
        let mut got = Vec::new();
        while got.len() < (rate * channels as u32) as usize && r.read(&mut got)? {}
        got.truncate((rate * channels as u32) as usize);
        // The same second from the full read, to check the seek landed on the right sample.
        let from = (t * rate as f64).round() as usize * channels as usize;
        let want = &all[from.min(all.len())..(from + got.len()).min(all.len())];
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        // Past the decoder's warm-up, after 0.1 s: how far off in time, and how different.
        let skip = (rate as usize / 10) * channels as usize;
        let (mut best, mut best_lag) = (f32::MAX, 0i64);
        for lag in -2048i64..=2048 {
            let mut sum = 0.0f32;
            let mut n = 0;
            for i in (skip..got.len().min(skip + 9600)).step_by(channels as usize) {
                let j = i as i64 + lag * channels as i64;
                if j >= 0 && (j as usize) < want.len() {
                    sum += (got[i] - want[j as usize]).abs();
                    n += 1;
                }
            }
            let d = if n > 0 { sum / n as f32 } else { f32::MAX };
            if d < best {
                best = d;
                best_lag = lag;
            }
        }
        println!(
            "seek {t:5.2}: 1 s in {ms:5.1} ms, rms {:.3}, off by {best_lag} samples, mean difference then {best:.5}",
            rms(&got)
        );
    }
    Ok(())
}
