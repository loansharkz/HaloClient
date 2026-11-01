# Roadmap

| # | Phase | Status |
|---|---|---|
| 0 | UI mockup | ✅ approved |
| 1 | Foundation: Tauri v2 shell, frameless window, settings, parallel hash-verified downloader + shared cache, Adoptium Java | ✅ built, unit-tested |
| 2 | Vanilla launch: manifest, libraries/assets/natives, Microsoft multi-account auth, offline profiles | ✅ built; needs Azure app + real-account test |
| 3 | Loaders + instances: Fabric/Quilt/Forge/NeoForge, instance management, logs, crash analyzer, imports | ✅ built; needs Windows test |
| 4 | Content: Modrinth (full), CurseForge (behind key), dependencies, updates, .mrpack import/export, shaders, data packs | ✅ built |
| 5 | Performance: tuned G1, AppCDS (real-JVM tested), Performance profile, boot timer | ✅ built; benchmarks pending (docs/BENCHMARKS.md) |
| 6 | Polish: Discord RPC, quick join, skins/capes, screenshots, Sentry opt-out, update check, MSI/NSIS via Actions | ✅ built |
| 6b | Halo HUD mod (26.3) | ✅ written; compiled by CI |
| 7 | Ads | 🚫 design-only until Leo says go |

## Next up (after first Windows test)
- Run docs/MULTIPLAYER_TESTS.md and docs/BENCHMARKS.md, then log the results.
- Switch to Tauri's signed auto-updater (generate a signing key, add `plugins.updater`).
- CurseForge `.zip` modpack import (needs the API key).
- Landing page + domain.
