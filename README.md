# Halo Client

A fast, glassy Minecraft: Java Edition launcher for Windows. It handles multiple Microsoft accounts with one-click switching, has a built-in Modrinth browser (CurseForge too, once a key is set), and supports Fabric, Quilt, Forge and NeoForge. Launches use tuned Java flags, class-data sharing and a server-safe Performance mod profile. It also ships with a companion HUD mod.

Halo is not affiliated with Mojang or Microsoft. It ships no Minecraft code or assets; everything is downloaded from official sources at runtime and verified by hash.

## What's in the box

| Area | Highlights |
|---|---|
| Accounts | Microsoft device-code login (MSA → Xbox → XSTS → Minecraft), silent token refresh, ownership check, unlimited alts, offline profiles (singleplayer/LAN only), skin upload and cape switching. Tokens live in Windows Credential Manager. |
| Instances | Vanilla, Fabric, Quilt, Forge, NeoForge. Per-instance RAM, Java, JVM args, window size, hooks, account. Duplicate, repair, delete, custom icons. |
| Content | Modrinth search with facets, project pages (description, gallery, versions), one-click install with required dependencies, update checks, enable/disable, shader-loader detection, data packs per world. CurseForge behind an API key, respecting authors' `allowModDistribution`. |
| Modpacks | Install Modrinth packs as new instances; import/export `.mrpack`; import Prism/MultiMC instances and vanilla `.minecraft`. |
| Speed | Rust/Tauri launcher (small, opens fast), background pre-flight, parallel hash-verified downloads with a shared cache, managed Adoptium Java per version, tuned G1 flags, AppCDS on Java 19+, Performance profile (Sodium, Lithium, FerriteCore, ModernFix, ImmediatelyFast, Entity Culling), Play → title-screen timer. |
| Multiplayer | Real Mojang session auth, Quick join for servers you've played (live favicon, MOTD, players, ping), servers.dat editor, quick-play straight into a server. |
| Extras | Live logs with search and "copy for support", crash analyzer with one-click fixes, screenshot gallery, Discord Rich Presence, update check, opt-out crash reports. |
| Halo HUD | Companion Fabric mod: FPS/CPS/ping, keystrokes, armor durability, potion timers, zoom. Display-only. |

## Repository layout

```
src/                  SolidJS + TypeScript + Tailwind UI
  lib/api.ts          typed IPC bridge (mock backend in the browser)
  lib/store.ts        app state + backend events
  pages/              Home, Library, Instance, Browse, Project, Accounts, Settings
src-tauri/            Rust backend (Tauri v2)
  src/auth.rs         Microsoft login, keychain, offline profiles, skins/capes
  src/versions.rs     Mojang manifest, version JSON inheritance, rules, files
  src/java.rs         Adoptium runtimes
  src/loaders.rs      Fabric/Quilt meta, Forge/NeoForge installers
  src/launch.rs       arguments, JVM flags, CDS, process + log monitor, crash analyzer
  src/content.rs      Modrinth/CurseForge, perf profile, Halo HUD, modpacks, imports
  src/servers.rs      Server List Ping, servers.dat, recent servers
mods/halo-hud/        companion Fabric mod (Gradle)
docs/                 spec, roadmap, multiplayer checklist, vault note
```

## Develop

Requires Node 20+, pnpm 10, Rust (stable) and, for the HUD mod, JDK 25.

```bash
pnpm install
```

Run the full desktop app (Rust backend + UI, hot reload):

```bash
pnpm tauri dev
```

UI only in a browser, with a mock backend:

```bash
pnpm dev
```

Backend tests (one of them launches a real JVM):

```bash
cd src-tauri && cargo test
```

Build the HUD mod:

```bash
cd mods/halo-hud && ./gradlew build
```

Build Windows installers locally (on Windows):

```bash
pnpm tauri build
```

Releases: push a tag like `v0.1.0`. GitHub Actions builds the HUD mod, bundles it, and produces `.msi` and setup `.exe` installers in a draft release.

## Configuration

These are read by the **Rust backend at build time** (or from the environment at runtime). They are never put in the web UI bundle. In GitHub, set them under repo **Settings → Secrets and variables → Actions**.

| Variable | Needed for | Notes |
|---|---|---|
| `HALO_MSA_CLIENT_ID` | Microsoft login | Azure app client ID (steps below). Required to sign in. |
| `CURSEFORGE_API_KEY` | CurseForge browsing | Optional. The CurseForge tab stays disabled without it. |
| `HALO_SENTRY_DSN` | Crash reports | Optional. Reporting is off when unset. Users can opt out. |
| `HALO_DISCORD_APP_ID` | Discord Rich Presence | Optional. Create an app at discord.com/developers and upload an art asset named `halo`. |
| `VITE_ADS_ENABLED` | Phase 7 ad slots | Default `false`. |

A note on secrets in desktop apps: an API key compiled into a native binary can be extracted by a determined person. That's the standard trade-off for desktop launchers. The key never reaches the webview, and a server-side proxy would be the next step if abuse appears.

### Azure app registration (Microsoft login)
1. Go to <https://portal.azure.com> → **Microsoft Entra ID** → **App registrations** → **New registration**.
2. Name it `Halo Client`. For **Supported account types**, choose *Personal Microsoft accounts only*.
3. Leave the redirect URI empty and click **Register**. Copy the **Application (client) ID**.
4. **Authentication** → **Advanced settings** → set **Allow public client flows** to **Yes** (needed for the device-code flow). Save.
5. Don't create a client secret. Halo is a public client and must never ship one.
6. **Request Minecraft API access:** new apps are blocked from `api.minecraftservices.com` until Mojang approves them. Submit the form at <https://aka.ms/mce-reviewappid> with your client ID. Approval can take days to weeks.
7. Set `HALO_MSA_CLIENT_ID` (repo secret for releases, or an environment variable for `pnpm tauri dev`). Until Mojang approves the app, sign-in stops at the Minecraft step with a message saying so.

For local testing you can also paste a client ID in **Settings → Advanced**.

### CurseForge API key
1. Apply at <https://console.curseforge.com> (CurseForge for Studios) and wait for approval.
2. Set `CURSEFORGE_API_KEY`. All CurseForge calls go through the backend.
3. Projects whose authors disallow third-party downloads show a **Download from CurseForge** link instead of installing.

## Discord Rich Presence (shows "Playing Halo Client" with the logo)
1. Go to <https://discord.com/developers/applications> → **New Application** → name it **Halo Client**. Discord shows this name after "Playing".
2. **General Information** → App Icon: upload `branding/halo-logo-1024.png`.
3. **Rich Presence → Art Assets** → **Add Image(s)**: upload `branding/discord-halo-512.png` and name it exactly `halo`. New assets can take a few minutes to appear.
4. The Application ID (`1566610639223529514`) is built in as the default. Override it with `HALO_DISCORD_APP_ID` if you ever make a new Discord app.

The status shows the instance name, its version and loader (or the server you joined), elapsed time, and a **Get Halo Client** button that links to the download page.

## Sharing with friends
- **Download page:** `site/` is a one-page site with a Download button that always points at the newest installer. Turn it on once under repo **Settings → Pages → Source: GitHub Actions**. It deploys from the default branch to <https://loansharkz.github.io/HaloClient/>, or run the "Download site" workflow manually.
- **Updates:** installed copies update themselves. They check GitHub Releases at startup, download and verify the new installer, and ask to restart.
- **Installers:** push a tag (`v0.1.0`) → GitHub builds and publishes the release, and the download page picks it up automatically.
- **Windows warning:** the installer isn't code-signed yet, so Windows SmartScreen shows "Windows protected your PC". Friends click **More info → Run anyway**. Code signing (e.g. Azure Trusted Signing, about $10/month) removes this later.
- **Custom domain:** buy a domain, add it under Settings → Pages → Custom domain, then set `HALO_DOWNLOAD_URL` so the Discord button uses it.

## Data on disk

Everything lives in `%APPDATA%\HaloClient` (movable in Settings): `instances/<id>/minecraft` (game folders), shared `libraries/`, `assets/`, `versions/`, `java/`, a hash-keyed `cache/` that hard-links identical mod jars across instances, and `logs/launcher.log`.

## Multiplayer guarantee

Every change must keep normal online play working: real session auth, no server-side or unfair mods in the Performance profile. Run [`docs/MULTIPLAYER_TESTS.md`](docs/MULTIPLAYER_TESTS.md) before each release and log results in [`DECISIONS.md`](DECISIONS.md).
