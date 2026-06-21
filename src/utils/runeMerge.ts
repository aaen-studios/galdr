import type { ConversionParams, PresetParams, RuneTag } from "../types";

/**
 * Merge a rune's preset into a conversion-params object.
 *
 * Rule: only the fields the rune actually sets (non-`undefined`) are applied.
 * Fields the rune leaves unset must NOT clobber the current values — a naive
 * spread would wipe them. The job-specific path fields are always preserved
 * (they describe the current file, not a preset).
 */
export function applyRuneToConversion(current: ConversionParams, rune: PresetParams): ConversionParams {
  const merged: ConversionParams = { ...current };
  (Object.keys(rune) as (keyof PresetParams)[]).forEach((key) => {
    const value = rune[key];
    if (value !== undefined) {
      (merged as unknown as Record<string, unknown>)[key] = value;
    }
  });
  // Paths are never part of a preset; always keep the current job's paths.
  merged.input_path = current.input_path;
  merged.output_dir = current.output_dir;
  return merged;
}

/**
 * Derive a preset-params snapshot from a conversion-params object by stripping
 * the two path fields. Used by "save as rune": whatever the user has configured
 * for the current job becomes the saved preset.
 */
export function conversionToPreset(params: ConversionParams): PresetParams {
  const { input_path: _in, output_dir: _out, ...preset } = params;
  return preset;
}

/** Pick a sensible rune glyph if one isn't already chosen. */
export function defaultRuneGlyph(): string {
  return "ᚠ";
}

/** Format a preset's params into a compact summary for cards / pickers. */
export function summarizePreset(p: PresetParams): string {
  const parts: string[] = [p.output_format.toUpperCase()];
  if (p.video_codec) parts.push(p.video_codec);
  if (p.crf !== undefined) parts.push(`CRF ${p.crf}`);
  if (p.audio_codec) parts.push(p.audio_codec);
  if (p.audio_bitrate) parts.push(p.audio_bitrate);
  if (p.resolution) parts.push(`${p.resolution[0]}x${p.resolution[1]}`);
  if (p.framerate) parts.push(`${p.framerate}fps`);
  return parts.join(" · ");
}

const AUDIO_FORMATS = ["mp3", "aac", "m4a", "ogg", "opus", "wav", "aiff", "flac", "wma", "ac3"];
const IMAGE_FORMATS = ["png", "jpeg", "jpg", "webp", "bmp", "tiff", "avif"];

/** High-level category for a preset, used for card badges & grouping. */
export type PresetType = "video" | "audio" | "image" | "animated" | "time";

export function presetType(p: PresetParams): PresetType {
  const fmt = p.output_format.toLowerCase();
  if (fmt === "gif") return "animated";
  if (IMAGE_FORMATS.includes(fmt)) return "image";
  if (AUDIO_FORMATS.includes(fmt)) return "audio";
  // Video preset that only alters time (speed/trim) and nothing else reads as "time".
  if ((p.speed_video !== undefined || p.speed_audio !== undefined || p.trim_start !== undefined || p.trim_end !== undefined)
      && p.video_codec === undefined && p.crf === undefined && p.resolution === undefined) {
    return "time";
  }
  return "video";
}

/** Human label for a preset type. */
export function presetTypeLabel(t: PresetType): string {
  switch (t) {
    case "video": return "video";
    case "audio": return "audio";
    case "image": return "image";
    case "animated": return "animated";
    case "time": return "time fx";
  }
}

export type { RuneTag };
