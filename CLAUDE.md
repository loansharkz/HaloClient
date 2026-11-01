# Halo Client: notes for Claude

- **Project memory:** Leo's Obsidian vault note is `C:\Users\sariy\Desktop\ClaudeVault\Projects\Halo Client.md`. A copy-ready version is kept in `docs/vault-note.md`. Update it as phases complete.
- **Decisions:** `DECISIONS.md` is the source of truth. Log every autonomous choice there.
- **Build spec:** `docs/SPEC.md` (Leo's original spec).
- **Hard rules:**
  - Multiplayer must keep working every phase. Log test results in `DECISIONS.md`.
  - Use official APIs only. Bundle no Minecraft code or assets. Hash-verify every download.
  - No secrets in the frontend. The CurseForge key, MSA client ID and Sentry DSN live in the backend/env only.
  - Ads (Phase 7) are design-only until Leo says go: `ADS_ENABLED=false`.
- **Stack:** Tauri v2 + SolidJS/TS/Tailwind v4, pnpm. Windows is the target; installers are built in GitHub Actions.
- **Commands:** `pnpm dev` (port 1420), `pnpm build` (typecheck + bundle).
- **Layout:** `src/pages/*` lazy routes (Home is eager), `src/components/*`, `src/lib/mock.ts` (shapes mirror future IPC types), `src/lib/store.ts` (settings + app state).
