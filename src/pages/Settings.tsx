import { For, type JSX } from "solid-js";
import { Bug, Download, Gamepad2, Info, Palette, RefreshCw } from "lucide-solid";
import { HaloLogo } from "../components/Icons";
import { PageHeader, Toggle } from "../components/Shell";
import { APP_VERSION } from "../lib/config";
import { setSettings, settings, toast } from "../lib/store";

const ACCENTS = [
  { name: "Violet", h: 275 },
  { name: "Pink", h: 340 },
  { name: "Ice", h: 220 },
  { name: "Mint", h: 165 },
  { name: "Gold", h: 75 },
  { name: "Ember", h: 30 },
];

function Row(props: { title: string; desc?: string; children: JSX.Element }) {
  return (
    <div class="flex items-center gap-4 py-3 border-b last:border-0" style={{ "border-color": "var(--border)" }}>
      <div class="flex-1 min-w-0">
        <div class="font-semibold text-[13.5px]">{props.title}</div>
        {props.desc && <div class="text-faint text-[12px] mt-0.5">{props.desc}</div>}
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
                      class="w-6 h-6 rounded-full cursor-pointer transition-transform hover:scale-110"
                      style={{
                        background: `linear-gradient(135deg, oklch(0.7 0.2 ${a.h}), oklch(0.6 0.22 ${a.h + 60}))`,
                        "box-shadow": settings.accentHue === a.h ? "0 0 0 2px var(--bg), 0 0 0 4px var(--accent)" : "none",
                      }}
                      onClick={() => setSettings("accentHue", a.h)}
                    />
                  )}
                </For>
              </div>
            </Row>
            <Row title="Custom hue" desc={`${settings.accentHue}°`}>
              <input type="range" min={0} max={359} value={settings.accentHue} class="!w-40" onInput={(e) => setSettings("accentHue", +e.currentTarget.value)} />
            </Row>
            <Row title="Animated background" desc="Pauses automatically while the game runs">
              <Toggle on={settings.animatedBackground} onChange={(v) => setSettings("animatedBackground", v)} />
            </Row>
          </Section>

          <Section title="Game & performance" icon={Gamepad2}>
            <Row title="Default memory" desc={`${(settings.defaultRamMb / 1024).toFixed(1)} GB for new instances`}>
              <input type="range" min={1024} max={16384} step={512} value={settings.defaultRamMb} class="!w-40" onInput={(e) => setSettings("defaultRamMb", +e.currentTarget.value)} />
            </Row>
            <Row title="Manage Java automatically" desc="Downloads the right Adoptium runtime per version">
              <Toggle on={settings.autoJava} onChange={(v) => setSettings("autoJava", v)} />
            </Row>
            <Row title="Pre-flight on open" desc="Refresh tokens & verify the last instance in the background so Play is instant">
              <Toggle on={settings.preflight} onChange={(v) => setSettings("preflight", v)} />
            </Row>
            <Row title="Close launcher on game start">
              <Toggle on={settings.closeOnLaunch} onChange={(v) => setSettings("closeOnLaunch", v)} />
            </Row>
            <Row title="Discord Rich Presence">
              <Toggle on={settings.discordRpc} onChange={(v) => setSettings("discordRpc", v)} />
            </Row>
          </Section>
        </div>

        <div class="flex flex-col gap-4">
          <Section title="Downloads" icon={Download}>
            <Row title="Parallel downloads" desc={`${settings.downloadThreads} threads`}>
              <input type="range" min={2} max={32} value={settings.downloadThreads} class="!w-40" onInput={(e) => setSettings("downloadThreads", +e.currentTarget.value)} />
            </Row>
            <Row title="Data folder" desc="%APPDATA%\HaloClient">
              <button class="btn h-8" onClick={() => toast("Folder picker opens in the real app", "info")}>Change</button>
            </Row>
          </Section>

          <Section title="Privacy" icon={Bug}>
            <Row title="Send crash reports" desc="Anonymous launcher crash reports via Sentry. Never includes tokens, usernames or your files.">
              <Toggle on={settings.crashReports} onChange={(v) => setSettings("crashReports", v)} />
            </Row>
          </Section>

          <Section title="About" icon={Info}>
            <div class="flex items-center gap-3 py-3">
              <HaloLogo size={40} />
              <div class="flex-1">
                <div class="font-bold">Halo Client</div>
                <div class="text-faint text-[12px]">Version {APP_VERSION} · not affiliated with Mojang or Microsoft</div>
              </div>
              <button class="btn h-8" onClick={() => toast("You're on the latest version", "ok")}><RefreshCw size={14} /> Check for updates</button>
            </div>
            <Row title="Show reserved ad slots" desc="Dev preview of future layout slots (ADS_ENABLED=false)">
              <Toggle on={settings.showReservedSlots} onChange={(v) => setSettings("showReservedSlots", v)} />
            </Row>
          </Section>
        </div>
      </div>
    </div>
  );
}
