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
- **Commands:** `pnpm tauri dev` (full app), `pnpm dev` (UI + mock backend, port 1420), `pnpm build`, `cd src-tauri && cargo test`, `cd mods/halo-hud && ./gradlew build`, `pnpm build:preview` (single-file shareable preview).
- **Layout:** see README → Repository layout. `src/lib/types.ts` mirrors the Rust serde types: keep them in sync. `src/lib/mockApi.ts` answers IPC calls in a plain browser.
- **Halo HUD** targets MC 26.3 (Mojang names, SDL scancodes, `GuiGraphicsExtractor`). Check Fabric API source before using new game APIs.
