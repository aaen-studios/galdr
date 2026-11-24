//! How fast one decoder goes through a file, and where the time goes: decoding, copying frames
//! down from the GPU, and scaling them to the viewer.
//!
//!   cargo run --release -p media --example decode_speed -- FILE [HEIGHT]

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("pass a file");
    let height: u32 = args.next().map_or(Ok(900), |h| h.parse())?;
    let count = 120;
    for (hw, threads) in [("none", 8), ("none", 16), ("d3d11", 8), ("cuda", 8)] {
        let started = std::time::Instant::now();
        let (decode, download, scale, n, backend) =
            media::speed_test(path.as_ref(), media::Hw::parse(hw)?, threads, height, 5.0, count)?;
        let all = started.elapsed().as_secs_f64() * 1000.0;
        let per = |ms: f64| ms / n.max(1) as f64;
        println!(
            "{backend:24} {n} frames: {:6.1} fps; per frame decode {:5.2} download {:5.2} scale {:5.2} ms",
            n as f64 / (all / 1000.0),
            per(decode),
            per(download),
            per(scale)
        );
    }
    Ok(())
}
