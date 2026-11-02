import { For, Show, createSignal, type JSX } from "solid-js";
import { Bug, FolderOpen, Zap, Gamepad2, HardDrive, Info, KeyRound, LoaderCircle, Palette, RefreshCw } from "lucide-solid";
import { HaloLogo } from "../components/Icons";
import { PageHeader, Toggle } from "../components/Shell";
import { api, pickFile } from "../lib/api";
import { app, attempt, setSettings, settings, toast } from "../lib/store";

const ACCENTS = [
  { name: "Violet", h: 275 },
  { name: "Pink", h: 340 },
  { name: "Ice", h: 220 },
  { name: "Mint", h: 165 },
  { name: "Gold", h: 75 },
  { name: "Ember", h: 30 },
];

function Row(props: { title: string; desc?: JSX.Element; children: JSX.Element }) {
  return (
    <div class="flex items-center gap-4 py-3 border-b last:border-0" style={{ "border-color": "var(--border)" }}>
      <div class="flex-1 min-w-0">
        <div class="font-semibold text-[13.5px]">{props.title}</div>
        <Show when={props.desc}><div class="text-faint text-[12px] mt-0.5">{props.desc}</div></Show>
      </div>
      {props.children}
    </div>
  );
}

function Section(props: { title: string; icon: any; children: JSX.Element }) {
  return (
    <section class="glass px-5 py-3">
      <div class="flex items-center gap-2 pt-1 pb-1"><props.icon size={16} class="text-accent" /><h3 class="font-semibold">{props.title}</h3></div>
      {props.children}
    </section>
  );
}

export default function Settings() {
  const [checking, setChecking] = createSignal(false);
  async function checkUpdate() {
    setChecking(true);
    const r = await attempt(() => api.prepareUpdate());
    setChecking(false);
    if (r === undefined) return;
    if (!r) toast("You're on the latest version", "ok");
  }
  async function moveData() {
    const p = await pickFile({ title: "New data folder", directory: true });
    if (!p) return;
    if ((await attempt(() => api.setDataDir(p))) !== undefined) toast("Restart Halo Client to use the new data folder. Move your files there first to keep them.", "info");
  }

  return (
    <div class="page-enter pb-2">
      <PageHeader title="Settings" />
      <div class="grid grid-cols-2 gap-4 items-start">
        <div class="flex flex-col gap-4">
          <Section title="Appearance" icon={Palette}>
            <Row title="Theme">
              <div class="seg">
                <button classList={{ active: settings.theme === "dark" }} onClick={() => setSettings("theme", "dark")}>Dark</button>
                <button classList={{ active: settings.theme === "light" }} onClick={() => setSettings("theme", "light")}>Light</button>
              </div>
            </Row>
            <Row title="Accent color">
              <div class="flex gap-1.5">
                <For each={ACCENTS}>
                  {(a) => (
                    <button
                      title={a.name}
                      aria-label={a.name}
                      class="w-6 h-6 rounded-full cursor-pointer transition-transform hover:scale-110"
                      style={{ background: `linear-gradient(135deg, oklch(0.7 0.2 ${a.h}), oklch(0.6 0.22 ${a.h + 60}))`, "box-shadow": settings.accentHue === a.h ? "0 0 0 2px var(--bg), 0 0 0 4px var(--accent)" : "none" }}
                      onClick={() => setSettings("accentHue", a.h)}
                    />
                  )}
                </For>
              </div>
            </Row>
            <Row title="Custom hue" desc={`${settings.accentHue}°`}>
              <input id="set-hue" type="range" min={0} max={359} value={settings.accentHue} class="!w-40" onInput={(e) => setSettings("accentHue", +e.currentTarget.value)} aria-label="Accent hue" />
            </Row>
            <Row title="Shooting stars" desc="Animated sky. Pauses while the game runs or the window is hidden.">
              <Toggle on={settings.animatedBackground} onChange={(v) => setSettings("animatedBackground", v)} label="Animated background" />
            </Row>
          </Section>

          <Section title="Instant Play" icon={Zap}>
            <Row title="Instant Play" desc="Keeps your last instance loaded and hidden, so Play opens it in about a second. While it waits it's muted, runs at the lowest priority in efficiency mode, and its memory is moved out of RAM.">
              <Toggle on={settings.instantPlay} onChange={(v) => setSettings("instantPlay", v)} label="Instant Play" />
            </Row>
            <Row title="Keep Halo in the tray" desc="Closing the window leaves Halo running in the system tray (the window itself is unloaded). Play from the tray icon.">
              <Toggle on={settings.runInBackground} onChange={(v) => setSettings("runInBackground", v)} label="Keep Halo in the tray" />
            </Row>
            <Row title="Start with Windows" desc="Starts quietly in the tray when you sign in, so Instant Play is ready before you open Halo.">
              <Toggle on={settings.startWithWindows} onChange={(v) => setSettings("startWithWindows", v)} label="Start with Windows" />
            </Row>
          </Section>

          <Section title="Game & performance" icon={Gamepad2}>
            <Row title="Default memory" desc={`${(settings.defaultRamMb / 1024).toFixed(1)} GB for new instances`}>
              <input id="set-ram" type="range" min={1024} max={16384} step={512} value={settings.defaultRamMb} class="!w-40" onInput={(e) => setSettings("defaultRamMb", +e.currentTarget.value)} aria-label="Default memory" />
            </Row>
            <Row title="Manage Java automatically" desc="Downloads the right Adoptium runtime per Minecraft version, verified by SHA-256">
              <Toggle on={settings.autoJava} onChange={(v) => setSettings("autoJava", v)} label="Manage Java" />
            </Row>
            <Row title="Pre-flight on open" desc="Refreshes your login and checks the last instance in the background, so Play starts right away">
              <Toggle on={settings.preflight} onChange={(v) => setSettings("preflight", v)} label="Pre-flight" />
            </Row>
            <Row title="Class-data sharing" desc="Reuses Java's loaded classes between launches. Experimental on Windows: compare the boot time with it on and off.">
              <Toggle on={settings.classDataSharing} onChange={(v) => setSettings("classDataSharing", v)} label="Class-data sharing" />
            </Row>
            <Row title="Hide launcher while playing">
              <Toggle on={settings.closeOnLaunch} onChange={(v) => setSettings("closeOnLaunch", v)} label="Hide launcher" />
            </Row>
            <Row title="Discord Rich Presence" desc={app.info?.discord === false ? "Not included in this build" : "Shows what you're playing on Discord"}>
              <Toggle on={settings.discordRpc && app.info?.discord !== false} disabled={app.info?.discord === false} onChange={(v) => setSettings("discordRpc", v)} label="Discord Rich Presence" />
            </Row>
          </Section>
        </div>

        <div class="flex flex-col gap-4">
          <Section title="Downloads & storage" icon={HardDrive}>
            <Row title="Parallel downloads" desc={`${settings.downloadThreads} at once`}>
              <input id="set-threads" type="range" min={2} max={32} value={settings.downloadThreads} class="!w-40" onInput={(e) => setSettings("downloadThreads", +e.currentTarget.value)} aria-label="Parallel downloads" />
            </Row>
            <Row title="Data folder" desc={<span class="mono break-all">{app.info?.dataDir}</span>}>
              <div class="flex gap-1.5">
                <button class="btn h-8" onClick={() => app.info && attempt(() => api.openPath(app.info!.dataDir))}><FolderOpen size={14} /></button>
                <button class="btn h-8" onClick={moveData}>Change</button>
              </div>
            </Row>
          </Section>

          <Section title="Privacy" icon={Bug}>
            <Row title="Send crash reports" desc={app.info?.sentry ? "Anonymous launcher crash reports. Never your username, tokens or files. Takes effect after a restart." : "Crash reporting isn't included in this build"}>
              <Toggle on={settings.crashReports} disabled={!app.info?.sentry} onChange={(v) => setSettings({ crashReports: v, crashReportsPrompted: true })} label="Crash reports" />
            </Row>
          </Section>

          <Section title="Advanced" icon={KeyRound}>
            <Row title="Azure app client ID" desc="Only for developers testing their own Azure app. Leave empty to use the built-in one.">
              <input id="set-msa" class="input w-48 mono text-[12px]" placeholder={app.info?.msaConfigured ? "Built-in" : "Not configured"} value={settings.msaClientId ?? ""} onChange={(e) => setSettings("msaClientId", e.currentTarget.value.trim() || null)} />
            </Row>
            <Row title="Update automatically" desc="Downloads new versions in the background and verifies them. You choose when to restart.">
              <Toggle on={settings.autoUpdate} onChange={(v) => setSettings("autoUpdate", v)} label="Update automatically" />
            </Row>
            <Row title="Show reserved ad slots" desc="Dev preview of future layout slots (ADS_ENABLED=false)">
              <Toggle on={settings.showReservedSlots} onChange={(v) => setSettings("showReservedSlots", v)} label="Show reserved slots" />
            </Row>
          </Section>

          <Section title="About" icon={Info}>
            <div class="flex items-center gap-3 py-3">
              <HaloLogo size={40} />
              <div class="flex-1">
                <div class="font-bold">Halo Client</div>
                <div class="text-faint text-[12px]">Version {app.info?.version} · Not affiliated with Mojang or Microsoft</div>
              </div>
              <button class="btn h-8" disabled={checking()} onClick={checkUpdate}>
                <Show when={checking()} fallback={<RefreshCw size={14} />}><LoaderCircle size={14} class="spin" /></Show> Check for updates
              </button>
            </div>
            <div class="flex flex-wrap gap-1.5 pb-3">
              <span class="chip">Microsoft login {app.info?.msaConfigured ? "ready" : "not configured"}</span>
              <span class="chip">CurseForge {app.info?.curseforge ? "on" : "off"}</span>
              <span class="chip">Halo HUD {app.info?.hud ? `${app.info.hud.version} (${app.info.hud.minecraft.join(", ")})` : "not bundled"}</span>
            </div>
          </Section>
        </div>
      </div>
    </div>
  );
}

