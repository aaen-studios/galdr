import { useRef, useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useForgeStore } from "../../store/forgeStore";
import type { ForgeClip } from "../../types";

const MIME_TYPES: Record<string, string> = {
  mp4: "video/mp4", mkv: "video/x-matroska", avi: "video/x-msvideo",
  mov: "video/quicktime", webm: "video/webm", m4v: "video/mp4",
  flv: "video/x-flv", ogv: "video/ogg", wmv: "video/x-ms-wmv",
  mp3: "audio/mpeg", flac: "audio/flac", wav: "audio/wav",
  aac: "audio/aac", ogg: "audio/ogg", opus: "audio/opus",
  m4a: "audio/mp4", aiff: "audio/aiff", wma: "audio/x-ms-wma",
};

interface SlotState {
  clipId: string;
  blobUrl: string;
  ready: boolean;
  loading: boolean;
}

function emptySlot(): SlotState {
  return { clipId: "", blobUrl: "", ready: false, loading: false };
}

export default function VideoPreview() {
  const videoARef = useRef<HTMLVideoElement>(null);
  const videoBRef = useRef<HTMLVideoElement>(null);
  const audioRef = useRef<HTMLAudioElement>(null);

  const activeSlotRef = useRef<0 | 1>(0);
  const slotsRef = useRef<[SlotState, SlotState]>([emptySlot(), emptySlot()]);
  const playingRef = useRef(false);
  const playheadTimeRef = useRef(0);

  const videoBlobCacheRef = useRef<Map<string, string>>(new Map());
  const audioBlobCacheRef = useRef<Map<string, string>>(new Map());

  const audioClipIdRef = useRef<string | null>(null);
  const audioBlobUrlRef = useRef<string | null>(null);

  const vClipsRef = useRef<ForgeClip[]>([]);
  const aClipsRef = useRef<ForgeClip[]>([]);
  const fpsRef = useRef(30);

  const onTimeUpdateRef = useRef<() => void>(() => {});
  const transitioningRef = useRef(false);

  const [activeSlot, setActiveSlot] = useState<0 | 1>(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [hasError, setHasError] = useState<string | null>(null);

  const project = useForgeStore((s) => s.project);
  const setPlayhead = useForgeStore((s) => s.setPlayhead);
  const playheadTime = project.playheadTime;

  vClipsRef.current = project.videoTrack.clips;
  aClipsRef.current = project.audioTrack.clips;
  fpsRef.current = project.fps || 30;

  const loadBlob = useCallback(
    async (path: string, cache: Map<string, string>): Promise<string> => {
      const existing = cache.get(path);
      if (existing) return existing;
      const bytes = await invoke<number[]>("read_file_bytes", { path });
      const ext = path.split(".").pop()?.toLowerCase() || "";
      const url = URL.createObjectURL(
        new Blob([new Uint8Array(bytes)], { type: MIME_TYPES[ext] || "video/mp4" })
      );
      cache.set(path, url);
      return url;
    },
    []
  );

  const getActiveVideo = useCallback(() => {
    return activeSlotRef.current === 0 ? videoARef.current : videoBRef.current;
  }, []);

  const findNextClip = useCallback((afterTime: number): ForgeClip | null => {
    const sorted = [...vClipsRef.current].sort((a, b) => a.startTime - b.startTime);
    return sorted.find((c) => c.startTime >= afterTime) || null;
  }, []);

  const getCurrentClip = useCallback((): ForgeClip | null => {
    const slot = slotsRef.current[activeSlotRef.current];
    return vClipsRef.current.find((c) => c.id === slot.clipId) || null;
  }, []);

  const loadClipIntoSlot = useCallback(
    async (slotIdx: 0 | 1, clip: ForgeClip) => {
      const cur = slotsRef.current[slotIdx];
      if (cur.clipId === clip.id && cur.ready) return;

      slotsRef.current[slotIdx] = {
        clipId: clip.id,
        blobUrl: "",
        ready: false,
        loading: true,
      };

      const vid = slotIdx === 0 ? videoARef.current : videoBRef.current;
      if (!vid) {
        slotsRef.current[slotIdx].loading = false;
        return;
      }

      try {
        const url = await loadBlob(clip.sourcePath, videoBlobCacheRef.current);
        if (slotsRef.current[slotIdx].clipId !== clip.id) return;

        slotsRef.current[slotIdx].blobUrl = url;
        vid.src = url;
        vid.load();

        await new Promise<void>((resolve) => {
          const onCanPlay = () => {
            vid.removeEventListener("canplay", onCanPlay);
            resolve();
          };
          vid.addEventListener("canplay", onCanPlay);
          setTimeout(() => {
            vid.removeEventListener("canplay", onCanPlay);
            resolve();
          }, 5000);
        });

        if (slotsRef.current[slotIdx].clipId !== clip.id) return;

        slotsRef.current[slotIdx] = {
          clipId: clip.id,
          blobUrl: url,
          ready: true,
          loading: false,
        };
      } catch (err) {
        setHasError(String(err));
        slotsRef.current[slotIdx].loading = false;
      }
    },
    [loadBlob]
  );

  const preloadIntoInactiveSlot = useCallback(
    async (clip: ForgeClip) => {
      const otherIdx = activeSlotRef.current === 0 ? 1 : 0;
      const other = slotsRef.current[otherIdx];
      if (other.clipId === clip.id && other.ready) return;
      if (other.loading) return;

      await loadClipIntoSlot(otherIdx, clip);

      const vid = otherIdx === 0 ? videoARef.current : videoBRef.current;
      if (!vid) return;

      slotsRef.current[otherIdx].ready = false;
      vid.currentTime = clip.sourceStart;

      await new Promise<void>((resolve) => {
        const onCanPlay = () => {
          vid.removeEventListener("canplay", onCanPlay);
          resolve();
        };
        vid.addEventListener("canplay", onCanPlay);
        setTimeout(() => {
          vid.removeEventListener("canplay", onCanPlay);
          resolve();
        }, 5000);
      });

      slotsRef.current[otherIdx] = {
        ...slotsRef.current[otherIdx],
        ready: true,
      };
    },
    [loadClipIntoSlot]
  );

  const preloadAllClips = useCallback(
    async (vClips: ForgeClip[], aClips: ForgeClip[]) => {
      for (const c of vClips) {
        try {
          await loadBlob(c.sourcePath, videoBlobCacheRef.current);
        } catch {
          /* skip */
        }
      }
      for (const c of aClips) {
        try {
          await loadBlob(c.sourcePath, audioBlobCacheRef.current);
        } catch {
          /* skip */
        }
      }
    },
    [loadBlob]
  );

  const syncAudio = useCallback(
    (timelineTime: number) => {
      const aud = audioRef.current;
      if (!aud) return;

      const activeClip = aClipsRef.current.find(
        (c) => timelineTime >= c.startTime && timelineTime < c.startTime + c.duration
      );

      if (!activeClip) {
        if (!aud.paused) aud.pause();
        audioClipIdRef.current = null;
        audioBlobUrlRef.current = null;
        return;
      }

      if (activeClip.id !== audioClipIdRef.current) {
        const cache = audioBlobCacheRef.current;
        const url = cache.get(activeClip.sourcePath);
        if (url) {
          aud.src = url;
          aud.load();
          audioClipIdRef.current = activeClip.id;
          audioBlobUrlRef.current = url;
        } else {
          loadBlob(activeClip.sourcePath, cache).then((url) => {
            if (audioClipIdRef.current !== activeClip.id) {
              aud.src = url;
              aud.load();
              audioClipIdRef.current = activeClip.id;
              audioBlobUrlRef.current = url;
            }
          });
        }
        return;
      }

      if (!audioBlobUrlRef.current) return;

      const target = timelineTime - activeClip.startTime + activeClip.sourceStart;
      if (Math.abs(aud.currentTime - target) > 0.1) {
        aud.currentTime = target;
      }

      if (playingRef.current && aud.paused) {
        aud.play().catch(() => {});
      } else if (!playingRef.current && !aud.paused) {
        aud.pause();
      }
    },
    [loadBlob]
  );

  const transitionToNextClip = useCallback(() => {
    if (transitioningRef.current) return;
    transitioningRef.current = true;

    const activeClip = getCurrentClip();
    if (!activeClip) { transitioningRef.current = false; return; }

    const clipEnd = activeClip.startTime + activeClip.duration;
    const nextClip = findNextClip(clipEnd - 0.0001);

    if (!nextClip) {
      playingRef.current = false;
      setIsPlaying(false);
      playheadTimeRef.current = clipEnd;
      setPlayhead(clipEnd);
      getActiveVideo()?.pause();
      audioRef.current?.pause();
      transitioningRef.current = false;
      return;
    }

    const otherIdx = activeSlotRef.current === 0 ? 1 : 0;
    const otherSlot = slotsRef.current[otherIdx];

    if (otherSlot.clipId === nextClip.id && otherSlot.ready) {
      activeSlotRef.current = otherIdx as 0 | 1;
      setActiveSlot(activeSlotRef.current);
      const vid = getActiveVideo();
      if (vid) {
        vid.play().catch(() => {
          playingRef.current = false;
          setIsPlaying(false);
        });
      }
      playheadTimeRef.current = nextClip.startTime;
      setPlayhead(nextClip.startTime);
      syncAudio(nextClip.startTime);
      transitioningRef.current = false;
    } else {
      loadClipIntoSlot(activeSlotRef.current, nextClip).then(() => {
        const vid = getActiveVideo();
        if (vid) {
          vid.currentTime = nextClip.sourceStart;
          vid.play().catch(() => {
            playingRef.current = false;
            setIsPlaying(false);
          });
        }
        playheadTimeRef.current = nextClip.startTime;
        setPlayhead(nextClip.startTime);
        syncAudio(nextClip.startTime);
        transitioningRef.current = false;
      });
    }
  }, [
    getActiveVideo,
    getCurrentClip,
    findNextClip,
    loadClipIntoSlot,
    syncAudio,
    setPlayhead,
  ]);

  const handleTimeUpdate = useCallback(() => {
    if (!playingRef.current) return;

    const vid = getActiveVideo();
    const clip = getCurrentClip();
    if (!vid || !clip) {
      playingRef.current = false;
      setIsPlaying(false);
      return;
    }

    const srcTime = vid.currentTime;
    const srcStart = clip.sourceStart;
    const srcEnd = clip.sourceEnd;
    const timelineTime = clip.startTime + (srcTime - srcStart) * clip.speed;
    const clipEnd = clip.startTime + clip.duration;

    playheadTimeRef.current = timelineTime;
    setPlayhead(timelineTime);

    if (clipEnd - timelineTime < 2.0 && clipEnd - timelineTime > 0) {
      const next = findNextClip(timelineTime + 0.1);
      if (next) {
        const otherIdx = activeSlotRef.current === 0 ? 1 : 0;
        const other = slotsRef.current[otherIdx];
        if (other.clipId !== next.id && !other.loading) {
          preloadIntoInactiveSlot(next);
        }
      }
    }

    if (srcTime >= srcEnd - 0.04 || timelineTime >= clipEnd - 0.04) {
      transitionToNextClip();
      return;
    }

    syncAudio(timelineTime);
  }, [
    getActiveVideo,
    getCurrentClip,
    findNextClip,
    preloadIntoInactiveSlot,
    transitionToNextClip,
    syncAudio,
    setPlayhead,
  ]);

  onTimeUpdateRef.current = handleTimeUpdate;

  const clearCache = useCallback(() => {
    const vCache = videoBlobCacheRef.current;
    const aCache = audioBlobCacheRef.current;
    for (const url of vCache.values()) URL.revokeObjectURL(url);
    for (const url of aCache.values()) URL.revokeObjectURL(url);
    vCache.clear();
    aCache.clear();
    slotsRef.current = [emptySlot(), emptySlot()];
    activeSlotRef.current = 0;
    setActiveSlot(0);
    playingRef.current = false;
    setIsPlaying(false);
    audioClipIdRef.current = null;
    audioBlobUrlRef.current = null;
    if (audioRef.current) audioRef.current.src = "";
  }, []);

  const togglePlay = useCallback(() => {
    if (playingRef.current) {
      playingRef.current = false;
      setIsPlaying(false);
      getActiveVideo()?.pause();
      audioRef.current?.pause();
      return;
    }

    const vClips = vClipsRef.current;
    if (vClips.length === 0) return;

    const maxEnd = vClips.reduce(
      (m, c) => Math.max(m, c.startTime + c.duration),
      0
    );
    if (playheadTimeRef.current >= maxEnd) {
      setPlayhead(0);
      playheadTimeRef.current = 0;
    }

    const clip = vClips.find(
      (c) =>
        playheadTimeRef.current >= c.startTime &&
        playheadTimeRef.current < c.startTime + c.duration
    );

    const doPlay = (c: ForgeClip) => {
      const vid = getActiveVideo();
      if (!vid) return;
      const srcTime =
        c.sourceStart +
        (playheadTimeRef.current - c.startTime) / c.speed;
      vid.currentTime = srcTime;
      playingRef.current = true;
      setIsPlaying(true);
      vid.play().catch(() => {
        playingRef.current = false;
        setIsPlaying(false);
      });
      syncAudio(playheadTimeRef.current);
    };

    if (clip) {
      const cur = slotsRef.current[activeSlotRef.current];
      if (cur.clipId === clip.id && cur.ready) {
        doPlay(clip);
      } else {
        loadClipIntoSlot(activeSlotRef.current, clip).then(() => {
          doPlay(clip);
          const next = findNextClip(clip.startTime + clip.duration);
          if (next) preloadIntoInactiveSlot(next);
        });
      }
    } else {
      const next = findNextClip(playheadTimeRef.current);
      if (next) {
        setPlayhead(next.startTime);
        playheadTimeRef.current = next.startTime;
        loadClipIntoSlot(activeSlotRef.current, next).then(() => {
          const vid = getActiveVideo();
          if (!vid) return;
          vid.currentTime = next.sourceStart;
          playingRef.current = true;
          setIsPlaying(true);
          vid.play().catch(() => {
            playingRef.current = false;
            setIsPlaying(false);
          });
          syncAudio(next.startTime);
        });
      }
    }
  }, [
    getActiveVideo,
    findNextClip,
    loadClipIntoSlot,
    preloadIntoInactiveSlot,
    syncAudio,
    setPlayhead,
  ]);

  const seek = useCallback(
    (time: number) => {
      playheadTimeRef.current = time;
      setPlayhead(time);

      const clip = vClipsRef.current.find(
        (c) => time >= c.startTime && time < c.startTime + c.duration
      );
      if (!clip) {
        getActiveVideo()?.pause();
        if (audioRef.current) audioRef.current.pause();
        return;
      }

      const doSeek = (c: ForgeClip) => {
        const vid = getActiveVideo();
        if (!vid) return;
        const srcTime = c.sourceStart + (time - c.startTime) / c.speed;
        vid.currentTime = srcTime;
        syncAudio(time);
      };

      const cur = slotsRef.current[activeSlotRef.current];
      if (cur.clipId === clip.id && cur.ready) {
        doSeek(clip);
      } else {
        loadClipIntoSlot(activeSlotRef.current, clip).then(() => doSeek(clip));
      }
    },
    [getActiveVideo, loadClipIntoSlot, syncAudio, setPlayhead]
  );

  const stepFrame = useCallback(
    (dir: number) => {
      const step = dir / fpsRef.current;
      seek(Math.max(0, playheadTimeRef.current + step));
    },
    [seek]
  );

  useEffect(() => {
    preloadAllClips(project.videoTrack.clips, project.audioTrack.clips);
  }, [project.videoTrack.clips, project.audioTrack.clips, preloadAllClips]);

  useEffect(() => {
    playheadTimeRef.current = playheadTime;

    const clip = vClipsRef.current.find(
      (c) => playheadTime >= c.startTime && playheadTime < c.startTime + c.duration
    );
    if (!clip) return;

    const cur = slotsRef.current[activeSlotRef.current];
    if (cur.clipId !== clip.id) {
      loadClipIntoSlot(activeSlotRef.current, clip);
    } else if (cur.ready) {
      const vid = getActiveVideo();
      if (vid) {
        const srcTime =
          clip.sourceStart + (playheadTime - clip.startTime) / clip.speed;
        if (Math.abs(vid.currentTime - srcTime) > 0.01) {
          vid.currentTime = srcTime;
        }
        syncAudio(playheadTime);
      }
    }

    const next = findNextClip(clip.startTime + clip.duration);
    if (next) preloadIntoInactiveSlot(next);
  }, [
    playheadTime,
    project.videoTrack.clips,
    loadClipIntoSlot,
    preloadIntoInactiveSlot,
    getActiveVideo,
    findNextClip,
    syncAudio,
  ]);

  useEffect(() => {
    return () => clearCache();
  }, [clearCache]);

  useEffect(() => {
    const handler = () => togglePlay();
    window.addEventListener("forge-toggle-play", handler);
    return () => window.removeEventListener("forge-toggle-play", handler);
  }, [togglePlay]);

  const activeVClip = vClipsRef.current.find(
    (c) => playheadTime >= c.startTime && playheadTime < c.startTime + c.duration
  );
  const totalDuration = vClipsRef.current.reduce(
    (max, c) => Math.max(max, c.startTime + c.duration),
    0
  );
  const hasVideo = !!activeVClip;

  const formatTime = (t: number) => {
    const m = Math.floor(t / 60);
    const s = Math.floor(t % 60);
    const ms = Math.floor((t % 1) * 10);
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${ms}`;
  };

  return (
    <div className="forge-preview-inner">
      <div className="forge-preview-canvas">
        {hasVideo ? (
          <div className="forge-preview-video-wrapper">
            <div className="forge-preview-video-slots">
              <audio ref={audioRef} preload="auto" playsInline />
              <video
                ref={videoARef}
                muted
                className={`forge-preview-video-slot${activeSlot !== 0 ? " hidden" : ""}`}
                onTimeUpdate={() => { if (activeSlotRef.current === 0) onTimeUpdateRef.current(); }}
                onError={() => setHasError("Video error")}
                playsInline
              />
              <video
                ref={videoBRef}
                muted
                className={`forge-preview-video-slot${activeSlot !== 1 ? " hidden" : ""}`}
                onTimeUpdate={() => { if (activeSlotRef.current === 1) onTimeUpdateRef.current(); }}
                onError={() => setHasError("Video error")}
                playsInline
              />
            </div>
          </div>
        ) : hasError ? (
          <div className="forge-preview-placeholder">
            <div className="forge-preview-placeholder-frame error">
              <div className="forge-preview-placeholder-inner">
                <span className="forge-preview-placeholder-rune">ᚲ</span>
                <span className="forge-preview-placeholder-text">{hasError}</span>
              </div>
            </div>
          </div>
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

      {hasVideo && (
        <div className="forge-transport">
          <button
            className="forge-transport-btn"
            onClick={() => stepFrame(-1)}
            title="Frame back"
          >
            ⏮
          </button>
          <button
            className="forge-transport-btn forge-transport-play"
            onClick={togglePlay}
          >
            {isPlaying ? "■" : "▶"}
          </button>
          <button
            className="forge-transport-btn"
            onClick={() => stepFrame(1)}
            title="Frame forward"
          >
            ⏭
          </button>
          <span className="forge-transport-time">
            {formatTime(playheadTime)}
          </span>
          <span className="forge-transport-sep">/</span>
          <span className="forge-transport-time dim">
            {formatTime(totalDuration)}
          </span>
          <input
            type="range"
            className="forge-transport-scrub"
            min={0}
            max={totalDuration || 0}
            step={1 / (project.fps || 30)}
            value={playheadTime}
            onChange={(e) => seek(parseFloat(e.target.value))}
          />
        </div>
      )}
    </div>
  );
}