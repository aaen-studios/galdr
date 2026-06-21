import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence } from "framer-motion";
import Dropdown from "./Dropdown";
import RuneTagEditor from "./RuneTagEditor";
import { useGaldrStore } from "../store";
import { useContextMenu } from "./ContextMenu";
import { conversionToPreset, summarizePreset } from "../utils/runeMerge";
import type { DropdownOption } from "./Dropdown";
import type { ConversionParams, PresetParams, RuneTag } from "../types";

interface Props {
  /**
   * Snapshot of the page's current conversion state, used to prefill the
   * "save as rune" editor. Callers pass a full ConversionParams (paths
   * included); they're stripped before persisting.
   */
  currentParams: ConversionParams;
  /**
   * Called with a loaded rune's preset so the page can merge it into its own
   * state with whatever semantics it needs.
   */
  onApply: (preset: PresetParams) => void;
  /** Optional: navigate to the runes page when "manage runes" is clicked. */
  onManage?: () => void;
}

const NONE = "__none__";

export default function PresetPicker({ currentParams, onApply, onManage }: Props) {
  const runeTags = useGaldrStore((s) => s.runeTags);
  const refreshRuneTags = useGaldrStore((s) => s.refreshRuneTags);
  const [editing, setEditing] = useState<RuneTag | null>(null);
  const [creating, setCreating] = useState(false);
  const { show } = useContextMenu();

  const options: DropdownOption[] = [
    { value: NONE, label: runeTags.length === 0 ? "no runes saved" : "apply a rune…", type: "preset" },
    ...runeTags.map((t) => ({
      value: t.id,
      label: `${t.rune} ${t.name}`,
      type: "preset",
      category: "runes",
    })),
  ];

  const applyRune = async (id: string) => {
    if (id === NONE) return;
    try {
      const preset = await invoke<PresetParams>("apply_rune_tag", { id });
      onApply(preset);
    } catch (e) {
      console.error("Failed to apply rune tag", e);
    }
  };

  const handleSaveFromEditor = async (tag: RuneTag) => {
    try {
      await invoke<RuneTag>("save_rune_tag", { tag });
      await refreshRuneTags();
    } catch (e) {
      console.error("Failed to save rune tag", e);
    }
    setEditing(null);
    setCreating(false);
  };

  const openSaveAsRune = () => {
    // Prefill the editor with the page's current settings (minus paths).
    const preset = conversionToPreset(currentParams);
    setEditing({
      id: "",
      name: "",
      rune: "ᚠ",
      description: "",
      params: preset,
    });
    setCreating(true);
  };

  const handleContext = (e: React.MouseEvent) => {
    e.stopPropagation();
    const items = [
      { label: "save current as rune", rune: "ᚠ", action: openSaveAsRune },
    ];
    if (onManage) {
      items.push({ label: "manage runes", rune: "ᛏ", action: onManage });
    }
    show(e, items);
  };

  // Short preview of what the page's current settings look like as a preset.
  const currentPreview = summarizePreset(conversionToPreset(currentParams));

  return (
    <div className="preset-picker" onContextMenu={handleContext}>
      <div className="preset-picker-glyph">ᚠ</div>
      <div className="preset-picker-fields">
        <span className="preset-picker-label">rune preset</span>
        <Dropdown
          options={options}
          value={NONE}
          onChange={(v) => applyRune(v)}
          showCategories
          placeholder="apply a rune…"
        />
      </div>
      <div className="preset-picker-actions">
        <span className="preset-picker-current" title={currentPreview}>{currentPreview}</span>
        <button
          className="btn preset-save-btn"
          onClick={openSaveAsRune}
          title="save current settings as a rune"
        >
          + save as rune
        </button>
      </div>

      <AnimatePresence>
        {(creating || editing) && (
          <RuneTagEditor
            tag={editing ?? undefined}
            onSave={handleSaveFromEditor}
            onCancel={() => {
              setEditing(null);
              setCreating(false);
            }}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
