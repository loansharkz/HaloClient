# Halo Client — Minecraft Launcher Build Spec for Claude Code

> **Name:** Halo Client
> **Owner:** Leo
> **Goal:** A fast, polished Minecraft Java launcher with multi-account support, built-in Modrinth + CurseForge browsing, its own mod-loader handling, and a performance-tuned launch path that gets players into the game far faster than vanilla. Ad monetization comes **later** (see Phase 5); design for it now, don't build it yet.

---

## ⚠️ STEP 0 — ASK QUESTIONS BEFORE WRITING ANY CODE

**Do not scaffold, install, or write code until this step is finished.**

1. Read this whole file first.
2. **Research first:** look up **Horizon Client** (the Minecraft client Leo referenced) and the "Rust" Minecraft client he mentioned. Find out what they actually are, how they get their fast load times, what features they ship, and what their UI looks like. Summarize the findings for Leo in a few lines and use them to sharpen your questions. If you can't find solid info, tell him so and ask him to describe what he liked about them.
3. Ask Leo **as many questions as you need — there is no cap.** Ask everything below **plus your own follow-up questions**. Go deep: it's way better to over-ask now than guess wrong later. You can ask in multiple rounds (answers often lead to better follow-ups). Don't ask things you can decide yourself with a sensible default.
4. Write his answers into a `DECISIONS.md` in the project root.
5. After that, **work autonomously** — Leo prefers to submit and let it run. Don't stop for confirmations after the question round unless something is irreversible or actually blocked (missing API key, missing Azure app approval, etc.). For anything else, pick the reasonable option and log it in `DECISIONS.md`.

### Questions to ask

**Platform**
- Which OS targets for v1? Windows only, or Windows + macOS + Linux?

**Look & feel (ask a lot here — Leo cares about this)**
- Overall vibe: sleek dark/minimal (Lunar-style), gamer/RGB, glassy/frosted, soft/pastel, brutalist, anime/cherry-blossom, Mio-style rainbow, something else? Can he point at launchers/clients/apps he thinks look good (incl. Horizon Client)?
- Dark mode only, light mode only, or both with a toggle? User-selectable themes / custom accent color?
- Main color palette and accent color?
- Logo idea for Halo Client — literal halo ring, glowing circle, letter mark, something else? Should Claude generate a placeholder logo (SVG)?
- Layout: left sidebar nav, top tab bar, or minimal floating nav? Big hero "Play" button on home?
- Home screen content: news feed, recent instances, featured modpacks, friends, server list, a 3D skin render of the current account?
- Background: solid, gradient, blurred Minecraft-style screenshot, animated/particle background, video background? (Animated stuff must not hurt the instant-open goal.)
- Fonts: clean modern sans (Inter/Geist), something blocky/pixel, or a mix (pixel for headings)?
- Animation level: none, subtle micro-interactions, or flashy transitions?
- Window style: custom frameless window with its own titlebar, or normal OS window? Rounded corners?
- Mod browser layout: grid of cards with banners, or compact list? Both with a toggle?
- Any sounds (click/launch sounds) or keep it silent?
- Should Claude make a quick clickable UI mockup first (before the backend) so Leo can approve the look?

**Tech stack**
- Launcher shell: **Tauri (Rust backend + web UI)** — recommended: tiny binary, opens instantly, low RAM — vs Electron (easier, heavier, slower to open) vs native Rust UI (egui/Slint). Confirm Tauri unless Leo wants otherwise.
- Frontend framework inside Tauri: React, Svelte, or SolidJS? (Default: React + TypeScript + Tailwind.)

**Accounts**
- "Alt support" = multiple Microsoft accounts with quick switching. Also want **offline/local profiles** for singleplayer + offline-mode servers? (Offline profiles must never be presented as a way to play online servers without owning the game.)
- Has Leo already registered an **Azure app** for Microsoft login? New apps must request Minecraft API access via Mojang's form before Xbox→Minecraft auth works. If not, Claude should write step-by-step instructions and build the auth flow so it works the moment the client ID is plugged in.

**Mods / content**
- Does Leo have a **CurseForge API key** yet (from the CurseForge for Studios console)? If not, build Modrinth first and stub CurseForge behind a feature flag.
- Which mod loaders for v1? Fabric, Quilt, Forge, NeoForge — all four, or start with Fabric + NeoForge?
- Should the launcher import existing instances from CurseForge app / Prism / MultiMC / vanilla `.minecraft`? (Nice-to-have.)
- Support importing/exporting modpacks (`.mrpack` and CurseForge `.zip`)?

**"Instant load" / performance**
- Leo referenced fast-loading clients ("Rust client", "Horizon client"). Clarify: does he mean (a) the **launcher** opens instantly, (b) **Minecraft itself** boots fast, or (c) both? (Plan assumes both — see Performance section.)
- Should the launcher ship a built-in **"Performance" profile** that auto-installs a vetted Fabric performance stack (Sodium, Lithium, FerriteCore, ModernFix, ImmediatelyFast, Entity Culling, etc.) on a fresh instance?
- Bundle its own Java runtime (auto-downloaded per MC version, e.g. Adoptium/Azul) or rely on system Java? (Default: auto-managed runtimes.)

**Features / "all the features"**
- Any must-have client-side features beyond a normal launcher? e.g. Discord Rich Presence, server list with ping, screenshot gallery, crash-log analyzer, per-instance RAM/JVM settings, skin/cape viewer & changer, friends list, auto-updater for the launcher itself.
- Any PvP-client-style in-game features (HUD mods, FPS counter, keystrokes, zoom)? If yes → those should be a **companion Fabric mod** the launcher can auto-install, not baked into the launcher. Confirm.

**Monetization (future, design-only now)**
- Ad placement idea for later: home/news panel banner, mod-browser sidebar, or launch-screen interstitial? (Just so the layout reserves space now.)
- Planning a paid "no ads" tier later?

**More to ask (follow-ups)**
- Any features from Horizon Client (after research) he specifically wants copied in spirit?
- Should the launcher have its own account/login system (Halo Client accounts, cloud-synced settings), or stay local-only?
- Telemetry/crash reporting: none, opt-in, or on by default?
- Distribution: where will people download it (website, GitHub releases)? Does he want a landing page site too? (He prefers a unique domain per project.)
- Target audience: PvP players, modpack players, casual players, Donut SMP-type economy players?
- Anything he *hates* in other launchers (CurseForge app bloat, TLauncher sketchiness, slow Lunar startup, etc.)?

**Workflow**
- Project folder name/location on Desktop? (Default: `C:\Users\sariy\Desktop\HaloClient`.)
- Git repo + GitHub? Public or private?

---

## Project setup (do this right after questions)

Leo keeps project memory in his Obsidian vault at `C:\Users\sariy\Desktop\ClaudeVault`:

1. Create a vault note from `Templates/Project Template.md` in `Projects/`.
2. Add it to the **Projects Index** and the right **Hub** note.
3. Fill in **Connections** (hub, tech notes, related projects — e.g. LARPcraft, StarCore, other Minecraft projects).
4. Create a `CLAUDE.md` in the project folder pointing to the vault note.
5. Keep the vault note updated as phases complete (status, decisions, blockers).

---

## Core architecture (default — adjust per answers)

```
launcher/
├── src-tauri/                 # Rust backend
│   ├── auth/                  # MSA → Xbox Live → XSTS → Minecraft token flow, token refresh, account store
│   ├── instances/             # instance create/clone/delete, per-instance config
│   ├── versions/              # Mojang version manifest, libraries, assets, natives
│   ├── loaders/               # Fabric / Quilt / Forge / NeoForge installers
│   ├── content/               # Modrinth + CurseForge clients, dependency resolver, update checker
│   ├── java/                  # JRE discovery + auto-download per MC version
│   ├── launch/                # classpath build, JVM args, process spawn, log streaming
│   ├── cache/                 # shared content-addressed file cache (hash-deduped jars/assets)
│   └── download/              # parallel downloader, SHA1/SHA512 verification, retry, resume
├── src/                       # Web UI (React/TS by default)
│   ├── pages/  Home, Instances, Browse (Mods/Resource Packs/Shaders/Modpacks/Datapacks), Accounts, Settings
│   └── components/
├── DECISIONS.md
└── CLAUDE.md
```

**Data layout:** one launcher data dir, instances in `instances/<id>/`, shared `libraries/`, `assets/`, `versions/`, `java/`, and a hash-keyed `cache/` so the same mod jar is never downloaded twice across instances.

---

## Feature spec

### 1. Accounts ("alt support")
- Unlimited Microsoft accounts, one-click switch, set default per instance.
- Device-code or embedded-webview OAuth (Tauri webview).
- Tokens encrypted at rest using OS keychain (Windows Credential Manager / macOS Keychain / Secret Service). **Never** log tokens.
- Silent token refresh before launch; clear error if the account doesn't own Java Edition.
- Skin + cape preview per account (skin change via official Mojang API).
- Offline/local profiles (if Leo says yes): username only, clearly labeled "Offline", for singleplayer/LAN/offline-mode servers.

### 2. Content browser (Mods / Resource Packs / Shader Packs / Modpacks / Data Packs)
- Tabs for each content type; source toggle: **Modrinth | CurseForge | Both**.
- Search with filters: MC version, loader, category, sort (relevance/downloads/updated), environment (client/server).
- Defaults filter to the **current instance's** MC version + loader.
- Project page view: description (render markdown/HTML safely), gallery, versions list, changelog, dependencies.
- One-click install → auto-resolve required dependencies, warn on incompatibilities/duplicates.
- Installed tab per instance: enable/disable (rename to `.disabled`), update-all, check for updates, remove.
- Shader packs: detect if Iris/Oculus is missing and offer to install it.
- **APIs:**
  - Modrinth API v2 — set a proper `User-Agent` (`leo/HaloClient/<version> (contact)`), respect rate limits.
  - CurseForge API — requires API key; **respect `allowModDistribution`**: if a project disallows third-party distribution, show a "Download from CurseForge" button that opens the page instead of auto-downloading. Never hardcode the key in the frontend bundle.
- Modpack import/export: `.mrpack` and CurseForge manifest zips.

### 3. Mod loaders
- Install Fabric / Quilt / Forge / NeoForge for any supported MC version from the instance creation dialog.
- Fabric/Quilt: use their meta APIs.
- Forge/NeoForge: run official installers headless or replicate processors; cache results.
- Loader version picker with "recommended/latest" default.

### 4. Instances
- Create from: vanilla version, loader + version, modpack, or import.
- Per-instance: RAM, JVM args, Java runtime, window size, pre-launch/post-exit commands, game dir.
- Duplicate, export, open folder, icon picker.
- Live log console with search + "copy for support"; crash detection with a basic analyzer (common causes: wrong Java, missing dependency, mod conflict).

### 5. Multiplayer — HARD REQUIREMENT
Multiplayer is non-negotiable. Every speed optimization, every mod bundle, every feature must keep normal online play working.
- Normal online play must work exactly like vanilla (real Mojang session auth via the logged-in Microsoft account) on vanilla, Fabric, Forge, NeoForge and Quilt instances.
- Must join regular public servers (Hypixel, Donut SMP, 2b2t-style, etc.) without kicks. Performance mods in the perf profile must be **client-side only** and server-safe — no mods that servers flag as unfair or that require server-side installs.
- **Test multiplayer at the end of every phase** (join a real online server with a real account, plus a local test server) and record the result in `DECISIONS.md`. A phase isn't done if multiplayer broke.
- Server list per instance (reads/writes `servers.dat`) with ping/MOTD/player count.
- "Quick join" button that launches straight into a server (`--quickPlayMultiplayer` on 1.20+, `--server` on older).

### 6. Settings
- Default RAM, Java management, download threads, theme, language, data folder location, close-launcher-on-launch, Discord RPC toggle.
- Launcher self-updater (Tauri updater with signed releases).

---

## ⚡ Performance — "instant load" plan

Be honest in the UI and docs: Minecraft is a JVM game, so true zero-second boot isn't possible. The goal is **launcher opens in <1s** and **game reaches title screen dramatically faster than vanilla**. Do all of these:

**Launcher side**
- Tauri/Rust backend → small binary, near-instant cold start, low idle RAM.
- Lazy-load UI pages; cache version manifests and search results locally.
- Parallel, resumable, hash-verified downloads; shared deduped cache.
- **Pre-flight on launcher open:** verify/refresh auth token and pre-validate the last-played instance's files in the background, so clicking Play skips straight to spawn.

**Game side**
- Auto-managed Java per version (Java 21 for modern versions) with tuned default flags (G1GC or generational ZGC on Java 21+; don't over-allocate RAM — sensible default ~4GB, user-adjustable).
- **AppCDS / class data sharing:** generate a CDS archive per instance on first launch, reuse on later launches to cut JVM class-loading time. Benchmark it; keep only if it measurably helps.
- Optional **Performance profile** (if Leo approves): auto-install a vetted Fabric stack — Sodium, Lithium, FerriteCore, ModernFix (big load-time win), ImmediatelyFast, Entity Culling, etc. Pull versions from Modrinth matched to MC version; let users toggle each.
- Measure it: add a launch timer (click Play → title screen, using log markers) and show it in the instance view. Keep a `BENCHMARKS.md` comparing vanilla vs tuned.

---

## 💰 Monetization (Phase 5 — DO NOT BUILD YET)

Design only:
- Reserve layout slots (e.g. home news panel, browse sidebar) as empty placeholder components behind a `ADS_ENABLED=false` flag.
- Ads must **never** appear in-game, never slow down launch, and never block the Play button.
- Note for later: desktop apps can't just drop in AdSense like a website — research desktop-allowed ad networks or a sponsored-content/news feed approach when this phase starts. Also check that ad content near Minecraft branding follows Mojang's usage guidelines.

---

## Build phases

1. **Foundation** — Tauri app scaffold, UI shell + routing, settings store, downloader + cache, Java auto-management.
2. **Vanilla launch** — version manifest, libraries/assets/natives, Microsoft multi-account auth, launch vanilla and join a real server.
3. **Loaders + instances** — Fabric/Quilt/NeoForge/Forge install, full instance management, logs, crash analyzer.
4. **Content browser** — Modrinth first (full), then CurseForge (behind key), dependency resolution, modpack import/export, shaders/resource packs.
5. **Performance pass** — CDS, tuned flags, perf profile, launch timer, benchmarks.
6. **Polish** — themes, Discord RPC, quick-join, skins, self-updater, installer builds (`.msi`/`.exe`, plus `.dmg`/AppImage if cross-platform).
7. **(Later) Ads** — only when Leo says go.

At the end of each phase: run it, fix what breaks, update the vault note + `DECISIONS.md`, commit.

---

## Rules for Claude Code
- After Step 0, run autonomously; log choices in `DECISIONS.md`.
- Multiplayer must work at every phase — never ship a change that breaks online play.
- Ask the Step 0 research + questions first (no question cap). Hand Leo **complete files** when showing changes, not partial diffs.
- Use official APIs only (Mojang/Microsoft, Modrinth, CurseForge, Fabric/Quilt/Forge/NeoForge meta). No Minecraft code or assets bundled in the launcher — everything downloads from official sources at runtime.
- Verify every downloaded file by hash.
- Never hardcode secrets (Azure client secret if any, CurseForge key) in shipped frontend code; use env/config + backend proxy where needed.
- Write a `README.md` covering setup, Azure app registration steps, CurseForge key setup, and build commands.
