# Halo HUD

Companion Fabric mod for Halo Client (Minecraft 26.3, Java 25).

- FPS, CPS (left | right), ping
- Keystrokes (WASD, mouse buttons, jump)
- Armor and held-item durability, potion effects with timers
- Zoom: hold **C** (lowers FOV, with optional smooth camera). Toggle the HUD with **H**. Both can be rebound under Controls → Halo HUD.

Everything is display-only: nothing automates input or shows information the game hides, so it's fine on servers that allow standard PvP HUDs.

Settings are in `config/halohud.json` (toggles per element, colors, scale, zoom FOV).

Build: `./gradlew build` → `build/libs/halo-hud-<version>.jar` and `halo-hud.json` (the launcher reads it to know which Minecraft versions are supported).
