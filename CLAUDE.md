Acta board: aaen-studios/galdr

# galdr

- `/`: the desktop app. Tauri 2 shell in `src-tauri/`, React frontend in `src/`.
- `web/`: the landing site, a Next.js static export served at galdr.aaenz.no from the VPS. A push to `master` that touches `web/` deploys it.

## web/

That Next.js version has breaking changes, so APIs, conventions and file structure may differ from what you know. Read the relevant guide in `web/node_modules/next/dist/docs/` before writing code there, and heed deprecation notices.
