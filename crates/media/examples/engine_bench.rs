//! Measures the frame engine without a window: random jumps, a fast drag, and playback.
//!
//!   cargo run --release -p media --example engine_bench -- FILE [--workers N] [--threads N] [--viewer WxH] [--hw none|d3d11|cuda]

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use media::{Engine, Settings, Want};

fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * p).round() as usize]
}

fn avg(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut settings = Settings::default();
    let mut file = None;
    let mut viewer = (1600u32, 900u32);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--workers" => settings.workers = args.next().unwrap().parse()?,
            "--threads" => settings.threads = args.next().unwrap().parse()?,
            "--hw" => settings.hw = media::Hw::parse(&args.next().unwrap())?,
            "--viewer" => {
                let v = args.next().unwrap();
                let (w, h) = v.split_once('x').unwrap();
                viewer = (w.parse()?, h.parse()?);
            }
            _ => file = Some(a),
        }
    }
    let file = file.expect("pass a video file");
    let engine: Engine<Arc<Vec<u8>>> = Engine::new(settings, |_, _, bgra| Arc::new(bgra));
    engine.set_viewer(viewer.0, viewer.1);
    let opened = Instant::now();
    let info = engine.add_source(1, file.as_ref())?;
    println!(
        "{}x{} {:.0} fps {:.1}s {}, opened in {:.0} ms; {} workers x {} threads, viewer {}x{}",
        info.width,
        info.height,
        info.fps,
        info.duration,
        info.codec,
        opened.elapsed().as_secs_f64() * 1000.0,
        settings.workers,
        settings.threads,
        viewer.0,
        viewer.1
    );
    let ft = 1.0 / info.fps;
    let wait = |t: f64, limit: f64| -> Option<f64> {
        let start = Instant::now();
        while start.elapsed().as_secs_f64() < limit {
            if engine.get(1, t).is_some() {
                return Some(start.elapsed().as_secs_f64() * 1000.0);
            }
            thread::sleep(Duration::from_micros(200));
        }
        None
    };

    // Random jumps, each given 300 ms to settle and prefetch around, as when clicking about.
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut random = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut jumps = Vec::new();
    for _ in 0..20 {
        let t = random() * (info.duration - 1.0);
        engine.want(vec![
            Want { source: 1, from: t, to: t },
            Want { source: 1, from: t, to: t + 1.0 },
            Want { source: 1, from: t - 1.0, to: t },
        ]);
        if let Some(ms) = wait(t, 5.0) {
            jumps.push(ms);
        }
        thread::sleep(Duration::from_millis(300));
    }
    println!(
        "20 jumps: exact frame avg {:.1} p50 {:.1} p95 {:.1} max {:.1} ms",
        avg(&jumps),
        pct(&jumps, 0.5),
        pct(&jumps, 0.95),
        pct(&jumps, 1.0)
    );

    // Drags at a few speeds, the mouse reporting at 120 Hz: how often the exact frame under the
    // playhead is ready, and how far behind what can be shown is.
    for speed in [0.5, 2.0, 8.0] {
        let (from, length) = (2.0, 3.0);
        let mut hits = 0;
        let mut samples = 0;
        let mut behind = Vec::new();
        let start = Instant::now();
        loop {
            let elapsed = start.elapsed().as_secs_f64();
            let t = from + (elapsed * speed) % (info.duration - from - 1.0);
            if elapsed > length {
                break;
            }
            engine.want(vec![
                Want { source: 1, from: t, to: t },
                Want { source: 1, from: t, to: t + 0.5 * speed.max(1.0) },
                Want { source: 1, from: t - 0.5, to: t },
            ]);
            samples += 1;
            if engine.get(1, t).is_some() {
                hits += 1;
                behind.push(0.0);
            } else if let Some((_, at)) = engine.nearest(1, t, t - 2.0, t + 2.0) {
                behind.push(((t - at) / ft).abs());
            }
            thread::sleep(Duration::from_millis(8));
        }
        println!(
            "drag at {speed}x for {length}s: exact under the playhead {:.0}% of {samples}, shown frame off by avg {:.1} p95 {:.1} frames",
            100.0 * hits as f64 / samples as f64,
            avg(&behind),
            pct(&behind, 0.95)
        );
    }

    // Playback from a cold spot: frames due each tick, ready or not.
    let t0 = info.duration * 0.6;
    engine.want(vec![Want { source: 1, from: t0, to: t0 + 2.0 }]);
    let first = wait(t0, 5.0).unwrap_or(f64::NAN);
    let start = Instant::now();
    let (mut due, mut late) = (0, 0);
    let length = 6.0f64.min(info.duration - t0 - 0.5);
    loop {
        let t = t0 + start.elapsed().as_secs_f64();
        if t > t0 + length {
            break;
        }
        engine.want(vec![Want { source: 1, from: t, to: t }, Want { source: 1, from: t, to: t + 2.0 }]);
        due += 1;
        if engine.get(1, t).is_none() {
            late += 1;
        }
        thread::sleep(Duration::from_secs_f64(ft));
    }
    let (frames, bytes) = engine.usage();
    println!(
        "play {length:.1}s from {t0:.1}s: first frame {first:.1} ms, {late} of {due} ticks without their frame; cache {frames} frames, {} MB",
        bytes >> 20
    );
    Ok(())
}
