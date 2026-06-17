export interface FlagDef {
  flag: string;
  description: string;
  category: "codec" | "quality" | "filter" | "format" | "audio" | "general";
}

export const FFMPEG_FLAGS: FlagDef[] = [
  { flag: "-y", description: "Overwrite output files without asking", category: "general" },
  { flag: "-i", description: "Input file path", category: "general" },
  { flag: "-c:v", description: "Video codec (e.g. libx264, libx265, libwebp)", category: "codec" },
  { flag: "-c:a", description: "Audio codec (e.g. aac, libmp3lame, flac)", category: "codec" },
  { flag: "-crf", description: "Constant Rate Factor — lower = better quality (0–51 for H.264, 0–63 for VP9)", category: "quality" },
  { flag: "-preset", description: "Encoding speed/quality tradeoff (ultrafast, fast, medium, slow, veryslow)", category: "codec" },
  { flag: "-b:v", description: "Video bitrate (e.g. 2M, 500k)", category: "quality" },
  { flag: "-b:a", description: "Audio bitrate (e.g. 192k, 320k)", category: "quality" },
  { flag: "-vf", description: "Video filter graph (scale, fps, crop, etc.)", category: "filter" },
  { flag: "-pix_fmt", description: "Pixel format (yuv420p for maximum compatibility)", category: "format" },
  { flag: "-vn", description: "Disable video output (audio-only conversion)", category: "general" },
  { flag: "-ac", description: "Audio channel count (1 = mono, 2 = stereo)", category: "audio" },
  { flag: "-q:v", description: "Video quality for constant-quality codecs (JPEG: 1–31, lower = better)", category: "quality" },
  { flag: "-quality", description: "Quality percentage (WebP/AVIF: 0–100, higher = better)", category: "quality" },
  { flag: "-compression_level", description: "Compression level (PNG: 0–9, FLAC: 0–8, higher = smaller)", category: "quality" },
  { flag: "-compression_algo", description: "Compression algorithm for TIFF (raw, deflate, lzw)", category: "codec" },
  { flag: "-ss", description: "Seek to timestamp before input (start trim point in seconds)", category: "general" },
  { flag: "-to", description: "Stop writing at timestamp after input (end trim point in seconds)", category: "general" },
  { flag: "-af", description: "Audio filter graph (atempo, volume, etc.)", category: "filter" },
  { flag: "-ar", description: "Audio sample rate in Hz (e.g. 44100, 48000)", category: "audio" },
];

export function getFlagDef(flag: string): FlagDef | undefined {
  return FFMPEG_FLAGS.find((f) => f.flag === flag);
}
