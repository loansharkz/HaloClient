---
type: project
status: active
tags: [project, minecraft, tauri, rust, solidjs]
---
# Halo Client

**What:** Fast glassy Minecraft Java launcher for Windows. Multi-account (alts), Modrinth/CurseForge browser, all four loaders, Performance profile, Halo HUD companion mod.
**Repo:** github.com/loansharkz/haloclient (`claude/inspiring-goodall-bt41fy`)
**Status:** Phase 0: UI mockup built, awaiting approval.

## Connections
- Hub: [[Minecraft Hub]]
- Tech: [[Tauri]] · [[Rust]] · [[SolidJS]] · [[Modrinth API]] · [[Microsoft Auth (MSA→XBL→XSTS)]]
- Related: [[LARPcraft]] · [[StarCore]]

## Key decisions
See `DECISIONS.md` in the repo. Short version: Windows only, Tauri + SolidJS, glassy violet with dark/light + accent picker, animated gradient, frameless window, Inter font, offline profiles, Perf profile on by default, Sentry opt-out crash reports, ads design-only.

## Blockers
- Azure app registration + Mojang Minecraft API approval (Leo)
- CurseForge API key (optional, Leo)
- Sentry DSN (optional, Leo)

## Log
- 2026-11-01: Step 0 Q&A done, DECISIONS.md written, UI mockup built.
