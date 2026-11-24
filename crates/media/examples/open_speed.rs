//! What opening a file costs, step by step, as the app does it: the frame engine reading it,
//! the media bin's thumbnail, the timeline's waveform, and its filmstrip.
//!
//!   cargo run --release -p media --example open_speed -- FILE...

use std::time::Instant;

fn main() -> anyhow::Result<()> {
    for path in std::env::args().skip(1) {
        let p = std::path::Path::new(&path);
        let engine = media::Engine::new(media::Settings::default(), |_, _, _| ());
        let t = Instant::now();
        let info = engine.add_source(1, p)?;
        let add = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        media::thumbnail(p, (info.duration * 0.1).min(1.0), 180)?;
        let thumb = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        media::AudioReader::peaks(p, 100)?;
        let peaks = t.elapsed().as_secs_f64() * 1000.0;
        let count = (info.duration.ceil() as usize).clamp(2, 240);
        let times: Vec<f64> = (0..count).map(|i| info.duration * (i as f64 + 0.5) / count as f64).collect();
        let t = Instant::now();
        media::filmstrip(p, &times, 160)?;
        let strip = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{}: engine {add:.0} ms, thumbnail {thumb:.0} ms, waveform {peaks:.0} ms, filmstrip of {count} {strip:.0} ms",
            p.file_name().unwrap().to_string_lossy()
        );
    }
    Ok(())
}
