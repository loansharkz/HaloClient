import { A, useNavigate, useParams, useSearchParams } from "@solidjs/router";
import { For, Show, createEffect, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import {
  ChevronRight, CircleArrowUp, Copy, Cpu, FileArchive, Folder, FolderOpen, Gauge, Image, LoaderCircle, MemoryStick, Plus, Puzzle, RefreshCw,
  Search, Server, Settings, Sparkles, Terminal, Timer, Trash2, Wrench, X,
} from "lucide-solid";
import { ICON_CHOICES, IconTile, InstanceIcon, hueOf } from "../components/Icons";
import { Empty, Modal, Toggle } from "../components/Shell";
import { PlayButton } from "../components/Overlays";
import Motd from "../components/Motd";
import { api, fileSrc, pickFile, pickSavePath } from "../lib/api";
import { IS_TAURI } from "../lib/config";
import { LOADER_LABEL, ago, bytes, hours, seconds } from "../lib/format";
import { app, attempt, instanceById, launchInstance, pingServer, refreshInstances, settings, toast } from "../lib/store";
import type { ContentItem, Instance, ServerEntry, UpdateInfo } from "../lib/types";

type Tab = "content" | "logs" | "screenshots" | "servers" | "settings";

// ---------------------------------------------------------------------------
// Content (Modrinth-app style table)
// ---------------------------------------------------------------------------

function ContentTab(props: { inst: Instance }) {
  const nav = useNavigate();
  const [items, { refetch }] = createResource(() => props.inst.id, (id) => api.listContent(id));
  const [kind, setKind] = createSignal<"all" | ContentItem["kind"]>("all");
  const [q, setQ] = createSignal("");
  const [updates, setUpdates] = createSignal<UpdateInfo[] | null>(null);
  const [checking, setChecking] = createSignal(false);
  const [working, setWorking] = createSignal(false);

  const list = createMemo(() => (items() ?? []).filter((c) => (kind() === "all" || c.kind === kind()) && (c.name + c.fileName).toLowerCase().includes(q().toLowerCase())));
  const updateFor = (key: string) => updates()?.find((u) => u.key === key);
  const isModded = () => props.inst.loader !== "vanilla";
  const hasPerf = () => (items() ?? []).some((c) => c.meta?.perf);
  const hasHud = () => (items() ?? []).some((c) => c.meta?.hud);
  const counts = createMemo(() => {
    const c = { all: 0, mod: 0, resourcepack: 0, shader: 0 } as Record<string, number>;
    (items() ?? []).forEach((i) => {
      c.all++;
      c[i.kind]++;
    });
    return c;
  });

  async function check() {
    setChecking(true);
    const r = await attempt(() => api.checkContentUpdates(props.inst.id));
    setChecking(false);
    if (r) {
      setUpdates(r);
      toast(r.length ? `${r.length} update${r.length === 1 ? "" : "s"} available` : "Everything is up to date", r.length ? "info" : "ok");
    }
  }
  async function updateAll() {
    const u = updates() ?? [];
    setWorking(true);
    const r = await attempt(() => api.applyContentUpdates(props.inst.id, u.map((x) => [x.projectId, x.latestVersionId] as [string, string])));
    setWorking(false);
    if (r) {
      r.warnings.forEach((w) => toast(w, "warn"));
      toast(`Updated ${r.installed.length} item${r.installed.length === 1 ? "" : "s"}`, "ok");
      setUpdates([]);
      refetch();
    }
  }
  async function perf() {
    setWorking(true);
    const r = await attempt(() => api.installPerf(props.inst.id));
    setWorking(false);
    if (r) {
      r.warnings.forEach((w) => toast(w, "warn"));
      toast("Performance profile installed", "ok");
      refetch();
      refreshInstances();
    }
  }
  async function hud() {
    if (await attempt(() => api.installHud(props.inst.id), "Halo HUD added")) refetch();
  }

  return (
    <div class="flex flex-col gap-3">
      <Show when={isModded() && (!hasPerf() || (props.inst.hudSupported && !hasHud())) && items()}>
        <div class="flex gap-3">
          <Show when={(props.inst.loader === "fabric" || props.inst.loader === "quilt") && !hasPerf()}>
            <div class="glass-flat p-3 flex items-center gap-3 flex-1">
              <Gauge size={18} class="text-accent" />
              <div class="text-[12.5px] flex-1"><b>Performance profile</b> <span class="text-muted">Sodium, Lithium, ModernFix and friends. Faster boot, higher FPS.</span></div>
              <button class="btn btn-primary h-8" disabled={working()} onClick={perf}>Install</button>
            </div>
          </Show>
          <Show when={props.inst.hudSupported && !hasHud()}>
            <div class="glass-flat p-3 flex items-center gap-3 flex-1">
              <Sparkles size={18} class="text-accent" />
              <div class="text-[12.5px] flex-1"><b>Halo HUD</b> <span class="text-muted">FPS/CPS, keystrokes, zoom, armor.</span></div>
              <button class="btn h-8" onClick={hud}>Add</button>
            </div>
          </Show>
        </div>
      </Show>

      <div class="glass p-2">
        <div class="flex items-center gap-2 p-2 pb-3 flex-wrap">
          <div class="seg">
            <For each={[["all", "All"], ["mod", "Mods"], ["resourcepack", "Resource packs"], ["shader", "Shaders"]] as const}>
              {([k, label]) => (
                <button classList={{ active: kind() === k }} onClick={() => setKind(k)}>
                  {label} <span class="text-faint tabular-nums">{counts()[k]}</span>
                </button>
              )}
            </For>
          </div>
          <div class="relative flex-1 min-w-[160px]">
            <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
            <input id="content-search" class="input pl-8 h-8 w-full" placeholder="Search installed" value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
          </div>
          <button class="btn h-8" disabled={checking()} onClick={check}>
            <Show when={checking()} fallback={<RefreshCw size={14} />}><LoaderCircle size={14} class="spin" /></Show> Check updates
          </button>
          <button class="btn h-8" disabled={!updates()?.length || working()} onClick={updateAll}><CircleArrowUp size={14} /> Update all</button>
          <button class="btn btn-primary h-8" onClick={() => nav(`/browse?instance=${props.inst.id}`)}><Plus size={14} /> Add content</button>
        </div>

        <Show when={!items.loading} fallback={<div class="p-8 text-center text-muted"><LoaderCircle size={18} class="spin inline" /></div>}>
          <Show when={list().length > 0} fallback={<div class="text-center text-muted py-10 text-[13px]">{isModded() || kind() !== "mod" ? "Nothing here yet." : "Vanilla instances can't load mods, but resource packs and shaders work."} <A href={`/browse?instance=${props.inst.id}`} class="text-accent">Browse content</A></div>}>
            <div class="grid grid-cols-[38px_34px_1fr_150px_90px_36px] items-center gap-x-3 px-3 pb-1.5 text-faint text-[11px] font-semibold uppercase tracking-wider">
              <span /> <span /> <span>Name</span> <span>Version</span> <span>Source</span> <span />
            </div>
            <For each={list()}>
              {(c) => (
                <div class="grid grid-cols-[38px_34px_1fr_150px_90px_36px] items-center gap-x-3 px-3 py-2 rounded-xl hover:bg-[var(--glass)] transition-colors" classList={{ "opacity-50": !c.enabled }}>
                  <Toggle on={c.enabled} label={`Enable ${c.name}`} onChange={async (v) => { await attempt(() => api.toggleContent(props.inst.id, c.key, v)); refetch(); }} />
                  <IconTile icon={c.meta?.hud ? "sparkles" : "package"} hue={c.meta?.hud ? settings.accentHue : hueOf(c.name)} size={34} src={c.meta?.iconUrl} />
                  <div class="min-w-0">
                    <div class="flex items-center gap-2">
                      <span class="font-semibold text-[13px] truncate">{c.name}</span>
                      <Show when={c.meta?.perf}><span class="chip chip-accent" style={{ height: "18px" }}><Gauge size={10} /> perf</span></Show>
                      <Show when={c.meta?.hud}><span class="chip chip-accent" style={{ height: "18px" }}><Sparkles size={10} /> Halo</span></Show>
                      <Show when={c.meta?.dependency}><span class="chip" style={{ height: "18px" }}>dependency</span></Show>
                    </div>
                    <div class="text-faint text-[11px] mono truncate">{c.fileName} · {bytes(c.size)}</div>
                  </div>
                  <div class="text-[12px] min-w-0">
                    <div class="truncate">{c.version ?? "—"}</div>
                    <Show when={updateFor(c.key)}><div class="text-[11px] flex items-center gap-1" style={{ color: "var(--warn)" }}><CircleArrowUp size={11} /> {updateFor(c.key)!.latest}</div></Show>
                  </div>
                  <span class="chip w-fit capitalize">{c.meta?.source ?? "local"}</span>
                  <button class="btn btn-ghost btn-icon h-8 text-faint hover:!text-[var(--bad)]" aria-label={`Remove ${c.name}`} onClick={async () => { if (await attempt(() => api.removeContent(props.inst.id, c.key), `Removed ${c.name}`)) refetch(); }}>
                    <Trash2 size={15} />
                  </button>
                </div>
              )}
            </For>
          </Show>
        </Show>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

function LogsTab(props: { inst: Instance }) {
  const [q, setQ] = createSignal("");
  const [history, { refetch: refetchHistory }] = createResource(() => props.inst.id, (id) => api.gameLog(id).catch(() => []));
  // Fallback: while the game runs but nothing has streamed in, re-read its own latest.log.
  const poll = setInterval(() => {
    if (app.launch[props.inst.id]?.phase === "running" && !app.logs[props.inst.id]?.length) refetchHistory();
  }, 2000);
  onCleanup(() => clearInterval(poll));
  const lines = () => (app.logs[props.inst.id]?.length ? app.logs[props.inst.id]! : history() ?? []);
  const shown = createMemo(() => (q() ? lines().filter((l) => l.toLowerCase().includes(q().toLowerCase())) : lines()));
  let box!: HTMLPreElement;
  createEffect(() => {
    shown().length;
    if (box && box.scrollHeight - box.scrollTop - box.clientHeight < 80) queueMicrotask(() => (box.scrollTop = box.scrollHeight));
  });
  const crash = () => (app.exit?.instanceId === props.inst.id ? app.exit.crash : null);
  async function copy() {
    // Strip local user paths before sharing.
    const text = shown().join("\n").replace(/([A-Za-z]:\\Users\\)[^\\\s]+/g, "$1<user>").replace(/(\/home\/|\/Users\/)[^/\s]+/g, "$1<user>");
    try {
      await navigator.clipboard.writeText(text);
      toast("Log copied (user paths and tokens hidden)", "ok");
    } catch {
      toast("Couldn't access the clipboard", "warn");
    }
  }
  const color = (l: string) => (/\b(ERROR|FATAL)\b|Exception/.test(l) ? "var(--bad)" : /\bWARN\b/.test(l) ? "var(--warn)" : l.includes("Halo") ? "var(--accent)" : "var(--muted)");
  return (
    <div class="flex flex-col gap-3">
      <Show when={crash()}>
        <div class="glass-flat p-3 flex items-start gap-3" style={{ background: "rgb(251 191 36 / .08)", "border-color": "rgb(251 191 36 / .25)" }}>
          <Wrench size={18} color="var(--warn)" class="mt-0.5 shrink-0" />
          <div class="text-[12.5px]"><div class="font-semibold">{crash()!.title}</div><div class="text-muted mt-0.5">{crash()!.detail}</div></div>
        </div>
      </Show>
      <div class="glass overflow-hidden">
        <div class="flex items-center gap-2 p-2.5 border-b" style={{ "border-color": "var(--border)" }}>
          <div class="relative flex-1">
            <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
            <input id="log-search" class="input pl-8 h-8 w-full" placeholder="Search log" value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
          </div>
          <span class="text-faint text-[12px] tabular-nums">{shown().length} lines</span>
          <button class="btn h-8" onClick={copy}><Copy size={14} /> Copy for support</button>
          <button class="btn h-8" onClick={async () => attempt(async () => api.openPath(`${await api.instanceDir(props.inst.id)}/logs`))}><Folder size={14} /> Logs folder</button>
        </div>
        <pre ref={box} class="mono text-[11.5px] leading-[1.65] p-4 scroll h-[440px] select-text whitespace-pre-wrap break-all" style={{ background: "var(--console)" }}>
          <Show when={shown().length} fallback={<span class="text-faint">No log yet. Launch the instance and the game's output streams here.</span>}>
            <For each={shown()}>{(l) => <div style={{ color: color(l) }}>{l}</div>}</For>
          </Show>
        </pre>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Screenshots
// ---------------------------------------------------------------------------

function ScreenshotsTab(props: { inst: Instance }) {
  const [shots] = createResource(() => props.inst.id, async (id) => Promise.all((await api.screenshots(id)).map(async (s) => ({ ...s, src: await fileSrc(s.path) }))));
  const [open, setOpen] = createSignal<string | null>(null);
  return (
    <Show when={(shots() ?? []).length > 0} fallback={<Empty icon={<Image size={26} />} title="No screenshots yet">Press F2 in-game. Screenshots from this instance show up here.</Empty>}>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
        <For each={shots()}>
          {(s) => (
            <button class="glass-flat lift overflow-hidden cursor-pointer text-left" onClick={() => setOpen(s.src)}>
              <img src={s.src} alt={s.name} loading="lazy" class="w-full aspect-video object-cover" />
              <div class="px-2.5 py-1.5 text-[11.5px] text-faint truncate">{ago(s.taken)}</div>
            </button>
          )}
        </For>
      </div>
      <Modal open={!!open()} onClose={() => setOpen(null)} width={1100} label="Screenshot">
        <img src={open()!} alt="" class="w-full rounded-2xl" />
      </Modal>
    </Show>
  );
}

// ---------------------------------------------------------------------------
// Servers (servers.dat)
// ---------------------------------------------------------------------------

function ServersTab(props: { inst: Instance }) {
  const [list, { refetch, mutate }] = createResource(() => props.inst.id, (id) => api.serverList(id));
  const [name, setName] = createSignal("");
  const [ip, setIp] = createSignal("");
  createEffect(() => (list() ?? []).forEach((s) => pingServer(s.ip)));
  async function save(next: ServerEntry[]) {
    mutate(next);
    await attempt(() => api.saveServerList(props.inst.id, next));
    refetch();
  }
  return (
    <div class="flex flex-col gap-3">
      <form class="glass p-3 flex gap-2" onSubmit={(e) => { e.preventDefault(); if (!ip().trim()) return; save([...(list() ?? []), { name: name().trim() || ip().trim(), ip: ip().trim() }]); setName(""); setIp(""); }}>
        <input id="srv-name" class="input w-48" placeholder="Name" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
        <input id="srv-ip" class="input flex-1" placeholder="Server address, e.g. mc.hypixel.net" value={ip()} onInput={(e) => setIp(e.currentTarget.value)} />
        <button class="btn btn-primary" type="submit"><Plus size={15} /> Add server</button>
      </form>
      <Show when={(list() ?? []).length > 0} fallback={<Empty icon={<Server size={26} />} title="No servers saved">Servers you add here (or in-game) appear in the multiplayer list.</Empty>}>
        <div class="glass p-1.5">
          <For each={list()}>
            {(s, i) => {
              const live = () => app.serverStatus[s.ip];
              return (
                <div class="flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-[var(--glass)]">
                  <Show when={live()?.favicon ?? (s.icon ? `data:image/png;base64,${s.icon}` : null)} fallback={<div class="w-10 h-10 rounded-lg flex items-center justify-center font-bold" style={{ background: "var(--glass-strong)" }}>{s.name.slice(0, 2).toUpperCase()}</div>}>
                    {(src) => <img src={src()} width={40} height={40} class="rounded-lg" style={{ "image-rendering": "pixelated" }} alt="" />}
                  </Show>
                  <div class="min-w-0 flex-1">
                    <div class="font-semibold text-[13px]">{s.name}</div>
                    <Show when={live()?.motd.length} fallback={<div class="text-faint text-[11.5px] mono">{s.ip}</div>}>
                      <Motd lines={live()!.motd} class="mono text-[11px]" />
                    </Show>
                  </div>
                  <Show when={live()?.online}><span class="text-[12px] text-muted tabular-nums">{live()!.playersOnline}/{live()!.playersMax}{live()!.pingMs != null ? ` · ${live()!.pingMs} ms` : ""}</span></Show>
                  <button class="btn btn-ghost btn-icon h-8 text-faint" aria-label={`Remove ${s.name}`} onClick={() => save((list() ?? []).filter((_, j) => j !== i()))}><X size={14} /></button>
                  <button class="btn btn-primary h-8" onClick={() => launchInstance(props.inst.id, s.ip)}>Join</button>
                </div>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

function SettingsTab(props: { inst: Instance }) {
  const nav = useNavigate();
  const [f, setF] = createStore<Instance>({ ...props.inst });
  const [confirmDelete, setConfirmDelete] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const dirty = () => JSON.stringify({ ...f, running: 0, lastBootMs: 0, bootHistory: 0 }) !== JSON.stringify({ ...props.inst, running: 0, lastBootMs: 0, bootHistory: 0 });
  async function save() {
    setSaving(true);
    if (await attempt(() => api.updateInstance({ ...f }), "Saved")) await refreshInstances();
    setSaving(false);
  }
  const boots = () => props.inst.bootHistory.slice(-12);
  const maxBoot = () => Math.max(1, ...boots().map((b) => b.ms));

  return (
    <div class="grid grid-cols-2 gap-3 items-start">
      <div class="glass p-5 flex flex-col gap-3">
        <div class="font-semibold">General</div>
        <label class="flex flex-col gap-1.5"><span class="field-label">Name</span><input id="is-name" class="input" value={f.name} onInput={(e) => setF("name", e.currentTarget.value)} /></label>
        <label class="flex flex-col gap-1.5"><span class="field-label">Group</span><input id="is-group" class="input" placeholder="e.g. PvP, SMP, Modpacks" value={f.group ?? ""} onInput={(e) => setF("group", e.currentTarget.value || null)} /></label>
        <div>
          <span class="field-label">Icon</span>
          <div class="flex gap-1.5 flex-wrap mt-1.5">
            <For each={ICON_CHOICES}>
              {(ic) => (
                <button class="rounded-lg p-0.5 cursor-pointer" classList={{ "ring-2 ring-[var(--accent)]": f.icon === ic && !f.iconPath }} onClick={() => setF({ icon: ic, hue: (ICON_CHOICES.indexOf(ic) * 47 + 200) % 360, iconPath: null })} aria-label={ic}>
                  <IconTile icon={ic} hue={(ICON_CHOICES.indexOf(ic) * 47 + 200) % 360} size={28} />
                </button>
              )}
            </For>
            <button class="btn h-8" onClick={async () => { const p = await pickFile({ title: "Instance icon", extensions: ["png", "jpg", "jpeg", "webp"] }); if (p && (await attempt(() => api.setInstanceIcon(props.inst.id, p), "Icon updated"))) refreshInstances(); else if (!IS_TAURI) toast("Custom icons need the desktop app", "info"); }}>Custom image…</button>
          </div>
        </div>
        <label class="flex flex-col gap-1.5">
          <span class="field-label">Account for this instance</span>
          <select id="is-account" class="input" value={f.accountId ?? ""} onChange={(e) => setF("accountId", e.currentTarget.value || null)}>
            <option value="">Use the active account</option>
            <For each={app.accounts}>{(a) => <option value={a.id}>{a.username}{a.kind === "offline" ? " (offline)" : ""}</option>}</For>
          </select>
        </label>
      </div>

      <div class="glass p-5 flex flex-col gap-3">
        <div class="flex items-center gap-2"><MemoryStick size={16} class="text-accent" /><span class="font-semibold">Memory</span><div class="flex-1" /><span class="font-bold tabular-nums">{((f.ramMb ?? settings.defaultRamMb) / 1024).toFixed(1)} GB</span></div>
        <input id="is-ram" type="range" min={1024} max={16384} step={512} value={f.ramMb ?? settings.defaultRamMb} onInput={(e) => setF("ramMb", +e.currentTarget.value)} aria-label="Memory" />
        <p class="text-faint text-[12px]">4 GB suits most setups; big modpacks want 6–8 GB. More isn't faster: oversized heaps mean longer GC pauses.</p>
        <div class="flex items-center gap-2 mt-2"><Cpu size={16} class="text-accent" /><span class="font-semibold">Java</span></div>
        <div class="flex gap-2">
          <input id="is-java" class="input flex-1 mono text-[12px]" placeholder="Managed by Halo (recommended)" value={f.javaPath ?? ""} onInput={(e) => setF("javaPath", e.currentTarget.value || null)} />
          <button class="btn" onClick={async () => { const p = await pickFile({ title: "Java executable" }); if (p) setF("javaPath", p); }}>Browse…</button>
        </div>
        <label class="flex flex-col gap-1.5"><span class="field-label">Extra JVM arguments</span><input id="is-jvm" class="input mono text-[12px]" placeholder="-Dfoo=bar" value={f.jvmArgs} onInput={(e) => setF("jvmArgs", e.currentTarget.value)} /></label>
        <div class="flex items-center gap-3"><Toggle on={f.tunedFlags} onChange={(v) => setF("tunedFlags", v)} label="Tuned GC flags" /><div class="text-[12.5px]"><b>Tuned G1 flags</b> <span class="text-faint">Shorter GC pauses, smoother frames</span></div></div>
        <div class="flex items-center gap-3"><Toggle on={f.cds} onChange={(v) => setF("cds", v)} label="Class data sharing" /><div class="text-[12.5px]"><b>Class-data sharing (AppCDS)</b> <span class="text-faint">Java 19+: reuses loaded classes between launches</span></div></div>
      </div>

      <div class="glass p-5 flex flex-col gap-3">
        <div class="font-semibold">Game window & hooks</div>
        <div class="grid grid-cols-2 gap-2">
          <label class="flex flex-col gap-1.5"><span class="field-label">Width</span><input id="is-w" class="input" type="number" placeholder="Default" value={f.width ?? ""} onInput={(e) => setF("width", e.currentTarget.value ? +e.currentTarget.value : null)} /></label>
          <label class="flex flex-col gap-1.5"><span class="field-label">Height</span><input id="is-h" class="input" type="number" placeholder="Default" value={f.height ?? ""} onInput={(e) => setF("height", e.currentTarget.value ? +e.currentTarget.value : null)} /></label>
        </div>
        <label class="flex flex-col gap-1.5"><span class="field-label">Pre-launch command</span><input id="is-pre" class="input mono text-[12px]" placeholder="optional" value={f.preLaunch ?? ""} onInput={(e) => setF("preLaunch", e.currentTarget.value || null)} /></label>
        <label class="flex flex-col gap-1.5"><span class="field-label">Post-exit command</span><input id="is-post" class="input mono text-[12px]" placeholder="optional" value={f.postExit ?? ""} onInput={(e) => setF("postExit", e.currentTarget.value || null)} /></label>
      </div>

      <div class="glass p-5 flex flex-col gap-3">
        <div class="flex items-center gap-2"><Timer size={16} class="text-accent" /><span class="font-semibold">Boot time</span><div class="flex-1" /><span class="text-faint text-[12px]">Play → title screen</span></div>
        <Show when={boots().length} fallback={<p class="text-faint text-[12.5px]">Launch the instance to start measuring.</p>}>
          <div class="flex items-end gap-1.5 h-24" role="img" aria-label="Recent boot times">
            <For each={boots()}>
              {(b) => (
                <div class="flex-1 flex flex-col items-center gap-1 h-full justify-end" title={`${seconds(b.ms)}${b.cds ? " · CDS" : ""}`}>
                  <div class="w-full rounded-md" style={{ height: `${(b.ms / maxBoot()) * 100}%`, background: b.cds ? "linear-gradient(var(--accent), var(--accent-2))" : "var(--glass-strong)" }} />
                </div>
              )}
            </For>
          </div>
          <div class="text-[12px] text-muted">Latest {seconds(props.inst.lastBootMs)} · colored bars ran with class-data sharing</div>
        </Show>
      </div>

      <div class="glass p-5 flex flex-col gap-3 col-span-2">
        <div class="font-semibold">Version</div>
        <p class="text-faint text-[12.5px] -mt-2">Changing these re-installs the loader on next launch. Your mods aren't changed, so check they match.</p>
        <div class="grid grid-cols-3 gap-2">
          <label class="flex flex-col gap-1.5"><span class="field-label">Minecraft</span><input id="is-mc" class="input" value={f.mcVersion} onInput={(e) => setF("mcVersion", e.currentTarget.value)} /></label>
          <label class="flex flex-col gap-1.5"><span class="field-label">Loader</span>
            <select id="is-loader" class="input" value={f.loader} onChange={(e) => setF("loader", e.currentTarget.value as any)}>
              <For each={Object.keys(LOADER_LABEL)}>{(l) => <option value={l}>{LOADER_LABEL[l]}</option>}</For>
            </select>
          </label>
          <label class="flex flex-col gap-1.5"><span class="field-label">Loader version</span><input id="is-lv" class="input" placeholder="Recommended" value={f.loaderVersion ?? ""} onInput={(e) => setF("loaderVersion", e.currentTarget.value || null)} /></label>
        </div>
        <div class="flex gap-2 flex-wrap mt-1">
          <button class="btn" onClick={async () => { const d = await attempt(() => api.duplicateInstance(props.inst.id), "Duplicated"); if (d) { await refreshInstances(); nav(`/instance/${d.id}`); } }}><Copy size={14} /> Duplicate</button>
          <button class="btn" onClick={() => attempt(() => api.repairInstance(props.inst.id), "All game files verified")}><Wrench size={14} /> Verify & repair files</button>
          <div class="flex-1" />
          <button class="btn hover:!bg-red-500/80 hover:!text-white" onClick={() => setConfirmDelete(true)}><Trash2 size={14} /> Delete instance</button>
        </div>
      </div>

      <Show when={dirty()}>
        <div class="col-span-2 sticky bottom-0 glass p-3 flex items-center gap-3" style={{ background: "var(--glass-strong)" }}>
          <span class="text-[13px] text-muted flex-1">You have unsaved changes</span>
          <button class="btn" onClick={() => setF({ ...props.inst })}>Discard</button>
          <button class="btn btn-primary" disabled={saving()} onClick={save}>Save changes</button>
        </div>
      </Show>

      <Modal open={confirmDelete()} onClose={() => setConfirmDelete(false)} width={420} label="Delete instance">
        <div class="p-6">
          <h2 class="text-lg font-bold">Delete {props.inst.name}?</h2>
          <p class="text-muted text-[13px] mt-1">This removes its worlds, mods, screenshots and settings from this PC. It can't be undone.</p>
          <div class="flex justify-end gap-2 mt-5">
            <button class="btn" onClick={() => setConfirmDelete(false)}>Cancel</button>
            <button class="btn !bg-red-500 !text-white !border-transparent" onClick={async () => { if (await attempt(() => api.deleteInstance(props.inst.id), "Instance deleted") !== undefined) { await refreshInstances(); nav("/library"); } }}>Delete</button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

// ---------------------------------------------------------------------------

export default function InstancePage() {
  const params = useParams();
  const [search, setSearch] = useSearchParams();
  const inst = () => instanceById(params.id ?? "");
  const tab = () => (search.tab as Tab) ?? "content";
  const TABS: { id: Tab; label: string; icon: any }[] = [
    { id: "content", label: "Content", icon: Puzzle },
    { id: "logs", label: "Logs", icon: Terminal },
    { id: "screenshots", label: "Screenshots", icon: Image },
    { id: "servers", label: "Servers", icon: Server },
    { id: "settings", label: "Settings", icon: Settings },
  ];
  // Jump to logs automatically while a launch is in progress.
  createEffect(() => {
    if (app.launch[params.id ?? ""]?.phase === "preparing" && tab() === "content") setSearch({ tab: "logs" });
  });
  const speed = () => {
    const i = inst();
    return i?.lastBootMs && i.vanillaBootMs && i.vanillaBootMs > i.lastBootMs ? (i.vanillaBootMs / i.lastBootMs).toFixed(1) : null;
  };

  async function exportPack() {
    const i = inst()!;
    const path = await pickSavePath("Export as Modrinth pack", `${i.name}.mrpack`, "mrpack");
    if (!path) return toast(IS_TAURI ? "Export cancelled" : "Export opens a save dialog in the desktop app", "info");
    await attempt(() => api.exportMrpack(i.id, path), `Exported ${i.name}.mrpack`);
  }

  return (
    <Show when={inst()} fallback={<Empty title="Instance not found"><A href="/library" class="text-accent">Back to Library</A></Empty>}>
      <div class="page-enter pb-2">
        <div class="flex items-center gap-1.5 text-[12px] text-faint mb-3">
          <A href="/library" class="hover:text-[var(--text)]">Library</A> <ChevronRight size={12} /> <span class="text-muted">{inst()!.name}</span>
        </div>

        <section class="glass p-5 flex items-center gap-4 relative overflow-hidden">
          <div class="absolute inset-0 pointer-events-none" style={{ background: `radial-gradient(80% 140% at 0% 0%, oklch(0.55 0.2 ${inst()!.hue} / .35), transparent 60%)` }} />
          <div class="relative"><InstanceIcon inst={inst()!} size={68} /></div>
          <div class="relative min-w-0">
            <h1 class="text-[26px] font-bold tracking-tight truncate">{inst()!.name}</h1>
            <div class="flex gap-1.5 mt-1.5 flex-wrap">
              <span class="chip">{inst()!.mcVersion}</span>
              <span class="chip">{LOADER_LABEL[inst()!.loader]} {inst()!.loaderVersion ?? ""}</span>
              <span class="chip">{hours(inst()!.playtimeSecs)} played</span>
              <Show when={inst()!.perfProfile}><span class="chip chip-accent"><Gauge size={11} /> Performance</span></Show>
            </div>
          </div>
          <div class="flex-1" />
          <div class="relative glass-flat px-4 py-2.5 text-center mr-1 min-w-[132px]">
            <div class="stat-label flex items-center gap-1 justify-center"><Timer size={11} /> Play → title</div>
            <div class="text-[22px] font-bold leading-tight tabular-nums">{seconds(inst()!.lastBootMs)}</div>
            <div class="text-[11px] text-faint">{speed() ? `${speed()}× faster than vanilla` : app.launch[inst()!.id]?.phase === "preparing" ? app.launch[inst()!.id]!.message : "last launch"}</div>
          </div>
          <button class="btn btn-icon relative" title="Open folder" aria-label="Open folder" onClick={async () => attempt(async () => api.openPath(await api.instanceDir(inst()!.id)))}><FolderOpen size={16} /></button>
          <button class="btn btn-icon relative" title="Export .mrpack" aria-label="Export .mrpack" onClick={exportPack}><FileArchive size={16} /></button>
          <div class="relative"><PlayButton instanceId={inst()!.id} /></div>
        </section>

        <div class="tabs mt-4 mb-3" role="tablist">
          <For each={TABS}>
            {(t) => (
              <button role="tab" aria-selected={tab() === t.id} classList={{ active: tab() === t.id }} onClick={() => setSearch({ tab: t.id })}>
                <t.icon size={15} /> {t.label}
              </button>
            )}
          </For>
        </div>

        <Show when={tab() === "content"}><ContentTab inst={inst()!} /></Show>
        <Show when={tab() === "logs"}><LogsTab inst={inst()!} /></Show>
        <Show when={tab() === "screenshots"}><ScreenshotsTab inst={inst()!} /></Show>
        <Show when={tab() === "servers"}><ServersTab inst={inst()!} /></Show>
        <Show when={tab() === "settings"}>
          {/* Re-key on id so the form resets when switching instances */}
          <For each={[inst()!.id]}>{() => <SettingsTab inst={inst()!} />}</For>
        </Show>
      </div>
    </Show>
  );
}

