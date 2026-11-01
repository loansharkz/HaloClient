# Multiplayer test checklist

Run at the end of every phase on Windows with a real Microsoft account that owns Java Edition. Log the results in `DECISIONS.md` → *Multiplayer test log*.

## Online (real session auth)
- [ ] Vanilla instance joins an online-mode server (e.g. Hypixel), no "Failed to verify username" kick
- [ ] Fabric + Performance profile + Halo HUD joins Hypixel and Donut SMP, plays 5 min, no kicks
- [ ] NeoForge instance joins an online-mode server
- [ ] Forge instance joins an online-mode server
- [ ] Quilt instance joins an online-mode server
- [ ] Account switch (alt A → alt B) and the server shows the right username/skin
- [ ] Expired token gets silently refreshed before launch (set system clock or wait a day)
- [ ] Quick-join button lands directly on the server (1.20+ `--quickPlayMultiplayer`, older `--server`)

## Local server
- [ ] Paper/vanilla server with `online-mode=true` on localhost: MS account joins
- [ ] Same server with `online-mode=false`: offline profile joins
- [ ] Offline profile is **rejected** by the `online-mode=true` server (expected)
- [ ] LAN world opened from one instance, joined from another
