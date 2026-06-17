import { useForgeStore, beginDrag } from "../../store/forgeStore";

export default function SourceBrowser() {
  const mediaLibrary = useForgeStore((s) => s.mediaLibrary);
  const importMediaFiles = useForgeStore((s) => s.importMediaFiles);
  const removeFromLibrary = useForgeStore((s) => s.removeFromLibrary);

  const formatDur = (s: number) => {
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    return `${m}:${String(sec).padStart(2, "0")}`;
  };

  const handlePointerDown = (e: React.PointerEvent, item: typeof mediaLibrary[0]) => {
    e.preventDefault();
    beginDrag({ id: item.id, path: item.path, duration: item.duration, name: item.name });
  };

  return (
    <div className="forge-source">
      <div className="forge-panel-header">
        <span className="forge-panel-title">ᚨ source</span>
        <button className="forge-source-import-btn" onClick={importMediaFiles}>
          + import
        </button>
      </div>
      <div className="forge-source-list">
        {mediaLibrary.length === 0 && (
          <div className="forge-source-empty">
            <span className="forge-source-empty-text">no media imported</span>
            <span className="forge-source-empty-hint">click + import or drag files in</span>
          </div>
        )}
        {mediaLibrary.map((item) => (
          <div
            key={item.id}
            className="forge-source-item"
            onPointerDown={(e) => handlePointerDown(e, item)}
          >
            <div className="forge-source-item-info">
              <span className="forge-source-item-name">{item.name}</span>
              <span className="forge-source-item-meta">
                {formatDur(item.duration)}
                {item.width && item.height ? `  ${item.width}x${item.height}` : ""}
              </span>
            </div>
            <button
              className="forge-source-item-remove"
              onClick={() => removeFromLibrary(item.id)}
              title="Remove from library"
            >
              ×
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}