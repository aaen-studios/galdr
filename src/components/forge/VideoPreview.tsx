import { useRef, useEffect, useState, useCallback } from "react";
import { useForgeStore } from "../../store/forgeStore";
import { convertFileSrc } from "@tauri-apps/api/core";

export default function VideoPreview() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const project = useForgeStore((s) => s.project);
  const setPlayhead = useForgeStore((s) => s.setPlayhead);
  const playheadTime = project.playheadTime;

  const activeClip = project.videoTrack.clips.find(
    (c) => playheadTime >= c.startTime && playheadTime < c.startTime + c.duration
  );

  const src = activeClip ? convertFileSrc(activeClip.sourcePath) : "";

  useEffect(() => {
    const vid = videoRef.current;
    if (!vid || !src) return;
    const seekTime = playheadTime - (activeClip?.startTime || 0) + (activeClip?.sourceStart || 0);
    if (Math.abs(vid.currentTime - seekTime) > 0.1) {
      vid.currentTime = seekTime;
    }
  }, [src, playheadTime, activeClip]);

  useEffect(() => {
    const vid = videoRef.current;
    if (!vid) return;
    const onLoaded = () => {
      setDuration(vid.duration);
      if (playing) vid.play();
    };
    const onTimeUpdate = () => {
      const t = vid.currentTime;
      setCurrentTime(t);
      if (activeClip) {
        const timelineT = activeClip.startTime + (t - activeClip.sourceStart);
        setPlayhead(timelineT);
      }
    };
    const onEnded = () => setPlaying(false);
    vid.addEventListener("loadedmetadata", onLoaded);
    vid.addEventListener("timeupdate", onTimeUpdate);
    vid.addEventListener("ended", onEnded);
    return () => {
      vid.removeEventListener("loadedmetadata", onLoaded);
      vid.removeEventListener("timeupdate", onTimeUpdate);
      vid.removeEventListener("ended", onEnded);
    };
  }, [src, activeClip, playing, setPlayhead]);

  useEffect(() => {
    if (!playing) videoRef.current?.pause();
  }, [playing]);

  const togglePlay = useCallback(() => {
    const vid = videoRef.current;
    if (!vid || !src) return;
    if (playing) {
      vid.pause();
      setPlaying(false);
    } else {
      const seek = playheadTime - (activeClip?.startTime || 0) + (activeClip?.sourceStart || 0);
      vid.currentTime = Math.max(0, seek);
      vid.play().then(() => setPlaying(true)).catch(() => {});
    }
  }, [playing, src, playheadTime, activeClip]);

  const stepFrame = useCallback((dir: number) => {
    const vid = videoRef.current;
    if (!vid || !src) return;
    const fps = project.fps || 30;
    const step = dir / fps;
    const newTime = Math.max(0, Math.min(vid.duration || 0, vid.currentTime + step));
    vid.currentTime = newTime;
    if (activeClip) {
      const timelineT = activeClip.startTime + (newTime - activeClip.sourceStart);
      setPlayhead(timelineT);
    }
  }, [src, activeClip, project.fps, setPlayhead]);

  const formatTime = (t: number) => {
    const m = Math.floor(t / 60);
    const s = Math.floor(t % 60);
    const ms = Math.floor((t % 1) * 10);
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${ms}`;
  };

  return (
    <div className="forge-preview">
      <div className="forge-preview-canvas">
        {src ? (
          <video ref={videoRef} className="forge-preview-video" src={src} preload="auto" />
        ) : (
          <div className="forge-preview-placeholder">
            <div className="forge-preview-placeholder-frame">
              <div className="forge-preview-placeholder-inner">
                <span className="forge-preview-placeholder-rune">ᚲ</span>
                <span className="forge-preview-placeholder-text">drop media here</span>
                <span className="forge-preview-placeholder-hint">or use + media in source</span>
              </div>
            </div>
          </div>
        )}
      </div>
      {src && (
        <div className="forge-transport">
          <button className="forge-transport-btn" onClick={() => stepFrame(-1)} title="Frame back">
            ⏮
          </button>
          <button className="forge-transport-btn forge-transport-play" onClick={togglePlay}>
            {playing ? "■" : "▶"}
          </button>
          <button className="forge-transport-btn" onClick={() => stepFrame(1)} title="Frame forward">
            ⏭
          </button>
          <span className="forge-transport-time">{formatTime(currentTime)}</span>
          <span className="forge-transport-sep">/</span>
          <span className="forge-transport-time dim">{formatTime(duration)}</span>
          <input
            type="range"
            className="forge-transport-scrub"
            min={0}
            max={duration || 0}
            step={1 / (project.fps || 30)}
            value={currentTime}
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              if (videoRef.current) videoRef.current.currentTime = v;
              if (activeClip) setPlayhead(activeClip.startTime + (v - activeClip.sourceStart));
            }}
          />
        </div>
      )}
    </div>
  );
}