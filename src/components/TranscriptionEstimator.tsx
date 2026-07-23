import { useMemo } from "react";
import type { WhisperModel } from "../types";

interface Props {
  model: WhisperModel | undefined;
  durationSec: number;
}

/** Relative speed factor per tier (1.0 = baseline "fast"). Larger = slower. */
const TIER_FACTOR: Record<WhisperModel["tier"], number> = {
  fast: 1.0,
  balanced: 1.8,
  accurate: 3.2,
  best: 5.0,
};

/**
 * Estimate transcription cost from the model tier and input duration.
 *
 * whisper.cpp runs roughly in real-time on CPU for the "tiny/base" models and
 * slower for larger ones. The factor below is a rough heuristic — actual
 * speed depends heavily on CPU/GPU — but it gives the user a sane ballpark
 * instead of committing to a 3-hour transcription blindly.
 *
 * RAM footprint ≈ model size × ~1.6 (weights + KV cache + decode buffers).
 */
function estimate(model: WhisperModel, durationSec: number) {
  const factor = TIER_FACTOR[model.tier] ?? 2.0;
  // whisper.cpp typically processes faster than real-time on small models.
  // Estimate: processing time ≈ duration / speedup, where speedup shrinks
  // with model tier. Small models ~6× faster than realtime, large ~1.2×.
  const speedup = 6 / factor;
  const etaSec = durationSec / Math.max(speedup, 0.18);
  const ramMb = (model.sizeBytes / (1024 * 1024)) * 1.6;
  return { etaSec, ramMb };
}

function fmtDuration(sec: number): string {
  if (!isFinite(sec) || sec <= 0) return "—";
  if (sec < 60) return `~${Math.round(sec)}s`;
  const m = Math.floor(sec / 60);
  const s = Math.round(sec % 60);
  if (m < 60) return `~${m}m ${s}s`;
  const h = Math.floor(m / 60);
  return `~${h}h ${m % 60}m`;
}

export default function TranscriptionEstimator({ model, durationSec }: Props) {
  const est = useMemo(
    () => (model && durationSec > 0 ? estimate(model, durationSec) : null),
    [model, durationSec],
  );

  if (!model || !est) return null;

  return (
    <div className="card estimator-card">
      <label className="label">estimated cost</label>
      <div className="estimator-row">
        <div className="estimator-stat">
          <span className="estimator-stat-label">time</span>
          <span className="estimator-stat-val">{fmtDuration(est.etaSec)}</span>
        </div>
        <div className="estimator-stat">
          <span className="estimator-stat-label">memory</span>
          <span className="estimator-stat-val">~{Math.round(est.ramMb)} MB</span>
        </div>
        <div className="estimator-stat">
          <span className="estimator-stat-label">accuracy</span>
          <span className="estimator-stat-val">{model.tier}</span>
        </div>
      </div>
      <p className="estimator-hint">
        heuristic estimate — actual speed depends on your hardware. larger models
        are more accurate but slower and hungrier.
      </p>
    </div>
  );
}
