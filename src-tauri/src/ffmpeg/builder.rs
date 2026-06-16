use crate::models::ConversionParams;

pub fn build_args(params: &ConversionParams) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    args.push("-y".to_string());
    args.push("-i".to_string());
    args.push(params.input_path.to_string_lossy().to_string());

    if let Some(codec) = &params.video_codec {
        args.push("-c:v".to_string());
        args.push(codec.clone());
    }

    if let Some(codec) = &params.audio_codec {
        args.push("-c:a".to_string());
        args.push(codec.clone());
    }

    if let Some(crf) = params.crf {
        args.push("-crf".to_string());
        args.push(crf.to_string());
    }

    if let Some(preset) = &params.preset {
        args.push("-preset".to_string());
        args.push(preset.clone());
    }

    if let Some(bitrate) = &params.video_bitrate {
        args.push("-b:v".to_string());
        args.push(bitrate.clone());
    }

    if let Some(bitrate) = &params.audio_bitrate {
        args.push("-b:a".to_string());
        args.push(bitrate.clone());
    }

    if let Some((w, h)) = params.resolution {
        args.push("-vf".to_string());
        args.push(format!("scale={}:{}", w, h));
    }

    if let Some(fps) = params.framerate {
        args.push("-r".to_string());
        args.push(fps.to_string());
    }

    let input_stem = params.input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let output_path = params.output_dir.join(format!(
        "{}.{}",
        input_stem,
        params.output_format
    ));

    args.push(output_path.to_string_lossy().to_string());

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_args_minimal() {
        let params = ConversionParams {
            input_path: PathBuf::from("input.mp4"),
            output_dir: PathBuf::from("."),
            output_format: "avi".to_string(),
            ..Default::default()
        };
        let args = build_args(&params);
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"input.mp4".to_string()));
        let last = args.last().unwrap();
        assert!(last.ends_with("input.avi"), "last arg should end with input.avi, got: {last}");
    }

    #[test]
    fn test_build_args_full() {
        let params = ConversionParams {
            input_path: PathBuf::from("input.mkv"),
            output_dir: PathBuf::from("out"),
            output_format: "mp4".to_string(),
            video_codec: Some("libx264".to_string()),
            audio_codec: Some("aac".to_string()),
            crf: Some(23),
            preset: Some("medium".to_string()),
            resolution: Some((1920, 1080)),
            ..Default::default()
        };
        let args = build_args(&params);
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-crf".to_string()));
        assert!(args.contains(&"23".to_string()));
    }
}
