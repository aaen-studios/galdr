// Mirrors src-tauri/src/ffmpeg/builder.rs — keep in sync when changing command generation
import type { ConversionParams } from "../types";

function quotePath(p: string): string {
  return p.includes(" ") ? `"${p}"` : p;
}

function audioBitrate(quality: number): string {
  if (quality >= 0.95) return "320k";
  if (quality >= 0.85) return "256k";
  if (quality >= 0.70) return "192k";
  if (quality >= 0.50) return "128k";
  if (quality >= 0.30) return "96k";
  if (quality >= 0.15) return "64k";
  if (quality >= 0.05) return "32k";
  if (quality >= 0.02) return "16k";
  return "8k";
}

function maybeMono(quality: number): string | null {
  if (quality < 0.15) return "-ac 1";
  return null;
}

/** Check if a codec name is a known hardware encoder. */
function isHwEncoder(codec: string): boolean {
  return /nvenc|amf|qsv|videotoolbox|vaapi/i.test(codec);
}

/** Parse a bitrate string like "128k" to bps. */
function parseBitrate(s: string): number {
  const v = s.trim().toLowerCase();
  if (v.endsWith("k")) return parseFloat(v) * 1000;
  if (v.endsWith("m")) return parseFloat(v) * 1_000_000;
  return parseFloat(v) || 128_000;
}

/** Format bps to ffmpeg "k" string. */
function fmtBitrate(bps: number): string {
  return `${Math.round(Math.max(bps / 1000, 1))}k`;
}

export function buildFFmpegCommand(params: ConversionParams, duration?: number, availableEncoders?: HardwareEncoderInfo[]): string {
  const parts: string[] = ["ffmpeg"];

  const fmt = params.output_format.toLowerCase();
  const isAudio = ["mp3", "aac", "m4a", "ogg", "opus", "wav", "aiff", "flac", "wma", "ac3"].includes(fmt);
  const isImage = ["jpg", "jpeg", "png", "webp", "avif", "bmp", "tiff"].includes(fmt);

  // ── Target-size mode ──
  if (params.target_size_bytes && duration && duration > 0) {
    const targetBytes = params.target_size_bytes;
    const totalBits = targetBytes * 8;
    const targetBitrateBps = (totalBits / duration) * 0.95;

    let audioBps: number;
    if (params.audio_bitrate) {
      audioBps = parseBitrate(params.audio_bitrate);
    } else if (params.quality !== undefined) {
      audioBps = parseBitrate(audioBitrate(params.quality));
    } else {
      audioBps = 128_000;
    }

    if (isImage) {
      // Images: fall back to quality mode
      return buildFFmpegCommand({ ...params, target_size_bytes: undefined }, duration, availableEncoders);
    }

    if (isAudio || fmt === "gif") {
      // Single pass with audio bitrate targeting
      const audioTarget = Math.max(targetBitrateBps, 16_000);
      return buildFFmpegCommand({
        ...params,
        target_size_bytes: undefined,
        audio_bitrate: fmtBitrate(audioTarget),
      }, duration, availableEncoders);
    }

    // Video: two-pass encoding
    const videoBps = audioBps >= targetBitrateBps ? 100_000 : Math.max(targetBitrateBps - audioBps, 100_000);
    const videoBr = fmtBitrate(videoBps);
    const audioBr = fmtBitrate(audioBps);

    return buildFFmpegCommand({
      ...params,
      target_size_bytes: undefined,
      video_bitrate: videoBr,
      audio_bitrate: audioBr,
    }, duration, availableEncoders) + "\n# two-pass encode (1st pass: analysis only)";
  }

  // ── Normal quality mode ──
  parts.push("-y");

  if (params.trim_start !== undefined && params.trim_start !== null && params.trim_start > 0) {
    parts.push("-ss", String(params.trim_start));
  }

  parts.push("-i");
  parts.push(quotePath(params.input_path));

  if (params.video_codec) {
    parts.push("-c:v", params.video_codec);
  } else if (params.preferred_video_encoder) {
    const resolved = resolvePreferredEncoder(
      params.preferred_video_encoder,
      params.output_format,
      availableEncoders ?? [],
    );
    if (resolved) {
      parts.push("-c:v", resolved);
    }
  }

  if (params.audio_codec) {
    parts.push("-c:a", params.audio_codec);
  }

  if (params.crf !== undefined && params.crf !== null) {
    parts.push("-crf", String(params.crf));
  }

  if (params.preset) {
    parts.push("-preset", params.preset);
  }

  if (params.video_bitrate) {
    parts.push("-b:v", params.video_bitrate);
  }

  if (params.audio_bitrate) {
    parts.push("-b:a", params.audio_bitrate);
  }

  if (params.trim_end !== undefined && params.trim_end !== null && params.trim_end > 0) {
    parts.push("-to", String(params.trim_end));
  }

  const filterParts: string[] = [];

  // Crop (ratio preset or manual)
  if (params.crop_ratio) {
    const r = params.crop_ratio === "16:9" ? "16/9"
      : params.crop_ratio === "4:3" ? "4/3"
      : params.crop_ratio === "1:1" ? "1/1"
      : params.crop_ratio === "9:16" ? "9/16"
      : "16/9";
    filterParts.push(
      `crop='min(iw\\,ih*${r})':'min(ih\\,iw/${r})':'(iw-min(iw\\,ih*${r}))/2':'(ih-min(ih\\,iw/${r}))/2'`,
    );
  } else if (params.crop_w !== undefined || params.crop_h !== undefined) {
    const cw = params.crop_w ?? 0;
    const ch = params.crop_h ?? 0;
    const cx = params.crop_x ?? 0;
    const cy = params.crop_y ?? 0;
    const cwEven = cw > 0 ? cw - (cw % 2) : 0;
    const chEven = ch > 0 ? ch - (ch % 2) : 0;
    if (cwEven > 0 && chEven > 0) {
      filterParts.push(`crop=${cwEven}:${chEven}:${cx}:${cy}`);
    }
  }

  // Rotate
  if (params.rotate) {
    if (params.rotate === 90) filterParts.push("transpose=1");
    else if (params.rotate === 180) filterParts.push("transpose=1,transpose=1");
    else if (params.rotate === 270) filterParts.push("transpose=2");
  }

  // Flip (horizontal/vertical)
  if (params.flip === "h") filterParts.push("hflip");
  else if (params.flip === "v") filterParts.push("vflip");

  // Video speed
  if (params.speed_video !== undefined && params.speed_video !== null && params.speed_video > 0 && Math.abs(params.speed_video - 1) > 1e-9) {
    filterParts.push(`setpts=${1 / params.speed_video}*PTS`);
  }

  if (params.resolution) {
    const [w, h] = params.resolution;
    filterParts.push(`scale=${w}:${h}:flags=lanczos`);
  }

  if (params.framerate !== undefined && params.framerate !== null) {
    filterParts.push(`fps=${params.framerate}`);
  }

  if (params.quality !== undefined && params.quality !== null) {
    if (params.quality < 0.25 && !params.resolution) {
      const scale = params.quality < 0.05 ? 0.35
        : params.quality < 0.10 ? 0.50
        : params.quality < 0.15 ? 0.60
        : params.quality < 0.20 ? 0.75
        : 0.85;
      filterParts.push(
        `scale='trunc(iw*${scale}/2)*2':'trunc(ih*${scale}/2)*2':flags=lanczos`,
      );
    }
  }

  if (params.quality !== undefined && params.quality !== null) {
    switch (fmt) {
      // ── Video formats ──
      case "mp4": case "m4v": case "mov": case "avi": case "flv":
      case "ogv": case "wmv": case "ts": case "3gp": {
        if (!params.video_codec && params.crf === undefined && !params.video_bitrate) {
          const crf = Math.round(Math.min(Math.max(51.0 - params.quality * 50.0, 0), 51));
          parts.push("-crf", String(crf));
        }
        if (!params.audio_codec && !params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        break;
      }
      case "mkv": case "webm": {
        if (!params.video_codec && params.crf === undefined && !params.video_bitrate) {
          const crf = Math.round(Math.min(Math.max(63.0 - params.quality * 63.0, 0), 63));
          parts.push("-crf", String(crf));
        }
        if (!params.audio_codec && !params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        break;
      }

      // ── GIF ──
      case "gif": {
        const maxColors = Math.round(Math.min(Math.max(4.0 + params.quality * 252.0, 4), 256));
        const fps = Math.round(Math.min(Math.max(2.0 + params.quality * 28.0, 2), 30));
        const bayerScale = Math.round(Math.min(Math.max((1.0 - params.quality) * 5.0, 0), 5));
        const prefix = filterParts.length > 0 ? `${filterParts.join(",")},` : "";
        const vf = `${prefix}fps=${fps},split[s0][s1];[s0]palettegen=max_colors=${maxColors}:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=${bayerScale}`;
        filterParts.length = 0;
        parts.push("-vf", vf);
        break;
      }

      // ── Image formats ──
      case "jpg": case "jpeg": {
        const qv = Math.round(Math.min(Math.max(1.0 + (1.0 - params.quality) * 30.0, 1), 31));
        if (!params.video_codec) parts.push("-c:v", "mjpeg");
        parts.push("-q:v", String(qv));
        break;
      }
      case "webp": {
        const q = Math.round(Math.min(Math.max(params.quality * 100.0, 0), 100));
        if (!params.video_codec) parts.push("-c:v", "libwebp");
        parts.push("-quality", String(q));
        break;
      }
      case "avif": {
        const q = Math.round(Math.min(Math.max(params.quality * 100.0, 0), 100));
        parts.push("-quality", String(q));
        break;
      }
      case "png": {
        const level = Math.round(Math.min(Math.max(1.0 + params.quality * 8.0, 1), 9));
        if (!params.video_codec) parts.push("-c:v", "png");
        parts.push("-compression_level", String(level));
        break;
      }
      case "bmp": {
        if (!params.video_codec) parts.push("-c:v", "bmp");
        break;
      }
      case "tiff": {
        const comp = params.quality >= 0.7 ? "lzw" : params.quality >= 0.4 ? "deflate" : "raw";
        if (!params.video_codec) parts.push("-c:v", "tiff");
        parts.push("-compression_algo", comp);
        break;
      }

      // ── Audio formats ──
      case "mp3": {
        if (!params.audio_codec) parts.push("-c:a", "libmp3lame");
        if (!params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        parts.push("-vn");
        break;
      }
      case "aac": case "m4a": {
        if (!params.audio_codec) parts.push("-c:a", "aac");
        if (!params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        parts.push("-vn");
        break;
      }
      case "ogg": case "opus": {
        if (!params.audio_codec) {
          parts.push("-c:a", fmt === "opus" ? "libopus" : "libvorbis");
        }
        if (!params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        parts.push("-vn");
        break;
      }
      case "wav": case "aiff": {
        if (!params.audio_bitrate) {
          parts.push("-b:a", "1411k");
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        parts.push("-vn");
        break;
      }
      case "flac": {
        if (!params.audio_codec) parts.push("-c:a", "flac");
        const level = Math.round(Math.min(Math.max(params.quality * 8.0, 0), 8));
        parts.push("-compression_level", String(level));
        parts.push("-vn");
        break;
      }
      case "wma": {
        if (!params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        parts.push("-vn");
        break;
      }
      case "ac3": {
        if (!params.audio_codec) parts.push("-c:a", "ac3");
        if (!params.audio_bitrate) {
          parts.push("-b:a", audioBitrate(params.quality));
          const mono = maybeMono(params.quality);
          if (mono) parts.push(...mono.split(" "));
        }
        parts.push("-vn");
        break;
      }
    }
  }

  // Pixel format for compatible video output
  const audioFormats = new Set([
    "mp3", "aac", "m4a", "ogg", "opus", "wav", "aiff", "flac", "wma", "ac3",
  ]);
  if (!audioFormats.has(fmt) && fmt !== "gif" && params.video_codec !== "png") {
    const effectiveCodec = params.video_codec || resolvePreferredEncoder(
      params.preferred_video_encoder ?? "",
      params.output_format,
      availableEncoders ?? [],
    );
    if (!effectiveCodec || effectiveCodec === "libx264" || isHwEncoder(effectiveCodec)) {
      parts.push("-pix_fmt", "yuv420p");
    }
  }

  // Remaining filter parts
  if (filterParts.length > 0) {
    parts.push("-vf", filterParts.join(","));
  }

  // Audio filter chain: speed (atempo) + normalization + fades, joined
  // into a single -af. Note: the real builder (Rust) computes the fade-out
  // start from the probed duration; this client-side preview omits the
  // exact start time since it can't probe here.
  const afParts: string[] = [];

  if (params.speed_audio !== undefined && params.speed_audio !== null && params.speed_audio > 0 && Math.abs(params.speed_audio - 1) > 1e-9) {
    let remaining = params.speed_audio;
    while (remaining < 0.5) {
      afParts.push("atempo=0.5");
      remaining /= 0.5;
    }
    while (remaining > 2.0) {
      afParts.push("atempo=2.0");
      remaining /= 2.0;
    }
    if (Math.abs(remaining - 1) > 1e-9) {
      afParts.push(`atempo=${remaining}`);
    }
  }

  if (params.audio_normalize === "loudnorm") {
    afParts.push("loudnorm=I=-16:TP=-1.5:LRA=11");
  } else if (params.audio_normalize === "dynaudnorm") {
    afParts.push("dynaudnorm");
  }

  if (params.fade_in !== undefined && params.fade_in > 0) {
    afParts.push(`afade=t=in:st=0:d=${params.fade_in}`);
  }
  if (params.fade_out !== undefined && params.fade_out > 0) {
    afParts.push(`afade=t=out:d=${params.fade_out}`);
  }

  if (afParts.length > 0) {
    parts.push("-af", afParts.join(","));
  }

  // Sample rate and channels
  if (params.sample_rate !== undefined && params.sample_rate !== null) {
    parts.push("-ar", String(params.sample_rate));
  }

  if (params.channels !== undefined && params.channels !== null) {
    parts.push("-ac", String(params.channels));
  }

  // Output path
  const inputStem = params.input_path
    .split("/").pop()?.split("\\").pop()
    ?.replace(/\.[^.]+$/, "") || "output";
  const outDir = params.output_dir || ".";
  const outputPath = `${outDir}/${inputStem}.${fmt}`;
  parts.push(quotePath(outputPath));

  return parts.join(" ");
}

import type { HardwareEncoderInfo } from "../types";

/**
 * Resolve the user's preferred video encoder to a concrete encoder name.
 *
 * - "software" or undefined → returns undefined (use default software codec)
 * - concrete encoder name (e.g. "h264_nvenc") → returns it as-is
 * - "auto" → picks the best available hardware encoder for the format
 *
 * Priority order: NVIDIA > AMD > Intel > Apple > VAAPI.
 */
export function resolvePreferredEncoder(
  preferred: string | undefined,
  outputFormat: string,
  availableEncoders: HardwareEncoderInfo[],
): string | undefined {
  if (!preferred || preferred === "software") return undefined;
  if (preferred !== "auto") return preferred;

  const fmt = outputFormat.toLowerCase();
  const isH264 = ["mp4", "m4v", "mov", "avi", "flv", "ts", "3gp"].includes(fmt);
  const isHEVC = ["mkv"].includes(fmt);
  if (!isH264 && !isHEVC) return undefined;

  const targetCodec = isH264 ? "h264" : "hevc";
  const vendorOrder = ["nvidia", "amd", "intel", "apple", "vaapi"];

  for (const vendor of vendorOrder) {
    const enc = availableEncoders.find(
      (e) => e.codec === targetCodec && e.vendor === vendor,
    );
    if (enc) return enc.name;
  }
  return undefined;
}
