import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import ScrambleText from "../components/ScrambleText";
import RuneTagEditor from "../components/RuneTagEditor";
import { useContextMenu } from "../components/ContextMenu";
import type { RuneTag } from "../types";

function paramsSummary(p: RuneTag["params"]): string {
  const parts: string[] = [p.output_format.toUpperCase()];
  if (p.video_codec) parts.push(p.video_codec);
  if (p.crf !== undefined) parts.push(`CRF ${p.crf}`);
  if (p.audio_codec) parts.push(p.audio_codec);
  if (p.audio_bitrate) parts.push(p.audio_bitrate);
  if (p.resolution) parts.push(`${p.resolution[0]}x${p.resolution[1]}`);
  return parts.join(" · ");
}

export default function RunesPage() {
  const [tags, setTags] = useState<RuneTag[]>([]);
  const [editing, setEditing] = useState<RuneTag | undefined>(undefined);
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const { show } = useContextMenu();

  const loadTags = useCallback(async () => {
    try {
      const result = await invoke<RuneTag[]>("list_rune_tags");
      setTags(result);
    } catch {
      console.error("Failed to load rune tags");
    }
  }, []);

  useEffect(() => {
    loadTags();
  }, [loadTags]);

  const handleSave = async (tag: RuneTag) => {
    try {
      await invoke<RuneTag>("save_rune_tag", { tag });
      await loadTags();
      setEditing(undefined);
      setCreating(false);
    } catch (e) {
      console.error("Failed to save rune tag", e);
    }
  };

  const handleDelete = async (id: string) => {
    setDeleting(id);
    try {
      await invoke("delete_rune_tag", { id });
      await loadTags();
    } catch (e) {
      console.error("Failed to delete rune tag", e);
    }
    setDeleting(null);
  };

  const handleCardContext = useCallback((e: React.MouseEvent, tag: RuneTag) => {
    e.stopPropagation();
    show(e, [
      { label: "edit", rune: "ᛏ", action: () => setEditing(tag) },
      { label: "duplicate", rune: "ᚷ", action: async () => {
        const dup: RuneTag = { ...tag, id: crypto.randomUUID(), name: `${tag.name} (copy)` };
        await invoke("save_rune_tag", { tag: dup });
        await loadTags();
      }},
      { label: "", rune: "", action: () => {}, divider: true },
      { label: "delete", rune: "ᚨ", action: () => handleDelete(tag.id) },
    ]);
  }, [show, loadTags]);

  const handleEmptyContext = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    show(e, [
      { label: "new preset", rune: "ᚨ", action: () => setCreating(true) },
    ]);
  }, [show]);

  return (
    <div className="page runes-page">
      <ScrambleText as="h1" className="page-heading" text="ᚠ rune tags" hover load />

      <div className="rune-grid">
        <motion.button
          className="rune-card rune-card-new"
          whileHover={{ scale: 1.02 }}
          whileTap={{ scale: 0.98 }}
          onClick={() => setCreating(true)}
          onContextMenu={(e) => {
            e.stopPropagation();
            show(e, [
              { label: "new preset", rune: "ᚨ", action: () => setCreating(true) },
            ]);
          }}
        >
          <span className="rune-card-rune new">+</span>
          <span className="rune-card-name">new preset</span>
          <span className="rune-card-desc">create a saved conversion preset</span>
        </motion.button>

        {tags.map((tag) => (
          <motion.div
            key={tag.id}
            className="rune-card"
            layout
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95 }}
            whileHover={{ scale: 1.02 }}
            onContextMenu={(e) => handleCardContext(e, tag)}
          >
            <div className="rune-card-main" onClick={() => setEditing(tag)}>
              <span className="rune-card-rune">{tag.rune}</span>
              <span className="rune-card-name">{tag.name}</span>
              <span className="rune-card-desc">{tag.description}</span>
              <span className="rune-card-params">{paramsSummary(tag.params)}</span>
            </div>
            <button
              className="rune-card-delete"
              onClick={(e) => {
                e.stopPropagation();
                handleDelete(tag.id);
              }}
              disabled={deleting === tag.id}
            >
              {deleting === tag.id ? "..." : "x"}
            </button>
          </motion.div>
        ))}
      </div>

      {tags.length === 0 && !creating && (
        <div className="rune-empty" onContextMenu={handleEmptyContext}>
          <span className="rune-empty-icon">ᚱ</span>
          <span className="rune-empty-text">no rune tags yet. create one to get started.</span>
        </div>
      )}

      <AnimatePresence>
        {(creating || editing) && (
          <RuneTagEditor
            tag={editing}
            onSave={handleSave}
            onCancel={() => {
              setEditing(undefined);
              setCreating(false);
            }}
          />
        )}
      </AnimatePresence>
    </div>
  );
}