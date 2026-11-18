# galdr-web

Landing page for **galdr** — a desktop GUI wrapper around FFmpeg with a terminal aesthetic and runic theme.

> **media incantations** — raw media in, enchanted media out.

## Stack

- **Framework:** Next.js 16 (App Router, static export)
- **Language:** TypeScript
- **Styling:** CSS Modules (no Tailwind, no CSS frameworks)
- **Animation:** Framer Motion
- **Font stack:** System monospace only (no web font downloads)
- **Deployment:** Static export (`next.config.ts` → `output: 'export'`)

## Getting started

```bash
bun install
bun run dev    # or: npm run dev
bun run build  # or: npm run build
```

The build outputs to `out/` — a fully static site.

## Structure

```
src/
  app/
    globals.css            # Design tokens, reset, CRT overlay
    layout.tsx             # Root layout, metadata, JSON-LD
    page.tsx               # Home page (server component)
    not-found.tsx          # Custom 404 page
    opengraph-image.tsx    # Build-time OG image generation
  components/
    HeroSection.tsx        # Full-vh hero with ASCII mockup
    AboutSection.tsx       # Terminal-style about card
    FeaturesSection.tsx    # 6-card feature grid
    AlchemySection.tsx     # Command builder card
    DownloadSection.tsx    # Server component, GitHub API fetch
    FooterSection.tsx      # Colophon footer
    StickyNav.tsx          # Fixed nav fades in on scroll
    BackToTop.tsx          # Floating back-to-top button
    ScrambleText.tsx       # Rune-scramble heading animation
    Button.tsx             # motion.button wrapper
    Divider.tsx            # Runic horizontal rule
  lib/
    github.ts              # Shared GitHub release fetch logic
```

## Key design decisions

- **No images** — the ASCII mockup, box-drawing characters, and runes carry all visual weight
- **No `use client`** unless required — maximises server components for smaller bundles
- **Build-time data** — the GitHub releases API is called once during `next build`, so the download section is fully static at runtime
- **`prefers-reduced-motion`** respected globally — all animations are skipped for accessibility

## Deployment

Push to GitHub, connect to Vercel — it auto-detects Next.js and runs the build. The `out/` directory is the production artifact.

For GitHub Pages, add a workflow:

```yaml
- uses: actions/checkout@v4
- uses: oven-sh/setup-bun@v2
- run: bun install && bun run build
- uses: peaceiris/actions-gh-pages@v4
  with:
    publish_dir: ./out
```

## Related

- [galdr desktop app](https://github.com/aaen-studios/galdr) — the app this site is for