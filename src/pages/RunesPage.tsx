import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import ScrambleText from "../components/ScrambleText";
import RuneTagEditor from "../components/RuneTagEditor";
import { useContextMenu } from "../components/ContextMenu";
import { useGaldrStore } from "../store";
import { summarizePreset, presetType, presetTypeLabel } from "../utils/runeMerge";
import type { RuneTag } from "../types";

/** Seeded starter runes use ids like "starter-1"; user runes use UUIDs. */
function isStarter(tag: RuneTag): boolean {
  return tag.id.startsWith("starter-");
}

export default function RunesPage() {
  const tags = useGaldrStore((s) => s.runeTags);
  const refreshRuneTags = useGaldrStore((s) => s.refreshRuneTags);
  const [editing, setEditing] = useState<RuneTag | undefined>(undefined);
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const { show } = useContextMenu();

  // Keep in sync with the shared store: refresh whenever the page mounts.
  useEffect(() => {
    refreshRuneTags();
  }, [refreshRuneTags]);

  const handleSave = async (tag: RuneTag) => {
    try {
      await invoke<RuneTag>("save_rune_tag", { tag });
      await refreshRuneTags();
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
      await refreshRuneTags();
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
        await refreshRuneTags();
      }},
      { label: "", rune: "", action: () => {}, divider: true },
      { label: "delete", rune: "ᚨ", action: () => handleDelete(tag.id) },
    ]);
  }, [show, refreshRuneTags]);

  const handleEmptyContext = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    show(e, [
      { label: "new preset", rune: "ᚨ", action: () => setCreating(true) },
    ]);
  }, [show]);

  const handleHeaderContext = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    show(e, [
      { label: "new preset", rune: "ᚨ", action: () => setCreating(true) },
    ]);
  }, [show]);

  const starters = tags.filter(isStarter);
  const userRunes = tags.filter((t) => !isStarter(t));

  const renderCard = (tag: RuneTag) => {
    const type = presetType(tag.params);
    return (
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
          <div className="rune-card-top">
            <span className="rune-card-rune">{tag.rune}</span>
            <span className={`rune-card-badge badge-${type}`}>{presetTypeLabel(type)}</span>
          </div>
          <span className="rune-card-name">{tag.name}</span>
          <span className="rune-card-desc">{tag.description}</span>
          <span className="rune-card-params">{summarizePreset(tag.params)}</span>
        </div>
        <button
          className="rune-card-delete"
          onClick={(e) => {
            e.stopPropagation();
            handleDelete(tag.id);
          }}
          disabled={deleting === tag.id}
          title="delete"
        >
          {deleting === tag.id ? "..." : "x"}
        </button>
      </motion.div>
    );
  };

  return (
    <div className="page runes-page">
      <header className="runes-header" onContextMenu={handleHeaderContext}>
        <ScrambleText as="h1" className="page-heading" text="ᚠ rune tags" hover load />
        <p className="runes-subtitle">
          save conversion settings as named runes, then apply them anywhere with one click —
          convert, compress, and batch. {tags.length > 0 && (
            <span className="runes-count">{tags.length} saved</span>
          )}
        </p>
      </header>

      <div className="rune-grid-actions">
        <motion.button
          className="rune-card rune-card-new"
          whileHover={{ scale: 1.02 }}
          whileTap={{ scale: 0.98 }}
          onClick={() => setCreating(true)}
          onContextMenu={(e) => {
            e.stopPropagation();
            show(e, [{ label: "new preset", rune: "ᚨ", action: () => setCreating(true) }]);
          }}
        >
          <span className="rune-card-rune new">+</span>
          <span className="rune-card-name">new preset</span>
          <span className="rune-card-desc">capture current settings as a reusable rune</span>
        </motion.button>
      </div>

      {userRunes.length > 0 && (
        <section className="rune-section">
          <h2 className="rune-section-title">your runes</h2>
          <div className="rune-grid">
            <AnimatePresence>
              {userRunes.map(renderCard)}
            </AnimatePresence>
          </div>
        </section>
      )}

      {starters.length > 0 && (
        <section className="rune-section">
          <h2 className="rune-section-title">
            starter runes
            <span className="rune-section-hint">examples — edit, duplicate, or delete any of them</span>
          </h2>
          <div className="rune-grid">
            <AnimatePresence>
              {starters.map(renderCard)}
            </AnimatePresence>
          </div>
        </section>
      )}

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
