# Halo Client — Decisions Log

Answers from Leo's Step 0 question round (2026-11-01), plus choices Claude made on its own (marked **[default]**).

## Research summary (Step 0)
- **Horizon Client and the "Rust client" are the same project**: an open-source Rust rewrite of the Minecraft Java 26.3 *client*, not a launcher. It is fast because it has no JVM at all. Reported gains are 6.5× faster world load and 363 → 2,409 FPS in one wiki example, and coverage says these vary a lot by hardware.
- Horizon's limits: no Microsoft login, so offline-mode servers only. No Fabric/Forge mods, and only one MC version.
- **What Halo does instead:** Halo launches the real Java game so multiplayer and mods keep working. It goes after speed with a Rust/Tauri launcher that opens in under a second, a background pre-flight check, AppCDS, tuned Java 21+ flags, and a performance mod stack (ModernFix, Sodium, etc.).
- Leo liked: **world/boot speed** and **the lightweight feel**.

## Platform & stack
| Topic | Decision |
|---|---|
| OS targets v1 | **Windows only** (.msi + NSIS .exe). Code stays cross-platform where it's free. |
| Shell | **Tauri v2** (Rust backend) |
| Frontend | **SolidJS + TypeScript + Tailwind** |
| Build/dev env | Developed in a Linux cloud container; Windows installers built by **GitHub Actions** |

## Look & feel
| Topic | Decision |
|---|---|
| Vibe | **Glassy / frosted**: translucent blurred panels |
| Palette | **Violet / purple**: dark indigo glass with a violet→pink glow |
| Theme modes | **Dark + light toggle**, plus an **accent color picker** |
| Background | **Shooting stars** (changed 2026-11-01): night sky with twinkling stars and occasional shooting stars on one canvas, tinted by the accent, over a static violet nebula glow. The loop stops when the window is hidden, the game is running, the toggle is off, or reduced-motion is set. |
| Logo | **Glowing halo ring** (placeholder SVG by Claude) |
| Layout | **Left icon sidebar**, big hero Play button on Home |
| Home content | Recent instances + Play, **3D skin render**, **news feed**, **servers quick-join** |
| Quick join | Shows **only servers the player actually joined**, newest first. History comes from quick-joins plus `Connecting to <host>` lines in game logs. Each row shows the server's **real favicon + MOTD** (§ color codes rendered; no server HTML) and player count from a live Server List Ping, plus when you last played it and on which instance. Join launches that instance. |
| Pictures | Public projects use their **real Modrinth icon + gallery images** (cards, banners, project gallery, installed-mod rows, news art). Server-linked instances use the server favicon. Gradient tiles are only a fallback when media can't load. |
| Motion / window | **Subtle micro-interactions**; **frameless rounded window** with custom titlebar |
| Fonts | **Inter** (clean sans) |
| Mod browser | **Grid/list toggle**, grid by default |
| Sounds | **None** |
| Mockup first | **Yes**: real SolidJS UI with mock data, for Leo to approve before backend work |

## Accounts
- Multiple Microsoft accounts with one-click switching and a per-instance default.
- **Offline profiles: yes.** Labeled "Offline", for singleplayer/LAN/offline-mode servers only. **[default]** Creating one requires at least one owned Microsoft account to have been added.
- Azure app: **not registered yet.** Claude builds the device-code auth flow and writes README steps; it works once `HALO_MSA_CLIENT_ID` is set.
- Tokens are stored in the Windows Credential Manager (`keyring` crate) and never logged.

## Content
- CurseForge key: **none yet.** Modrinth is fully built first; CurseForge sits behind a backend-only `CURSEFORGE_API_KEY`, and its UI is hidden until the key is present.
- Loaders v1: **Fabric, Quilt, Forge, NeoForge**.
- Import/export: **.mrpack import/export**, **Prism/MultiMC import**, **vanilla .minecraft import**. CurseForge zip deferred.

## Performance
- Performance profile: **on by default** for new Fabric instances: Sodium, Lithium, FerriteCore, ModernFix, ImmediatelyFast, Entity Culling. Each can be toggled, and all are client-side and server-safe.
- Java: **[default]** auto-managed Adoptium runtimes per MC version (Java 8 / 17 / 21 / 25), downloaded and hash-verified.
- AppCDS per instance: keep only if the benchmark shows a gain (see BENCHMARKS.md).

## Features v1
- Discord Rich Presence
- Live logs + crash analyzer
- Skin/cape changer (official Mojang API)
- Screenshot gallery
- **Companion in-game mod "Halo HUD"** (Fabric, **latest MC release only**): FPS/CPS/ping, keystrokes, zoom, armor/potion HUD. Auto-installable from the launcher. All of these are display-only features that common PvP servers allow.

## Audience
PvP players, modpack players, SMP/economy players.

## Things to avoid (Leo's dislikes)
- Bloat / slow startup
- Forced launcher accounts. **Halo never requires a Halo account.**

## Privacy / telemetry
- Crash reporting **on by default, with an opt-out** shown on first run and in Settings.
- Sink: **Sentry** (free tier). It stays inactive until `HALO_SENTRY_DSN` is set at build time. Payloads never include tokens, usernames, or file paths outside the launcher dir.
- No Halo cloud account; **local-only**.

## Monetization (design only, Phase 7)
- Reserved slots: **Home news panel** card and **Browse sidebar**, both behind `ADS_ENABLED=false`. Never in-game, never on the launch path.

## Distribution & workflow
- **GitHub Releases** built by Actions. Landing page later.
- Repo: `loansharkz/haloclient`, branch `claude/inspiring-goodall-bt41fy`.
- Obsidian vault is not reachable from the cloud container. Claude writes `docs/vault-note.md` in the project template's spirit for Leo to copy into `ClaudeVault/Projects/`.
- Multiplayer testing: Claude can't log into a real account here, so each phase gets a manual checklist in `docs/MULTIPLAYER_TESTS.md` for Leo to run. Results are logged below.

## Multiplayer test log
| Phase | Date | Online server | Local server | Result |
|---|---|---|---|---|
| 0 (mockup) | — | n/a (UI only) | n/a | n/a |
