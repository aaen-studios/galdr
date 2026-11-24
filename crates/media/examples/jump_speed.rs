//! How long one decoder takes to reach a frame from the keyframe before it, with each backend,
//! with and without skipping the frames passed over, and whether the frame reached is the same.
//!
//!   cargo run --release -p media --example jump_speed -- FILE [SECONDS,SECONDS,...]

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("pass a file");
    // Just before keyframes 1, 2 and 10 s in: the longest decodes in a 1 s, 2 s or 10 s group.
    let targets: Vec<f64> = std::env::args().nth(2).map_or(vec![0.98, 1.98, 9.95, 15.5], |s| s.split(",").map(|x| x.parse().unwrap()).collect());
    for (hw, threads) in [("none", 8), ("none", 16), ("d3d11", 8), ("cuda", 8)] {
        for skip in [false, true] {
            let mut cells = Vec::new();
            let mut same = true;
            for &t in &targets {
                let (ms, hash) = media::jump_test(path.as_ref(), media::Hw::parse(hw)?, threads, 900, t, skip)?;
                let (_, plain) = media::jump_test(path.as_ref(), media::Hw::parse(hw)?, threads, 900, t, false)?;
                same &= hash == plain;
                cells.push(format!("{ms:6.0}"));
            }
            println!(
                "{hw:6} x{threads:2} {:9}: to {targets:?} s in {} ms{}",
                if skip { "skipping" } else { "all" },
                cells.join(" "),
                if same { "" } else { "  FRAMES DIFFER" }
            );
        }
    }
    Ok(())
}
