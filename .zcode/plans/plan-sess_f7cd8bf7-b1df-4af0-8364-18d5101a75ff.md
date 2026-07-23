## UX polish: toast system + semantic CSS tokens

Two independent deliverables in one pass. **Hybrid** error approach (keep inline `alert-error` for in-context form errors; add toasts for background/silent outcomes) and **semantic tokens only** (low-risk, sets up theming later).

---

### Part A — Global toast/notification system

**New files:**

1. **`src/store/toastStore.ts`** — a small Zustand store. Shape:
   - `Toast` type: `{ id: string; kind: "success" | "error" | "info" | "warn"; title: string; message?: string; action?: { label: string; onClick: () => void } }`
   - State: `toasts: Toast[]`
   - Actions: `push(partial) → id` (generates id, appends, sets a 5s auto-dismiss timer via `setTimeout` unless `kind === "error"` which stays until dismissed — errors deserve a click), `dismiss(id)`, `clear()`
   - Cap stored toasts at e.g. 4 (drop oldest) to avoid pile-up during batch jobs.

2. **`src/components/Toaster.tsx`** — renders the toasts as a fixed-position stack (top-right, below the titlebar). Uses the existing Framer Motion pattern (AnimatePresence + motion.div) for enter/exit, matching how `UpdateBanner`/`HelpOverlay` already animate. Each toast: severity glyph (ᚠ info / ᛟ success / ᛏ warn / ᚹ error — reusing the runic vocabulary already in the app), title, optional message, optional action button, and a dismiss `×`. Styled with the new semantic tokens (Part B).

**Wiring (in `src/App.tsx`, minimal edits):**

3. **Mount** `<Toaster />` as a sibling of `<UpdateBanner />` inside `<main className="main-content">` at App.tsx:449. Fixed-position overlay so it floats above all pages.

4. **Queue job transitions → toasts.** The biggest UX gap: jobs complete/fail in the background and the user gets nothing unless the QueueDropdown is open. Add an effect in AppShell that subscribes to the queue store and diffs the jobs array to detect `running → completed` (success toast: "✓ {label}", action: "reveal in folder" if `outputPath`) and `running → failed` (error toast: "✓ {label} failed", message: `job.error`). Track previous statuses in a `useRef<Record<string, JobStatus>>`. This replaces the need to touch each page — one place covers convert/compress/concat/extract/transcription/subtitle/forge/batch.

5. **Settings load/save failures → toasts.** Two of the silent `.catch(() => {})` calls in App.tsx are worth surfacing:
   - `load_settings().catch(...)` at App.tsx:133 → warn toast "couldn't load saved settings — using defaults"
   - `save_app_preferences().catch(...)` at App.tsx:193 → error toast "couldn't save settings" (user thinks they persisted; they didn't)
   - The rest stay swallowed (version probe, titlebar/taskbar cosmetic, discord presence, autostart probe, forge recovery — all genuinely fine to ignore, per the inventory).

6. **Download completion → toast** (optional, low effort since downloadStore already has the `download-complete` listener at downloadStore.ts:213). Add a success toast there with a "show in folder" action using `lastDownloadedPath`. This is the second-biggest gap after the queue.

**Explicitly NOT changed this pass:**
- Inline `alert-error` blocks on ConvertPage/CompressPage/SubtitlesPage/ImportPage/ExtractFramesPanel — they stay (hybrid). They show in-context next to the triggering control, which is better UX for form-style errors than a global toast.
- Whisper/forge success paths beyond what the queue transition covers — out of scope.
- The watch folder per-folder activity UI (WatchFoldersPage:491) stays; watch completions also surface via the queue transition in step 4 since they route through the queue.

---

### Part B — Semantic CSS tokens

**Edit `src/App.css` `:root` block (lines 1-17)** — add semantic tokens, leaving the existing 6 tokens untouched:

```
--danger:      #ff6b6b   (destructive hovers — was scattered)
--danger-dark: #8b0000   (close-button hover bg)
--danger-dim:  #d08080   (failure-state text)
--warn:        #8b6914   (estimate warnings, btn-warn)
--warn-bg:     #1a1500   (warn card bg)
--info:        #8bc8ff   (command-palette flags)
--success:     #8bc34a   (success alerts, active cues)
--scrim:       rgba(0,0,0,0.75)  (modal/overlay backdrops)
--bg-elevated: #1a1a1a   (tooltips, elevated surfaces)
```

**Repoint the ~30 hardcoded sites** to `var(--token)`. Grouped (from the inventory):

- **Reds/danger:** App.css:93 (`#8b0000`→`--danger-dark`), 2222 & 5235 (`#ff6b6b`/`#d33`→`--danger`), 4861/4984/5016 (`#d08080`→`--danger-dim`), 4971/5068 (`#b04040`→`--danger-dim`); ForgePage.css:232/941/958 (`#ff6b6b`→`--danger`), 351 (`#8b0000`→`--danger-dark`).
- **Yellows/warn:** App.css:1226/1231/1235/1239/1417/1422/1423/1427 (`#8b6914`→`--warn`), 1227 (`#1a1500`→`--warn-bg`).
- **Blues/info:** App.css:2001/2041 (`#8bc8ff`→`--info`).
- **Greens/success:** App.css:4331/4696 (`#8bc34a`→`--success`), 4695 (`rgba(139,195,74,0.06)`→ a new `--success-bg` token or `color-mix`).
- **Elevated bg:** App.css:2029 (`#1a1a1a`→`--bg-elevated`); ForgePage.css:740.
- **Scrims:** the 6 `rgba(0,0,0,0.6–0.85)` modal backdrops in App.css (1695/1862/2380/2921/3030/4551) → `--scrim` (picking 0.75 as the unified value; these are close enough that one token is fine, and visual difference is negligible). ForgePage.css:329/897 likewise.
- **`--accent` left as-is** (harmless `--fg` alias; repointing it is scope creep for the "semantic only" choice).

**Add toast CSS** to App.css (a `.toaster` + `.toast` + `.toast-*` severity block) using the new tokens — placed near the existing `.alert-error` block (~L1548) since that's the semantic neighborhood.

**Not tokenized (deliberate):** the `rgba(200,200,200,N)` fg-alpha hovers (11 sites — they're literally `--fg` at alpha; folding them into tokens adds complexity for little gain this pass), the lone search-hit yellow (`rgba(255,235,59,0.08)` at 4700), ForgePage white-alpha literals, and `#000`/`#fff` pure literals (video canvas / subtitle text). These are candidates for a future "full tokenization" pass.

---

### Verification
- `tsc --noEmit` — typecheck the new store + component.
- `cargo check` not needed (frontend-only).
- Manual smoke (I can't run this, so I'll describe what to check): trigger a conversion and navigate away — success toast should fire on completion; kill a download mid-flight and confirm the error toast; toggle a setting with a read-only settings dir (if feasible) to confirm the save-failure toast. Confirm no visual regressions in the repointed color sites (close button hover, warn estimate bar, command-palette flags, success alerts).

### Out of scope (deferred)
- Repointing `--accent` to a real distinct value.
- A light theme (tokens make it *possible*; building it is a separate pass).
- Responsive design fixes.
- Replacing inline `alert-error` with toasts (the hybrid decision).

**Order of work:** Part B (tokens) first — it's mechanical and the toast CSS will use the new tokens, so doing tokens first means I write the toast CSS against real tokens instead of placeholders. Then Part A (toasts).