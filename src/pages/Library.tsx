import { useNavigate, useSearchParams } from "@solidjs/router";
import { For, Show, createEffect, createMemo, createResource, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { Copy, FileArchive, FolderOpen, Gauge, Import, LoaderCircle, Plus, Search, Sparkles, Timer } from "lucide-solid";
import { ICON_CHOICES, IconTile, InstanceIcon } from "../components/Icons";
import { Empty, Modal, PageHeader, Toggle } from "../components/Shell";
import { PlayButton } from "../components/Overlays";
import { api, pickFile } from "../lib/api";
import { IS_TAURI } from "../lib/config";
import { LOADER_LABEL, PERF_STACK, ago, hours, seconds } from "../lib/format";
import { app, attempt, refreshInstances, setApp, toast } from "../lib/store";
import type { Instance, Loader } from "../lib/types";

const LOADERS: Loader[] = ["vanilla", "fabric", "quilt", "forge", "neoforge"];

function NewInstance(props: { open: boolean; onClose: () => void }) {
  const nav = useNavigate();
  const [tab, setTab] = createSignal<"create" | "import">("create");
  const [snapshots, setSnapshots] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [f, setF] = createStore({
    name: "",
    mc: "",
    loader: "fabric" as Loader,
    loaderVersion: "",
    perf: Object.fromEntries(PERF_STACK.map((m) => [m.slug, true])) as Record<string, boolean>,
    hud: true,
    icon: "box",
    hue: 275,
  });
  const [versions] = createResource(() => props.open, () => api.gameVersions().catch(() => []));
  const shown = createMemo(() => (versions() ?? []).filter((v) => snapshots() || v.kind === "release"));
  createEffect(() => {
    if (!f.mc && shown().length) setF("mc", shown()[0].id);
  });
  const [loaderVersions] = createResource(
    () => (props.open && f.mc && f.loader !== "vanilla" ? { loader: f.loader, mc: f.mc } : null),
    (k) => api.loaderVersions(k.loader, k.mc).catch((e) => {
      toast(String(e), "warn");
      return [];
    }),
  );
  createEffect(() => {
    const list = loaderVersions();
    if (list) setF("loaderVersion", list.find((v) => v.recommended)?.version ?? list[0]?.version ?? "");
  });
  const perfAvailable = () => f.loader === "fabric" || f.loader === "quilt";
  const hudAvailable = () => f.loader === "fabric" && !!app.info?.hud?.minecraft.includes(f.mc);

  async function create() {
    setBusy(true);
    const r = await attempt(() =>
      api.createInstance({
        name: f.name.trim() || `${LOADER_LABEL[f.loader]} ${f.mc}`,
        mcVersion: f.mc,
        loader: f.loader,
        loaderVersion: f.loader === "vanilla" ? null : f.loaderVersion || null,
        perfMods: perfAvailable() ? PERF_STACK.filter((m) => f.perf[m.slug]).map((m) => m.slug) : [],
        haloHud: hudAvailable() && f.hud,
        icon: f.icon,
        hue: f.hue,
      }),
    );
    setBusy(false);
    if (!r) return;
    await refreshInstances();
    setApp("selectedInstanceId", r.instance.id);
    r.warnings.forEach((w) => toast(w, "warn"));
    toast(`Created ${r.instance.name}`, "ok");
    props.onClose();
    nav(`/instance/${r.instance.id}`);
  }

  async function doImport(kind: "mrpack" | "prism" | "vanilla") {
    if (!IS_TAURI) return toast("Importing opens a file picker in the desktop app.", "info");
    let r: Instance | undefined;
    if (kind === "mrpack") {
      const p = await pickFile({ title: "Modrinth modpack", extensions: ["mrpack"] });
      if (p) r = await attempt(() => api.importMrpack(p));
    } else if (kind === "prism") {
      const p = await pickFile({ title: "Prism / MultiMC instance folder", directory: true });
      if (p) r = await attempt(() => api.importPrism(p));
    } else {
      const p = (await pickFile({ title: ".minecraft folder", directory: true })) ?? (await api.defaultMinecraftDir());
      if (p) r = await attempt(() => api.importVanilla(p, f.mc || shown()[0]?.id, "Imported .minecraft"));
    }
    if (r) {
      await refreshInstances();
      toast(`Imported ${r.name}`, "ok");
      props.onClose();
      nav(`/instance/${r.id}`);
    }
  }

  return (
    <Modal open={props.open} onClose={props.onClose} width={580} label="New instance">
      <div class="p-6">
        <div class="flex items-center mb-5">
          <h2 class="text-lg font-bold">New instance</h2>
          <div class="flex-1" />
          <div class="seg">
            <button classList={{ active: tab() === "create" }} onClick={() => setTab("create")}><Plus size={13} /> Create</button>
            <button classList={{ active: tab() === "import" }} onClick={() => setTab("import")}><Import size={13} /> Import</button>
          </div>
        </div>

        <Show when={tab() === "create"}>
          <div class="flex flex-col gap-4">
            <div class="flex gap-3 items-end">
              <IconTile icon={f.icon} hue={f.hue} size={56} />
              <label class="flex flex-col gap-1.5 flex-1">
                <span class="field-label">Name</span>
                <input id="ni-name" class="input" placeholder={`${LOADER_LABEL[f.loader]} ${f.mc}`} value={f.name} onInput={(e) => setF("name", e.currentTarget.value)} />
              </label>
            </div>
            <div class="flex gap-1.5 flex-wrap">
              <For each={ICON_CHOICES}>
                {(ic) => (
                  <button class="rounded-lg p-0.5 cursor-pointer" classList={{ "ring-2 ring-[var(--accent)]": f.icon === ic }} onClick={() => setF({ icon: ic, hue: (ICON_CHOICES.indexOf(ic) * 47 + 200) % 360 })} aria-label={ic}>
                    <IconTile icon={ic} hue={(ICON_CHOICES.indexOf(ic) * 47 + 200) % 360} size={28} />
                  </button>
                )}
              </For>
            </div>
            <div>
              <span class="field-label">Mod loader</span>
              <div class="grid grid-cols-5 gap-2 mt-1.5">
                <For each={LOADERS}>
                  {(l) => (
                    <button
                      class="glass-flat glass-hover py-2.5 text-[12.5px] font-semibold cursor-pointer"
                      classList={{ "!border-[var(--accent)] !bg-[var(--accent-soft)] text-accent": f.loader === l }}
                      onClick={() => setF({ loader: l })}
                    >
                      {LOADER_LABEL[l]}
                    </button>
                  )}
                </For>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <label class="flex flex-col gap-1.5">
                <span class="field-label flex items-center">Minecraft version <span class="flex-1" />
                  <span class="flex items-center gap-1.5 normal-case font-medium">Snapshots <Toggle on={snapshots()} onChange={setSnapshots} label="Show snapshots" /></span>
                </span>
                <select id="ni-mc" class="input" value={f.mc} onChange={(e) => setF("mc", e.currentTarget.value)}>
                  <Show when={versions.loading}><option>Loading…</option></Show>
                  <For each={shown()}>{(v) => <option value={v.id}>{v.id}{v.kind !== "release" ? ` (${v.kind})` : ""}</option>}</For>
                </select>
              </label>
              <label class="flex flex-col gap-1.5">
                <span class="field-label">Loader version</span>
                <select id="ni-lv" class="input" disabled={f.loader === "vanilla" || loaderVersions.loading} value={f.loaderVersion} onChange={(e) => setF("loaderVersion", e.currentTarget.value)}>
                  <Show when={f.loader === "vanilla"}><option>Not needed</option></Show>
                  <Show when={loaderVersions.loading}><option>Loading…</option></Show>
                  <For each={loaderVersions() ?? []}>{(v) => <option value={v.version}>{v.version}{v.recommended ? " (recommended)" : ""}</option>}</For>
                </select>
              </label>
            </div>

            <Show
              when={perfAvailable()}
              fallback={<div class="glass-flat p-3 text-[12px] text-muted flex gap-2 items-center"><Gauge size={15} /> The Performance profile is for Fabric and Quilt. {LOADER_LABEL[f.loader]} instances still get tuned Java flags and class-data sharing.</div>}
            >
              <div class="glass-flat p-4">
                <div class="flex items-center gap-2 mb-1">
                  <Gauge size={16} class="text-accent" />
                  <span class="font-semibold">Performance profile</span>
                  <span class="chip chip-ok">client-side · server-safe</span>
                </div>
                <p class="text-muted text-[12px] mb-3">Installed from Modrinth for {f.mc || "this version"}. Mods without a build for it yet are skipped.</p>
                <div class="grid grid-cols-2 gap-x-4 gap-y-2.5">
                  <For each={PERF_STACK}>
                    {(m) => (
                      <div class="flex items-center gap-2.5">
                        <Toggle on={f.perf[m.slug]} onChange={(v) => setF("perf", m.slug, v)} label={m.name} />
                        <div class="min-w-0">
                          <div class="text-[12.5px] font-semibold">{m.name}</div>
                          <div class="text-faint text-[11px] truncate" title={m.why}>{m.why}</div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
                <div class="flex items-center gap-2.5 mt-3 pt-3 border-t" style={{ "border-color": "var(--border)" }}>
                  <Toggle on={f.hud && hudAvailable()} disabled={!hudAvailable()} onChange={(v) => setF("hud", v)} label="Halo HUD" />
                  <Sparkles size={14} class="text-accent" />
                  <div class="text-[12.5px]">
                    <b>Halo HUD</b>{" "}
                    <span class="text-faint">{hudAvailable() ? "FPS/CPS, keystrokes, zoom, armor HUD" : `Fabric on ${app.info?.hud?.minecraft.join(", ") ?? "supported versions"} only`}</span>
                  </div>
                </div>
              </div>
            </Show>

            <div class="flex justify-end gap-2 mt-1">
              <button class="btn" onClick={props.onClose}>Cancel</button>
              <button class="btn btn-primary" disabled={busy() || !f.mc || (f.loader !== "vanilla" && !f.loaderVersion)} onClick={create}>
                <Show when={busy()} fallback={<Plus size={15} />}><LoaderCircle size={15} class="spin" /></Show>
                {busy() ? "Creating…" : "Create instance"}
              </button>
            </div>
          </div>
        </Show>

        <Show when={tab() === "import"}>
          <div class="grid grid-cols-3 gap-3">
            <For each={[
              { t: "Modrinth pack", d: ".mrpack file", icon: FileArchive, k: "mrpack" as const },
              { t: "Prism / MultiMC", d: "An instance folder", icon: Import, k: "prism" as const },
              { t: "Vanilla .minecraft", d: "Worlds, packs, settings", icon: FolderOpen, k: "vanilla" as const },
            ]}>
              {(o) => (
                <button class="glass-flat glass-hover lift p-4 text-left cursor-pointer" onClick={() => doImport(o.k)}>
                  <o.icon size={20} class="text-accent mb-2" />
                  <div class="font-semibold">{o.t}</div>
                  <div class="text-faint text-[12px]">{o.d}</div>
                </button>
              )}
            </For>
          </div>
          <p class="text-faint text-[11.5px] mt-4">Vanilla import copies into a new {f.mc || "latest"} instance. Your original .minecraft isn't changed. CurseForge .zip import needs a CurseForge API key.</p>
        </Show>
      </div>
    </Modal>
  );
}

export default function Library() {
  const nav = useNavigate();
  const [params, setParams] = useSearchParams();
  const [q, setQ] = createSignal("");
  const [sort, setSort] = createSignal<"played" | "name" | "playtime">("played");
  const [group, setGroup] = createSignal<string | null>(null);
  const open = () => params.new === "1";
  const groups = createMemo(() => [...new Set(app.instances.map((i) => i.group).filter(Boolean) as string[])]);
  const list = createMemo(() => {
    let l = app.instances.filter((i) => i.name.toLowerCase().includes(q().toLowerCase()) && (!group() || i.group === group()));
    if (sort() === "name") l = [...l].sort((a, b) => a.name.localeCompare(b.name));
    if (sort() === "playtime") l = [...l].sort((a, b) => b.playtimeSecs - a.playtimeSecs);
    return l;
  });

  return (
    <div class="page-enter pb-2">
      <PageHeader title="Library" sub={`${app.instances.length} instance${app.instances.length === 1 ? "" : "s"} · shared files are downloaded once`}>
        <div class="relative">
          <Search size={15} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
          <input id="lib-search" class="input pl-9 w-52" placeholder="Search instances" value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
        </div>
        <select id="lib-sort" class="input" value={sort()} onChange={(e) => setSort(e.currentTarget.value as any)} aria-label="Sort">
          <option value="played">Last played</option>
          <option value="name">Name</option>
          <option value="playtime">Playtime</option>
        </select>
        <button class="btn btn-primary" onClick={() => setParams({ new: "1" })}><Plus size={16} /> New instance</button>
      </PageHeader>

      <Show when={groups().length > 0}>
        <div class="flex gap-1.5 mb-4">
          <button class="chip cursor-pointer" classList={{ "chip-accent": !group() }} onClick={() => setGroup(null)}>All</button>
          <For each={groups()}>{(g) => <button class="chip cursor-pointer" classList={{ "chip-accent": group() === g }} onClick={() => setGroup(g)}>{g}</button>}</For>
        </div>
      </Show>

      <Show when={app.instances.length > 0} fallback={<Empty icon={<Plus size={28} />} title="No instances yet">Create one to get playing, or import a Modrinth pack, Prism/MultiMC instance or your .minecraft folder.</Empty>}>
        <div class="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-3">
          <For each={list()}>
            {(i) => (
              <div class="glass glass-hover lift p-4 cursor-pointer flex flex-col gap-3.5 group" onClick={() => nav(`/instance/${i.id}`)}>
                <div class="flex items-center gap-3">
                  <InstanceIcon inst={i} size={48} />
                  <div class="min-w-0 flex-1">
                    <div class="font-semibold text-[15px] truncate">{i.name}</div>
                    <div class="text-faint text-[12px]">{i.mcVersion} · {LOADER_LABEL[i.loader]} {i.loaderVersion ?? ""}</div>
                  </div>
                </div>
                <div class="flex flex-wrap gap-1.5">
                  <Show when={i.perfProfile}><span class="chip chip-accent"><Gauge size={11} /> Performance</span></Show>
                  <Show when={i.lastBootMs}><span class="chip"><Timer size={11} /> {seconds(i.lastBootMs)} boot</span></Show>
                  <span class="chip">{hours(i.playtimeSecs)} played</span>
                </div>
                <div class="flex items-center gap-1">
                  <span class="text-faint text-[12px]">{ago(i.lastPlayed)}</span>
                  <div class="flex-1" />
                  <button class="btn btn-ghost btn-icon h-8 opacity-0 group-hover:opacity-100" title="Duplicate" onClick={async (e) => { e.stopPropagation(); if (await attempt(() => api.duplicateInstance(i.id), `Duplicated ${i.name}`)) refreshInstances(); }}><Copy size={14} /></button>
                  <button class="btn btn-ghost btn-icon h-8 opacity-0 group-hover:opacity-100" title="Open folder" onClick={async (e) => { e.stopPropagation(); const d = await api.instanceDir(i.id); attempt(() => api.openPath(d)); }}><FolderOpen size={14} /></button>
                  <PlayButton instanceId={i.id} size="md" />
                </div>
              </div>
            )}
          </For>
          <button class="glass-flat glass-hover lift flex flex-col items-center justify-center gap-2 min-h-[150px] cursor-pointer text-muted" style={{ "border-style": "dashed" }} onClick={() => setParams({ new: "1" })}>
            <Plus size={24} />
            <span class="font-semibold text-[13px]">New instance</span>
          </button>
        </div>
      </Show>
      <NewInstance open={open()} onClose={() => setParams({ new: undefined })} />
    </div>
  );
}

