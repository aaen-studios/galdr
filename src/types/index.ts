export interface ConversionParams {
  input_path: string;
  output_dir: string;
  output_format: string;
  video_codec?: string;
  audio_codec?: string;
  video_bitrate?: string;
  audio_bitrate?: string;
  resolution?: [number, number];
  framerate?: number;
  crf?: number;
  preset?: string;
  quality?: number;
  trim_start?: number;
  trim_end?: number;
  crop_w?: number;
  crop_h?: number;
  crop_x?: number;
  crop_y?: number;
  crop_ratio?: string;
  speed_video?: number;
  speed_audio?: number;
  rotate?: number;
  sample_rate?: number;
  channels?: number;
}

export interface PresetParams {
  output_format: string;
  video_codec?: string;
  audio_codec?: string;
  video_bitrate?: string;
  audio_bitrate?: string;
  resolution?: [number, number];
  framerate?: number;
  crf?: number;
  preset?: string;
  quality?: number;
}

export interface RuneTag {
  id: string;
  name: string;
  rune: string;
  description: string;
  params: PresetParams;
}

export interface StreamInfo {
  index: number;
  kind: string;
  codec: string;
  width?: number;
  height?: number;
  frame_rate?: number;
  sample_rate?: number;
  channels?: number;
  bitrate?: number;
  language?: string;
}

export interface MediaInfo {
  container: string;
  streams: StreamInfo[];
  duration: number;
  bitrate?: number;
  size: number;
}

export interface ConversionProgress {
  job_id: string;
  progress: number;
}

export interface ConversionDone {
  job_id: string;
  output_path: string;
}

export interface ScannedFile {
  path: string;
  name: string;
  size: number;
}

export interface BatchProgress {
  total: number;
  done: number;
  failed: number;
  current_file: string;
  file_progress: number;
}
