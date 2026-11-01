# Halo Client

A fast, glassy Minecraft: Java Edition launcher for Windows. It has multi-account switching, a built-in Modrinth browser (CurseForge once a key is set), Fabric/Quilt/Forge/NeoForge support, and a performance-tuned launch path.

> **Status: Phase 0, UI mockup.** The frontend runs on mock data so the look can be approved before the Rust backend is built. See [`DECISIONS.md`](DECISIONS.md) for every product decision and [`docs/ROADMAP.md`](docs/ROADMAP.md) for the phase plan.

Halo is not affiliated with Mojang or Microsoft. It ships no Minecraft code or assets; everything is downloaded from official sources at runtime.

## Stack
- **Tauri v2** (Rust backend). Small binary, instant open, low idle RAM.
- **SolidJS + TypeScript + Tailwind v4** frontend. Inter font is bundled, so nothing is fetched to render the UI.
- `skinview3d` for the 3D skin render. It's lazy-loaded so it never blocks first paint.

## Run the mockup
Requires Node 20+ and pnpm.

```bash
pnpm install
```

```bash
pnpm dev
```

Then open http://localhost:1420. `pnpm build` typechecks and produces `dist/`.

## Configuration (backend phases)
All secrets live in the **Rust backend / build environment**, never in the frontend bundle.

| Variable | Needed for | Notes |
|---|---|---|
| `HALO_MSA_CLIENT_ID` | Microsoft login | Azure app client ID (steps below) |
| `CURSEFORGE_API_KEY` | CurseForge browsing | Optional; CurseForge UI stays hidden without it |
| `HALO_SENTRY_DSN` | Crash reports | Optional; reporting is inert without it |
| `VITE_ADS_ENABLED` | Phase 7 ad slots | Default `false` |

### Azure app registration (Microsoft login)
1. Go to <https://portal.azure.com> → **Microsoft Entra ID** → **App registrations** → **New registration**.
2. Name it `Halo Client`. For **Supported account types**, choose *Personal Microsoft accounts only*.
3. Leave the Redirect URI empty and click **Register**. Copy the **Application (client) ID**.
4. **Authentication** → **Advanced settings** → set **Allow public client flows** to **Yes** (needed for the device-code flow). Save.
5. No client secret is needed. Halo is a public client and must never ship a secret.
6. **Request Minecraft API access:** new apps are blocked from `api.minecraftservices.com` until Mojang approves them. Submit the form at <https://aka.ms/mce-reviewappid> with your client ID and wait for approval (it can take days to weeks).
7. Put the ID in `HALO_MSA_CLIENT_ID` and build. Until approval, login fails at the Xbox → Minecraft step with a clear error.

### CurseForge API key
1. Apply at <https://console.curseforge.com> (CurseForge for Studios) and wait for approval.
2. Set `CURSEFORGE_API_KEY` in the build/runtime environment. The backend proxies all CurseForge calls, so the key never reaches the webview.
3. Projects with `allowModDistribution: false` show a **Download on CurseForge** button instead of auto-downloading.

## Multiplayer guarantee
Every phase must keep normal online play working. The manual test checklist lives in [`docs/MULTIPLAYER_TESTS.md`](docs/MULTIPLAYER_TESTS.md), and results are logged in `DECISIONS.md`.
