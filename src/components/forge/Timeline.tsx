import { useRef, useCallback, useState } from "react";
import { useForgeStore } from "../../store/forgeStore";

const PADDING = 16;
const HANDLE_WIDTH = 6;
const TRAILING_SPACE = 400;

export default function Timeline() {
  const project = useForgeStore((s) => s.project);
  const setPlayhead = useForgeStore((s) => s.setPlayhead);
  const setZoom = useForgeStore((s) => s.setZoom);
  const moveClip = useForgeStore((s) => s.moveClip);
  const trimClip = useForgeStore((s) => s.trimClip);
  const selectClip = useForgeStore((s) => s.selectClip);
  const splitClipAtPlayhead = useForgeStore((s) => s.splitClipAtPlayhead);
  const snapEnabled = useForgeStore((s) => s.snapEnabled);
  const setSnapEnabled = useForgeStore((s) => s.setSnapEnabled);
  const undo = useForgeStore((s) => s.undo);
  const redo = useForgeStore((s) => s.redo);

  const scrollRef = useRef<HTMLDivElement>(null);
  const rulerRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<{
    type: "playhead" | "clip" | "trim-start" | "trim-end";
    clipId?: string;
    track?: "video" | "audio";
    startX?: number;
    origVal?: number;
    origSourceStart?: number;
    origSourceEnd?: number;
  } | null>(null);

  const zoom = project.zoomLevel;
  const clipsEnd = Math.max(
    0,
    ...project.videoTrack.clips.map((c) => c.startTime + c.duration),
    ...project.audioTrack.clips.map((c) => c.startTime + c.duration)
  );
  const totalDur = Math.max(10, clipsEnd + TRAILING_SPACE / zoom);

  // Labels are now in a fixed column outside the scrollable area.
  // timeToX starts at 0 from the content edge (after the fixed label column + padding).
  const timeToX = (t: number) => t * zoom + PADDING;
  const xToTime = (x: number) => (x - PADDING) / zoom;

  const snapTime = useCallback(
    (t: number) => {
      if (!snapEnabled) return t;
      const snaps = [0, project.playheadTime];
      for (const c of project.videoTrack.clips) {
        snaps.push(c.startTime, c.startTime + c.duration);
      }
      for (const c of project.audioTrack.clips) {
        snaps.push(c.startTime, c.startTime + c.duration);
      }
      for (const m of project.markers) snaps.push(m.time);
      const threshold = 5 / zoom;
      for (const s of snaps) {
        if (Math.abs(t - s) < threshold) return s;
      }
      return t;
    },
    [snapEnabled, project, zoom]
  );

  const handleRulerMouseDown = useCallback(
    (e: React.MouseEvent) => {
      const rect = rulerRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left + (scrollRef.current?.scrollLeft || 0);
      setPlayhead(Math.max(0, xToTime(x)));
      setDragging({ type: "playhead", startX: e.clientX, origVal: project.playheadTime });
    },
    [project.playheadTime, setPlayhead]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!dragging) return;
      if (dragging.type === "playhead") {
        const rect = rulerRef.current?.getBoundingClientRect();
        if (!rect) return;
        const x = e.clientX - rect.left + (scrollRef.current?.scrollLeft || 0);
        setPlayhead(Math.max(0, snapTime(xToTime(x))));
      } else {
        const dx = (e.clientX - dragging.startX!) / zoom;
        if (dragging.type === "clip" && dragging.clipId && dragging.track) {
          const newStart = snapTime(Math.max(0, dragging.origVal! + dx));
          moveClip(dragging.clipId, newStart, dragging.track);
        } else if (dragging.type === "trim-start" && dragging.clipId && dragging.track) {
          const trackData = dragging.track === "video" ? project.videoTrack : project.audioTrack;
          const clip = trackData.clips.find((c) => c.id === dragging.clipId);
          if (!clip) return;
          const dt = (e.clientX - dragging.startX!) / zoom;
          const newSourceStart = Math.max(0, dragging.origSourceStart! + dt * clip.speed);
          if (newSourceStart < clip.sourceEnd - 0.05) {
            trimClip(dragging.clipId, newSourceStart, clip.sourceEnd, dragging.track);
          }
        } else if (dragging.type === "trim-end" && dragging.clipId && dragging.track) {
          const trackData = dragging.track === "video" ? project.videoTrack : project.audioTrack;
          const clip = trackData.clips.find((c) => c.id === dragging.clipId);
          if (!clip) return;
          const dt = (e.clientX - dragging.startX!) / zoom;
          const newSourceEnd = Math.max(clip.sourceStart + 0.05, dragging.origSourceEnd! + dt * clip.speed);
          trimClip(dragging.clipId, clip.sourceStart, newSourceEnd, dragging.track);
        }
      }
    },
    [dragging, zoom, snapTime, setPlayhead, moveClip, trimClip, project]
  );

  const handleMouseUp = useCallback(() => {
    setDragging(null);
  }, []);

  const handleClipMouseDown = useCallback(
    (e: React.MouseEvent, clipId: string, track: "video" | "audio") => {
      e.stopPropagation();
      selectClip(clipId, track);
      const target = e.target as HTMLElement;
      const isTrimStart = target.dataset.trim === "start";
      const isTrimEnd = target.dataset.trim === "end";
      const rect = rulerRef.current?.getBoundingClientRect();
      if (!rect) return;

      const trackData = track === "video" ? project.videoTrack : project.audioTrack;
      const clip = trackData.clips.find((c) => c.id === clipId);
      if (!clip) return;

      if (isTrimStart) {
        setDragging({
          type: "trim-start",
          clipId,
          track,
          startX: e.clientX,
          origSourceStart: clip.sourceStart,
        });
      } else if (isTrimEnd) {
        setDragging({
          type: "trim-end",
          clipId,
          track,
          startX: e.clientX,
          origSourceEnd: clip.sourceEnd,
        });
      } else {
        setDragging({
          type: "clip",
          clipId,
          track,
          startX: e.clientX,
          origVal: clip.startTime,
        });
      }
    },
    [selectClip, project]
  );

  const renderRuler = () => {
    const marks: React.ReactNode[] = [];
    const majorInterval = zoom < 50 ? 10 : zoom < 150 ? 5 : zoom < 300 ? 2 : 1;
    const minorInterval = zoom < 50 ? 2 : zoom < 150 ? 1 : zoom < 300 ? 0.5 : 0.2;
    let first = true;
    for (let t = 0; t <= totalDur; t += minorInterval) {
      const x = timeToX(t);
      const isMajor = t % majorInterval === 0;
      if (first) {
        marks.push(
          <div key={t} className="forge-ruler-mark major" style={{ left: x }}>
            <div className="forge-ruler-tick" style={{ height: 12 }} />
            <span className="forge-ruler-label">{t}s</span>
          </div>
        );
        first = false;
      } else {
        marks.push(
          <div key={t} className={`forge-ruler-mark${isMajor ? "" : " minor"}`} style={{ left: x }}>
            <div className="forge-ruler-tick" style={{ height: isMajor ? 12 : 5 }} />
            {isMajor && <span className="forge-ruler-label">{t}s</span>}
          </div>
        );
      }
    }
    return marks;
  };

  const renderClips = (track: "video" | "audio") => {
    const data = track === "video" ? project.videoTrack : project.audioTrack;
    return data.clips.map((clip) => {
      const x = timeToX(clip.startTime);
      const w = clip.duration * zoom;
      const durRatio = clip.sourceEnd - clip.sourceStart > 0
        ? clip.duration / (clip.sourceEnd - clip.sourceStart)
        : 1;

      return (
        <div
          key={clip.id}
          className={`forge-clip${clip.selected ? " selected" : ""}`}
          style={{ left: x, width: w }}
          onMouseDown={(e) => handleClipMouseDown(e, clip.id, track)}
        >
          <div
            className="forge-clip-handle left"
            data-trim="start"
            style={{ width: HANDLE_WIDTH, left: 0 }}
          />
          <div className="forge-clip-body">
            <span className="forge-clip-name">{clip.name}</span>
            {durRatio !== 1 && (
              <span className="forge-clip-speed">
                {durRatio > 1 ? `+${Math.round((durRatio - 1) * 100)}%` : `${Math.round(durRatio * 100)}%`}
              </span>
            )}
          </div>
          <div
            className="forge-clip-handle right"
            data-trim="end"
            style={{ width: HANDLE_WIDTH, right: 0 }}
          />
        </div>
      );
    });
  };

  const renderPlayhead = () => {
    const x = timeToX(project.playheadTime);
    return (
      <div className="forge-playhead" style={{ left: x }}>
        <div className="forge-playhead-head" />
        <div className="forge-playhead-line" />
      </div>
    );
  };

  const renderMarkers = () =>
    project.markers.map((m) => (
      <div key={m.time} className="forge-marker" style={{ left: timeToX(m.time) }}>
        <div className="forge-marker-diamond" />
        {m.label && <span className="forge-marker-label">{m.label}</span>}
      </div>
    ));

  const trackH = (h: number) => `${h}px`;

  return (
    <div
      className="forge-timeline"
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
    >
      <div className="forge-timeline-toolbar">
        <button className="forge-tl-btn" onClick={() => setZoom(zoom - 20)} title="Zoom out">
          ◁ zoom
        </button>
        <span className="forge-tl-zoom-label">{zoom} px/s</span>
        <button className="forge-tl-btn" onClick={() => setZoom(zoom + 20)} title="Zoom in">
          zoom ▷
        </button>
        <div className="forge-tl-spacer" />
        <button
          className={`forge-tl-btn${snapEnabled ? " active" : ""}`}
          onClick={() => setSnapEnabled(!snapEnabled)}
          title="Toggle snap"
        >
          ᛏ snap
        </button>
        <button className="forge-tl-btn" onClick={splitClipAtPlayhead} title="Split at playhead (S)">
          ✂ split
        </button>
        <button className="forge-tl-btn" onClick={undo} title="Undo (Ctrl+Z)">
          ↩ undo
        </button>
        <button className="forge-tl-btn" onClick={redo} title="Redo (Ctrl+Shift+Z)">
          ↪ redo
        </button>
      </div>

      <div className="forge-timeline-workspace">
        <div className="forge-timeline-labels">
          <div className="forge-timeline-labels-spacer" />
          <div className="forge-track-label">V1</div>
          <div className="forge-track-label">A1</div>
        </div>

        <div className="forge-timeline-body" ref={scrollRef}>
          <div className="forge-timeline-inner">
            <div className="forge-ruler" ref={rulerRef} onMouseDown={handleRulerMouseDown}>
              {renderRuler()}
              {renderMarkers()}
              {renderPlayhead()}
            </div>

            <div
              className="forge-track"
              data-track="video"
              style={{ height: trackH(project.videoTrack.height) }}
            >
              <div className="forge-track-clips">
                {renderClips("video")}
              </div>
            </div>

            <div
              className="forge-track audio"
              data-track="audio"
              style={{ height: trackH(project.audioTrack.height) }}
            >
              <div className="forge-track-clips">
                {renderClips("audio")}
              </div>
            </div>

            {renderPlayhead()}
          </div>
        </div>
      </div>
    </div>
  );
}