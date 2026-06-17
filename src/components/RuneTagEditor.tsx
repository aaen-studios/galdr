import { useState, useMemo } from "react";
import { motion } from "framer-motion";
import Dropdown from "./Dropdown";
import { FMT_OPTIONS } from "../options";
import type { RuneTag, PresetParams } from "../types";

const ELDER_FUTHARK = [
  "ᚠ", "ᚢ", "ᚦ", "ᚨ", "ᚱ", "ᚲ", "ᚷ", "ᚹ", "ᚺ", "ᚾ",
  "ᛁ", "ᛃ", "ᛇ", "ᛈ", "ᛉ", "ᛊ", "ᛏ", "ᛒ", "ᛖ", "ᛗ",
  "ᛚ", "ᛝ", "ᛟ", "ᛞ",
];

interface Props {
  tag?: RuneTag;
  onSave: (tag: RuneTag) => void;
  onCancel: () => void;
}

const emptyParams: PresetParams = {
  output_format: "mp4",
  video_codec: undefined,
  audio_codec: undefined,
  video_bitrate: undefined,
  audio_bitrate: undefined,
  resolution: undefined,
  framerate: undefined,
  crf: undefined,
  preset: undefined,
  quality: undefined,
};

export default function RuneTagEditor({ tag, onSave, onCancel }: Props) {
  const [name, setName] = useState(tag?.name ?? "");
  const [rune, setRune] = useState(tag?.rune ?? "ᚠ");
  const [description, setDescription] = useState(tag?.description ?? "");
  const [params, setParams] = useState<PresetParams>(tag?.params ?? { ...emptyParams });

  const fmtOptions = useMemo(
    () => FMT_OPTIONS.map((o) => ({ value: o.value, label: o.label })),
    [],
  );

  const setParam = <K extends keyof PresetParams>(key: K, value: PresetParams[K]) =>
    setParams((p) => ({ ...p, [key]: value }));

  const handleSave = () => {
    if (!name.trim()) return;
    onSave({
      id: tag?.id ?? "",
      name: name.trim(),
      rune,
      description: description.trim(),
      params,
    });
  };

  const paramInput = (
    label: string,
    key: keyof PresetParams,
    placeholder: string,
    type: "text" | "number" = "text",
  ) => (
    <label className="rune-editor-field">
      <span className="rune-editor-label">{label}</span>
      <input
        type={type}
        className="input"
        placeholder={placeholder}
        value={(params[key] as string | number) ?? ""}
        onChange={(e) => {
          const val = e.target.value;
          if (type === "number") {
            setParam(key, val ? (key === "crf" || key === "quality" ? Number(val) : type === "number" ? Number(val) : val) : undefined);
          } else {
            setParam(key, val || undefined);
          }
        }}
      />
    </label>
  );

  return (
    <motion.div
      className="rune-editor-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
    >
      <motion.div
        className="rune-editor"
        initial={{ scale: 0.95, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.95, opacity: 0 }}
      >
        <div className="rune-editor-header">
          <span className="rune-editor-rune">{rune}</span>
          <span className="rune-editor-title">
            {tag ? `edit ${tag.name}` : "new rune tag"}
          </span>
        </div>

        <div className="rune-editor-body">
          <label className="rune-editor-field">
            <span className="rune-editor-label">name</span>
            <input
              type="text"
              className="input"
              placeholder="e.g. Fehu"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>

          <label className="rune-editor-field">
            <span className="rune-editor-label">rune</span>
            <div className="rune-picker">
              <input
                type="text"
                className="input rune-picker-input"
                placeholder="ᚠ"
                value={rune}
                onChange={(e) => setRune(e.target.value)}
                maxLength={2}
              />
              <div className="rune-picker-grid">
                {ELDER_FUTHARK.map((r) => (
                  <button
                    key={r}
                    className={`rune-picker-btn${r === rune ? " active" : ""}`}
                    onClick={() => setRune(r)}
                  >
                    {r}
                  </button>
                ))}
              </div>
            </div>
          </label>

          <label className="rune-editor-field">
            <span className="rune-editor-label">description</span>
            <input
              type="text"
              className="input"
              placeholder="e.g. Archive: H.265 CRF 18, FLAC audio"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </label>

          <div className="rune-editor-divider">
            <span className="rune-editor-divider-label">conversion params</span>
          </div>

          <label className="rune-editor-field">
            <span className="rune-editor-label">format</span>
            <Dropdown
              options={fmtOptions}
              value={params.output_format}
              onChange={(v) => setParam("output_format", v)}
              placeholder="select format"
            />
          </label>

          {paramInput("video codec", "video_codec", "e.g. libx264")}
          {paramInput("audio codec", "audio_codec", "e.g. aac")}
          {paramInput("video bitrate", "video_bitrate", "e.g. 2M")}
          {paramInput("audio bitrate", "audio_bitrate", "e.g. 128k")}

          <label className="rune-editor-field">
            <span className="rune-editor-label">resolution (WxH)</span>
            <div className="rune-editor-row">
              <input
                type="number"
                className="input"
                placeholder="width"
                value={params.resolution?.[0] ?? ""}
                onChange={(e) => {
                  const w = e.target.value ? Number(e.target.value) : undefined;
                  const h = params.resolution?.[1];
                  setParam("resolution", w !== undefined && h !== undefined ? [w, h] : undefined);
                }}
              />
              <span className="rune-editor-sep">x</span>
              <input
                type="number"
                className="input"
                placeholder="height"
                value={params.resolution?.[1] ?? ""}
                onChange={(e) => {
                  const w = params.resolution?.[0];
                  const h = e.target.value ? Number(e.target.value) : undefined;
                  setParam("resolution", w !== undefined && h !== undefined ? [w, h] : undefined);
                }}
              />
            </div>
          </label>

          {paramInput("framerate", "framerate", "e.g. 30", "number")}
          {paramInput("CRF", "crf", "0-51 (lower = better)", "number")}
          {paramInput("preset", "preset", "e.g. medium, fast, slow")}
          {paramInput("quality", "quality", "0-100 (higher = better)", "number")}
        </div>

        <div className="rune-editor-footer">
          <button className="btn" onClick={onCancel}>
            cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={!name.trim()}
          >
            save
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}