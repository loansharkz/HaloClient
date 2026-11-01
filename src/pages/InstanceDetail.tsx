import { A, useParams } from "@solidjs/router";
import { For, Show, createMemo, createSignal } from "solid-js";
import {
  ChevronRight, CircleArrowUp, Copy, Cpu, FileArchive, Folder, Gauge, Image, MemoryStick, Puzzle, RefreshCw, Search, Settings, Sparkles, Terminal, Timer, Trash2, TriangleAlert,
} from "lucide-solid";
import { InstanceIcon, ModIcon } from "../components/Icons";
import { Toggle } from "../components/Shell";
import { LOADER_LABEL, MOCK_LOG } from "../lib/mock";
import { app, setApp, toast } from "../lib/store";
import { PlayButton } from "./Home";

type Tab = "content" | "logs" | "screenshots" | "settings";

export default function InstanceDetail() {
  const params = useParams();
  const inst = createMemo(() => app.instances.find((i) => i.id === params.id) ?? app.instances[0]);
  const [tab, setTab] = createSignal<Tab>("content");
  const [logQ, setLogQ] = createSignal("");
  const set = (...args: any[]) => (setApp as any)("instances", (i: any) => i.id === inst().id, ...args);

  const TABS: { id: Tab; label: string; icon: any }[] = [
    { id: "content", label: "Content", icon: Puzzle },
    { id: "logs", label: "Logs", icon: Terminal },
    { id: "screenshots", label: "Screenshots", icon: Image },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  const logLines = createMemo(() => MOCK_LOG.split("\n").filter((l) => l.toLowerCase().includes(logQ().toLowerCase())));

  return (
    <div class="page-enter pb-2">
      <div class="flex items-center gap-1.5 text-[12px] text-faint mb-3">
        <A href="/instances" class="hover:text-[var(--text)]">Instances</A> <ChevronRight size={12} /> <span class="text-muted">{inst().name}</span>
      </div>

      <section class="glass p-5 flex items-center gap-4 relative overflow-hidden">
        <div class="absolute inset-0 pointer-events-none" style={{ background: `radial-gradient(80% 140% at 0% 0%, oklch(0.55 0.2 ${inst().hue} / .35), transparent 60%)` }} />
        <InstanceIcon inst={inst()} size={68} />
        <div class="relative">
          <h1 class="text-[26px] font-bold tracking-tight">{inst().name}</h1>
          <div class="flex gap-1.5 mt-1.5">
            <span class="chip">{inst().mcVersion}</span>
            <span class="chip">{LOADER_LABEL[inst().loader]} {inst().loaderVersion}</span>
            <span class="chip">{inst().java}</span>
            <Show when={inst().perfProfile}><span class="chip chip-accent"><Gauge size={11} /> Performance</span></Show>
          </div>
        </div>
        <div class="flex-1" />
        <Show when={inst().lastBootMs}>
          <div class="relative glass-flat px-4 py-2.5 text-center mr-2">
            <div class="text-faint text-[10.5px] font-semibold uppercase tracking-wider flex items-center gap-1 justify-center"><Timer size={11} /> Click → title screen</div>
            <div class="text-[22px] font-bold leading-tight">{(inst().lastBootMs! / 1000).toFixed(1)}s</div>
            <Show when={inst().vanillaBootMs}>
              <div class="text-[11px] text-faint">vanilla {(inst().vanillaBootMs! / 1000).toFixed(1)}s</div>
            </Show>
          </div>
        </Show>
        <button class="btn btn-icon relative" title="Open folder" onClick={() => toast("Opens the instance folder in Explorer", "info")}><Folder size={16} /></button>
        <button class="btn btn-icon relative" title="Export .mrpack" onClick={() => toast("Exported Halo PvP.mrpack", "ok")}><FileArchive size={16} /></button>
        <div class="relative"><PlayButton instanceId={inst().id} /></div>
      </section>

      <div class="seg mt-4 mb-3">
        <For each={TABS}>
          {(t) => (
            <button classList={{ active: tab() === t.id }} onClick={() => setTab(t.id)}>
              <t.icon size={14} /> {t.label}
            </button>
          )}
        </For>
      </div>

      <Show when={tab() === "content"}>
        <div class="glass p-2">
          <div class="flex items-center gap-2 p-2 pb-3">
            <span class="font-semibold">{inst().mods.length} installed</span>
            <span class="text-faint text-[12px]">{inst().mods.filter((m) => m.enabled).length} enabled</span>
            <div class="flex-1" />
            <button class="btn h-8" onClick={() => toast("Checked Modrinth: 1 update available", "info")}><RefreshCw size={14} /> Check updates</button>
            <button class="btn h-8" disabled={!inst().mods.some((m) => m.update)} onClick={() => { set("mods", (m: any) => !!m.update, (m: any) => ({ ...m, version: m.update, update: undefined })); toast("Updated all mods", "ok"); }}>
              <CircleArrowUp size={14} /> Update all
            </button>
            <A href="/browse" class="btn btn-primary h-8"><Puzzle size={14} /> Add content</A>
          </div>
          <Show when={inst().mods.length === 0}>
            <div class="text-center text-muted py-10">Vanilla instance. No mods. <A href="/browse" class="text-accent">Browse content</A></div>
          </Show>
          <For each={inst().mods}>
            {(m) => (
              <div class="flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-[var(--glass)] transition-colors" classList={{ "opacity-50": !m.enabled }}>
                <Toggle on={m.enabled} onChange={(v) => set("mods", (x: any) => x.id === m.id, "enabled", v)} label={m.name} />
                <ModIcon slug={m.slug} name={m.name} size={34} icon={m.hud ? "sparkles" : undefined} hue={m.hud ? 275 : undefined} />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span class="font-semibold text-[13px]">{m.name}</span>
                    <Show when={m.perf}><span class="chip chip-accent" style={{ height: "18px" }}><Gauge size={10} /> perf</span></Show>
                    <Show when={m.hud}><span class="chip chip-accent" style={{ height: "18px" }}><Sparkles size={10} /> Halo</span></Show>
                  </div>
                  <div class="text-faint text-[11.5px] mono">{m.enabled ? "" : "(disabled) "}{m.id}-{m.version}.jar{m.enabled ? "" : ".disabled"}</div>
                </div>
                <Show when={m.update}><span class="chip chip-warn"><CircleArrowUp size={11} /> {m.update}</span></Show>
                <button class="btn btn-ghost btn-icon h-8 text-faint hover:!text-[var(--bad)]" onClick={() => { set("mods", (l: any[]) => l.filter((x) => x.id !== m.id)); toast(`Removed ${m.name}`, "info"); }}>
                  <Trash2 size={15} />
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>

      <Show when={tab() === "logs"}>
        <div class="glass-flat p-3 mb-3 flex items-start gap-3" style={{ background: "rgb(251 191 36 / .08)", "border-color": "rgb(251 191 36 / .25)" }}>
          <TriangleAlert size={18} color="var(--warn)" class="mt-0.5 shrink-0" />
          <div class="text-[12.5px]">
            <div class="font-semibold">Crash analyzer: last session exited normally</div>
            <div class="text-muted mt-0.5">Example of a detected issue: <i>"Iris 1.9.6 requires Sodium ≥ 0.7. Install it?"</i> Detected causes link to a one-click fix (install dependency, switch Java, disable a mod).</div>
          </div>
        </div>
        <div class="glass overflow-hidden">
          <div class="flex items-center gap-2 p-2.5 border-b" style={{ "border-color": "var(--border)" }}>
            <div class="relative flex-1">
              <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
              <input class="input pl-8 h-8 w-full" placeholder="Search log" value={logQ()} onInput={(e) => setLogQ(e.currentTarget.value)} />
            </div>
            <button class="btn h-8" onClick={() => { navigator.clipboard?.writeText(MOCK_LOG).catch(() => {}); toast("Log copied (tokens & paths redacted)", "ok"); }}>
              <Copy size={14} /> Copy for support
            </button>
          </div>
          <pre class="mono text-[11.5px] leading-[1.65] p-4 scroll max-h-[380px] select-text" style={{ background: "var(--console)" }}>
            <For each={logLines()}>
              {(l) => (
                <div style={{ color: l.includes("WARN") ? "var(--warn)" : l.includes("[Halo") ? "var(--accent)" : "var(--muted)" }}>{l}</div>
              )}
            </For>
          </pre>
        </div>
      </Show>

      <Show when={tab() === "screenshots"}>
        <Show when={inst().screenshots > 0} fallback={<div class="glass p-10 text-center text-muted">No screenshots yet. Press F2 in-game.</div>}>
          <div class="grid grid-cols-4 gap-3">
            <For each={Array.from({ length: Math.min(inst().screenshots, 12) }, (_, k) => k)}>
              {(k) => (
                <div class="glass-flat glass-hover lift aspect-video cursor-pointer overflow-hidden" style={{ background: `linear-gradient(${k * 37}deg, oklch(0.55 0.15 ${(inst().hue + k * 23) % 360}), oklch(0.35 0.12 ${(inst().hue + k * 23 + 80) % 360}))` }} />
              )}
            </For>
          </div>
        </Show>
      </Show>

      <Show when={tab() === "settings"}>
        <div class="grid grid-cols-2 gap-3">
          <div class="glass p-5">
            <div class="flex items-center gap-2 mb-1"><MemoryStick size={16} class="text-accent" /><span class="font-semibold">Memory</span><div class="flex-1" /><span class="font-bold">{(inst().ramMb / 1024).toFixed(1)} GB</span></div>
            <p class="text-faint text-[12px] mb-3">4 GB is right for most setups. Big modpacks want 6–8 GB. More isn't faster.</p>
            <input type="range" min={1024} max={16384} step={512} value={inst().ramMb} onInput={(e) => set("ramMb", +e.currentTarget.value)} />
          </div>
          <div class="glass p-5">
            <div class="flex items-center gap-2 mb-3"><Cpu size={16} class="text-accent" /><span class="font-semibold">Java runtime</span></div>
            <select class="input w-full" value={inst().java} onChange={(e) => set("java", e.currentTarget.value)}>
              <option>Java 21 (managed)</option><option>Java 17 (managed)</option><option>Java 25 (managed)</option><option>Custom path…</option>
            </select>
            <p class="text-faint text-[12px] mt-2">Managed runtimes are downloaded from Adoptium and hash-verified.</p>
          </div>
          <div class="glass p-5 col-span-2">
            <div class="font-semibold mb-2">JVM arguments</div>
            <input class="input w-full mono text-[12px]" value="-XX:+UseZGC -XX:+ZGenerational -XX:SharedArchiveFile=halo.jsa -XX:+AlwaysPreTouch" />
            <div class="grid grid-cols-3 gap-3 mt-4">
              <label class="flex flex-col gap-1.5"><span class="text-[12px] font-semibold text-muted">Window size</span><input class="input" value="1280 × 720" /></label>
              <label class="flex flex-col gap-1.5"><span class="text-[12px] font-semibold text-muted">Pre-launch command</span><input class="input mono text-[12px]" placeholder="optional" /></label>
              <label class="flex flex-col gap-1.5"><span class="text-[12px] font-semibold text-muted">Post-exit command</span><input class="input mono text-[12px]" placeholder="optional" /></label>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
