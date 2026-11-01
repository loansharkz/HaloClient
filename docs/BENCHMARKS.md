# Benchmarks: Play → title screen

**Status: not measured yet.** These need a Windows PC with Minecraft installed. The cloud container that built v0.1 can't download or run the game. Halo times every launch automatically (shown on the instance page and Home), so filling this in is just playing.

## How Halo measures
- Start: the moment you click Play.
- End: the first log line that means the title screen is ready (`Sound engine started`, or `OpenAL initialized` on old versions).
- Each launch is stored in the instance's boot history with whether CDS and the Performance profile were on.
- Includes account refresh and file checks, so it's the real wait a player feels.

## Method
1. Same PC, nothing else running, same Minecraft version (26.3).
2. Five launches per configuration. Report the median. The first launch after creating an instance downloads files, so exclude it.
3. Configurations:

| # | Config | How |
|---|---|---|
| A | Vanilla baseline | Vanilla instance, Settings → turn off *Tuned G1 flags* and *Class-data sharing* |
| B | Vanilla + Halo JVM | Vanilla instance, both toggles on |
| C | Fabric + Performance profile | Fabric instance with the profile, toggles on |
| D | C without CDS | Same as C with *Class-data sharing* off |

## Results

| Config | Median (s) | Runs | Notes |
|---|---|---|---|
| A |  |  |  |
| B |  |  |  |
| C |  |  |  |
| D |  |  |  |

**Keep CDS only if B < A and C < D** (from the spec). If CDS doesn't help, set the instance default `cds: false` in `src-tauri/src/instances.rs`.
