import { useEffect, useState, useCallback, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getVersion } from "@tauri-apps/api/app";
import { enable, disable } from "@tauri-apps/plugin-autostart";
import { useGaldrStore } from "../store";
import Dropdown from "../components/Dropdown";
import { TRANSITION_OPTIONS } from "../transitions";
import { useContextMenu } from "../components/ContextMenu";
import { resolvePreferredEncoder } from "../utils/ffmpegBuilder";
import { getEncoderTier, TIER_RANK } from "../utils/encoderTiers";

const SUBFOLDERS = ["video", "audio", "image"];

/** Theme palettes — names mirror the [data-theme] CSS blocks in App.css. */
const THEMES: { value: string; label: string; swatch: string }[] = [
  { value: "void", label: "void", swatch: "#000000" },
  { value: "ember", label: "ember", swatch: "#1a1100" },
  { value: "frost", label: "frost", swatch: "#00111a" },
  { value: "rune", label: "rune", swatch: "#15001a" },
  { value: "bone", label: "bone", swatch: "#e8e6e0" },
];

/** Human-readable vendor label. */
const VENDOR_LABEL: Record<string, string> = {
  nvidia: "NVIDIA",
  amd: "AMD",
  intel: "Intel",
  apple: "Apple",
  vaapi: "VAAPI",
};

interface Props {
  onNavigate: (page: "watch") => void;
}

export default function SettingsPage({ onNavigate }: Props) {
  const {
    outputDir, setOutputDir,
    transitionStyle, setTransitionStyle,
    triggerTransitionTest,
    setUpdateDismissed,
    discordEnabled, setDiscordEnabled,
    autostartEnabled, setAutostartEnabled,
    availableEncoders,
    preferredVideoEncoder, setPreferredVideoEncoder,
    autoFallbackHw, setAutoFallbackHw,
    downloadDir, setDownloadDir,
    showRuneInTitlebar, setShowRuneInTitlebar,
    autoDownloadSubtitles, setAutoDownloadSubtitles,
    autoEmbedSubtitles, setAutoEmbedSubtitles,
    theme, setTheme,
    crtEnabled, setCrtEnabled,
    loadHardwareEncoders,
  } = useGaldrStore();
  const [version, setVersion] = useState("");
  const [subsOpen, setSubsOpen] = useState(false);
  const [encodersLoading, setEncodersLoading] = useState(availableEncoders.length === 0);
  const { show } = useContextMenu();

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion("0.1.0"));
  }, []);

  // Load hardware encoders if the store doesn't have them yet (the app
  // startup call in App.tsx may still be in-flight, or the Rust cache is
  // already populated). Never freeze — the skeleton state keeps the page
  // interactive while detection runs.
  useEffect(() => {
    let cancelled = false;
    if (availableEncoders.length === 0 && encodersLoading) {
      loadHardwareEncoders().finally(() => {
        if (!cancelled) setEncodersLoading(false);
      });
    } else {
      setEncodersLoading(false);
    }
    return () => { cancelled = true; };
  }, [availableEncoders, encodersLoading, loadHardwareEncoders]);

  // Build categorized dropdown options from detected encoders.
  // The "auto" option's label dynamically shows which encoder it resolves to.
  const encoderDropdownOptions = useMemo(() => {
    const base: { value: string; label: string; category?: string }[] = [];

    // Resolve what "auto" would pick, so we can show it in the trigger label
    const autoName = resolvePreferredEncoder("auto", "mp4", availableEncoders);
    const autoEnc = autoName
      ? availableEncoders.find((e) => e.name === autoName)
      : undefined;
    base.push({
      value: "auto",
      label: autoEnc
        ? `Auto → ${autoEnc.name} (${autoEnc.description})`
        : "Auto (prefer hardware, fallback to software)",
    });
    base.push({
      value: "software",
      label: "Software encoding only",
    });

    if (availableEncoders.length > 0) {
      const sorted = [...availableEncoders].sort((a, b) => {
        const aRank = TIER_RANK[getEncoderTier(a.vendor).tier] ?? 0;
        const bRank = TIER_RANK[getEncoderTier(b.vendor).tier] ?? 0;
        return bRank - aRank;
      });
      sorted.forEach((enc) => {
        const vendor = VENDOR_LABEL[enc.vendor] ?? enc.vendor;
        base.push({
          value: enc.name,
          category: vendor,
          label: `${enc.name}  ${enc.description}`,
        });
      });
    }
    return base;
  }, [availableEncoders]);

  const toggleDiscord = useCallback(() => {
    const next = !discordEnabled;
    setDiscordEnabled(next);
    invoke("set_discord_enabled", { enabled: next }).catch(() => { });
  }, [discordEnabled, setDiscordEnabled]);

  const toggleAutostart = useCallback(() => {
    const next = !autostartEnabled;
    setAutostartEnabled(next);
    if (next) enable().catch(() => setAutostartEnabled(false));
    else disable().catch(() => setAutostartEnabled(true));
  }, [autostartEnabled, setAutostartEnabled]);

  const pickFolder = useCallback(async () => {
    const sel = await open({ directory: true, multiple: false });
    if (sel) setOutputDir(sel as string);
  }, [setOutputDir]);

  const pickDownloadFolder = useCallback(async () => {
    const sel = await open({ directory: true, multiple: false });
    if (sel) setDownloadDir(sel as string);
  }, [setDownloadDir]);

  const copyToClipboard = useCallback((text: string) => {
    navigator.clipboard.writeText(text).catch(() => { });
  }, []);

  const subPath = useCallback((sub: string) => `${outputDir}/${sub}/`, [outputDir]);

  return (
    <div className="page">
      <h2>ᚲ settings</h2>

      <div className="card" onContextMenu={(e) => show(e, [
        { label: "browse", rune: "ᚨ", action: pickFolder },
        { label: "copy path", rune: "ᚷ", action: () => copyToClipboard(outputDir) },
        ...(outputDir ? [{ label: "clear", rune: "ᛏ", action: () => setOutputDir("") }] : []),
      ])}>
        <label className="label">base output folder</label>
        <div className="row">
          <input className="input" value={outputDir} placeholder="not set — will prompt on convert" readOnly />
          <button className="btn" onClick={pickFolder}>browse</button>
        </div>
        {outputDir && (
          <>
            <button
              className="settings-disclosure"
              onClick={() => setSubsOpen((o) => !o)}
              aria-expanded={subsOpen}
            >
              <span>{subsOpen ? "▾" : "▸"}</span>
              <span>auto-created subfolders</span>
              <span className="settings-disclosure-count">{SUBFOLDERS.length}</span>
            </button>
            <AnimatePresence initial={false}>
              {subsOpen && (
                <motion.div
                  className="settings-subs"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18 }}
                >
                  {SUBFOLDERS.map((sf) => (
                    <div
                      key={sf}
                      className="settings-sub"
                      onContextMenu={(e) => show(e, [
                        { label: "copy path", rune: "ᚷ", action: () => copyToClipboard(subPath(sf)) },
                        { label: "open in explorer", rune: "ᛏ", action: () => invoke("reveal_in_folder", { path: subPath(sf) }).catch(() => { }) },
                      ])}
                    >
                      <span className="settings-sub-name">{sf}/</span>
                      <span className="settings-sub-path">{subPath(sf)}</span>
                    </div>
                  ))}
                </motion.div>
              )}
            </AnimatePresence>
          </>
        )}
      </div>

      <div className="card">
        <label className="label">background &amp; watch folders</label>
        <p className="settings-hint flush">
          watch folders convert incoming files automatically, even when the window is
          closed to the tray. configure them in{" "}
          <span className="nav-path-link" onClick={() => onNavigate("watch")}>
            ~/galdr/watch
          </span>.
        </p>
        <div className="row settings-toggle-row">
          <label className="toggle-label">launch at login</label>
          <button
            className={`btn toggle-btn${autostartEnabled ? " active" : ""}`}
            onClick={toggleAutostart}
          >
            {autostartEnabled ? "on" : "off"}
          </button>
        </div>
      </div>

      <div className="card" onContextMenu={(e) => show(e, [
        { label: "check for updates", rune: "ᚠ", action: () => setUpdateDismissed(false) },
      ])}>
        <label className="label">updates</label>
        <div className="row">
          <p className="settings-hint flush">
            v{version || "..."} &mdash; checks GitHub on startup
          </p>
          <button className="btn" onClick={() => setUpdateDismissed(false)}>
            ᚠ check
          </button>
        </div>
      </div>

      <div className="card" onContextMenu={(e) => show(e, [
        { label: "test transition", rune: "ᛟ", action: triggerTransitionTest },
        { label: "reset to default", rune: "ᛏ", action: () => setTransitionStyle("none") },
      ])}>
        <label className="label">page transition</label>
        <div className="row">
          <div className="row-grow">
            <Dropdown
              options={TRANSITION_OPTIONS}
              value={transitionStyle}
              onChange={(v) => setTransitionStyle(v as typeof transitionStyle)}
            />
          </div>
          <button
            className="btn"
            disabled={transitionStyle === "none"}
            onClick={triggerTransitionTest}
          >
            ᛟ test
          </button>
        </div>
        <div className="row settings-toggle-row" style={{ marginTop: 12 }}>
          <label className="toggle-label">show rune cards on home</label>
          <button
            className={`btn toggle-btn${showRuneInTitlebar ? " active" : ""}`}
            onClick={() => setShowRuneInTitlebar(!showRuneInTitlebar)}
          >
            {showRuneInTitlebar ? "on" : "off"}
          </button>
        </div>
        <p className="settings-hint">
          when off, the home page hides the runic tool cards for a minimal view
        </p>
      </div>

      {/* ── Appearance: theme + CRT ── */}
      <div className="card">
        <label className="label">ᚦ appearance</label>
        <div className="theme-swatches">
          {THEMES.map((t) => (
            <button
              key={t.value}
              className={`theme-swatch${theme === t.value ? " active" : ""}`}
              onClick={() => setTheme(t.value)}
              onContextMenu={(e) => show(e, [
                { label: `use ${t.label}`, rune: "ᚨ", action: () => setTheme(t.value) },
              ])}
            >
              <span className="theme-swatch-chip" style={{ background: t.swatch }} />
              <span className="theme-swatch-name">{t.label}</span>
            </button>
          ))}
        </div>
        <div className="row settings-toggle-row" style={{ marginTop: 12 }}>
          <label className="toggle-label">CRT scanlines</label>
          <button
            className={`btn toggle-btn${crtEnabled ? " active" : ""}`}
            onClick={() => setCrtEnabled(!crtEnabled)}
          >
            {crtEnabled ? "on" : "off"}
          </button>
        </div>
        <p className="settings-hint">
          overlay scanlines, phosphor glow, and vignette for a CRT terminal aesthetic
        </p>
      </div>

      <div className="card" onContextMenu={(e) => show(e, [
        { label: discordEnabled ? "turn off" : "turn on", rune: "ᚷ", action: toggleDiscord },
      ])}>
        <label className="label">Discord Rich Presence</label>
        <div className="row">
          <p className="settings-hint flush">
            show what you&rsquo;re doing on your Discord profile
          </p>
          <button
            className={`btn toggle-btn${discordEnabled ? " active" : ""}`}
            onClick={toggleDiscord}
          >
            {discordEnabled ? "on" : "off"}
          </button>
        </div>
      </div>

      {/* ── Encoding / hardware acceleration ── */}
      <div className="card">
        <label className="label">ᚲ encoding</label>

        <div className="row" style={{ marginBottom: 8 }}>
          <div className="row-grow">
            {encodersLoading ? (
              <div className="skeleton-bar" />
            ) : (
              <Dropdown
                options={encoderDropdownOptions}
                value={preferredVideoEncoder}
                showCategories={availableEncoders.length > 0}
                onChange={(v) => setPreferredVideoEncoder(v as string)}
              />
            )}
          </div>
        </div>

        {!encodersLoading && availableEncoders.length === 0 && (
          <p className="settings-hint" style={{ marginTop: 8 }}>
            no hardware encoders detected — encoding will use software codecs
          </p>
        )}

        <div className="row settings-toggle-row" style={{ marginTop: 8 }}>
          <label className="toggle-label">auto-fallback to software</label>
          <button
            className={`btn toggle-btn${autoFallbackHw ? " active" : ""}`}
            onClick={() => setAutoFallbackHw(!autoFallbackHw)}
          >
            {autoFallbackHw ? "on" : "off"}
          </button>
        </div>
        <p className="settings-hint">
          when the selected hardware encoder is unavailable, use the default
          software encoder instead of failing
        </p>
      </div>

      {/* ── Downloads (yt-dlp import) ── */}
      <div className="card" onContextMenu={(e) => show(e, [
        { label: "browse", rune: "ᚨ", action: pickDownloadFolder },
        { label: "copy path", rune: "ᚷ", action: () => copyToClipboard(downloadDir) },
        ...(downloadDir ? [{ label: "clear", rune: "ᛏ", action: () => setDownloadDir("") }] : []),
      ])}>
        <label className="label">ᛣ downloads folder</label>
        <div className="row">
          <input className="input" value={downloadDir} placeholder="default: app-managed downloads folder" readOnly />
          <button className="btn" onClick={pickDownloadFolder}>browse</button>
        </div>
        <p className="settings-hint">
          yt-dlp is downloaded on first use from GitHub (~15 MB) and cached on your machine.
        </p>

        <div className="row settings-toggle-row" style={{ marginTop: 12 }}>
          <label className="toggle-label">auto-download subtitles</label>
          <button
            className={`btn toggle-btn${autoDownloadSubtitles ? " active" : ""}`}
            onClick={() => setAutoDownloadSubtitles(!autoDownloadSubtitles)}
          >
            {autoDownloadSubtitles ? "on" : "off"}
          </button>
        </div>
        <p className="settings-hint">
          fetch available subtitle tracks alongside media when importing from a URL
        </p>

        <div className="row settings-toggle-row" style={{ marginTop: 12 }}>
          <label className="toggle-label">auto-embed subtitles</label>
          <button
            className={`btn toggle-btn${autoEmbedSubtitles ? " active" : ""}`}
            onClick={() => setAutoEmbedSubtitles(!autoEmbedSubtitles)}
          >
            {autoEmbedSubtitles ? "on" : "off"}
          </button>
        </div>
        <p className="settings-hint">
          mux downloaded subtitles directly into the output file (mkv/mp4) so they
          ship with the video
        </p>
      </div>
    </div>
  );
}
