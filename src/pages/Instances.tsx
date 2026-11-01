import { useNavigate } from "@solidjs/router";
import { For, Show, createMemo, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { FileArchive, Gauge, Import, Plus, Search, Sparkles, Timer } from "lucide-solid";
import { IconTile } from "../components/Icons";
import { Modal, PageHeader, Toggle } from "../components/Shell";
import { LOADER_LABEL, MC_VERSIONS, PERF_STACK, type Instance, type Loader } from "../lib/mock";
import { app, setApp, settings, toast } from "../lib/store";
import { PlayButton } from "./Home";

const LOADER_VERSIONS: Record<Loader, string[]> = {
  vanilla: [],
  fabric: ["0.17.3 (recommended)", "0.17.2", "0.16.14"],
  quilt: ["0.29.2 (recommended)", "0.29.1"],
  forge: ["60.0.12 (recommended)", "60.0.10", "47.4.0"],
  neoforge: ["21.10.38 (recommended)", "21.10.30", "21.1.209"],
};

function NewInstance(props: { open: boolean; onClose: () => void }) {
  const [tab, setTab] = createSignal<"create" | "import">("create");
  const [f, setF] = createStore({
    name: "",
    mc: MC_VERSIONS[0],
    loader: "fabric" as Loader,
    loaderV: LOADER_VERSIONS.fabric[0],
    perf: Object.fromEntries(PERF_STACK.map((m) => [m.id, true])) as Record<string, boolean>,
    hud: true,
  });
  const perfAvailable = () => f.loader === "fabric" || f.loader === "quilt";

  const create = () => {
    const id = `i${Date.now()}`;
    const hue = Math.floor(Math.random() * 360);
    const perfOn = perfAvailable() ? PERF_STACK.filter((m) => f.perf[m.id]) : [];
    const inst: Instance = {
      id, name: f.name || `${LOADER_LABEL[f.loader]} ${f.mc}`, mcVersion: f.mc, loader: f.loader,
      loaderVersion: f.loaderV.split(" ")[0] ?? "", hue, icon: "box", playtimeH: 0, lastPlayed: "Never",
      ramMb: settings.defaultRamMb, java: "Java 21 (managed)", lastBootMs: null, vanillaBootMs: null,
      perfProfile: perfOn.length > 0, screenshots: 0,
      mods: [
        ...perfOn.map((m) => ({ id: m.id, name: m.name, version: "latest", enabled: true, perf: true })),
        ...(perfAvailable() && f.hud && f.loader === "fabric" ? [{ id: "halo-hud", name: "Halo HUD", version: "0.1.0", enabled: true, hud: true }] : []),
      ],
    };
    setApp("instances", (l) => [inst, ...l]);
    setApp("selectedInstanceId", id);
    toast(`Created ${inst.name}${perfOn.length ? ` with ${perfOn.length} performance mods` : ""}`, "ok");
    props.onClose();
  };

  return (
    <Modal open={props.open} onClose={props.onClose} width={560}>
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
            <label class="flex flex-col gap-1.5">
              <span class="text-[12px] font-semibold text-muted">Name</span>
              <input class="input" placeholder={`${LOADER_LABEL[f.loader]} ${f.mc}`} value={f.name} onInput={(e) => setF("name", e.currentTarget.value)} />
            </label>
            <div>
              <span class="text-[12px] font-semibold text-muted">Mod loader</span>
              <div class="grid grid-cols-5 gap-2 mt-1.5">
                <For each={Object.keys(LOADER_LABEL) as Loader[]}>
                  {(l) => (
                    <button
                      class="glass-flat glass-hover py-2.5 text-[12.5px] font-semibold cursor-pointer"
                      classList={{ "!border-[var(--accent)] !bg-[var(--accent-soft)] text-accent": f.loader === l }}
                      onClick={() => setF({ loader: l, loaderV: LOADER_VERSIONS[l][0] ?? "" })}
                    >
                      {LOADER_LABEL[l]}
                    </button>
                  )}
                </For>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <label class="flex flex-col gap-1.5">
                <span class="text-[12px] font-semibold text-muted">Minecraft version</span>
                <select class="input" value={f.mc} onChange={(e) => setF("mc", e.currentTarget.value)}>
                  <For each={MC_VERSIONS}>{(v) => <option>{v}</option>}</For>
                </select>
              </label>
              <label class="flex flex-col gap-1.5">
                <span class="text-[12px] font-semibold text-muted">Loader version</span>
                <select class="input" disabled={f.loader === "vanilla"} value={f.loaderV} onChange={(e) => setF("loaderV", e.currentTarget.value)}>
                  <Show when={f.loader === "vanilla"}><option>n/a</option></Show>
                  <For each={LOADER_VERSIONS[f.loader]}>{(v) => <option>{v}</option>}</For>
                </select>
              </label>
            </div>

            <Show when={perfAvailable()} fallback={
              <div class="glass-flat p-3 text-[12px] text-muted flex gap-2 items-center">
                <Gauge size={15} /> The Performance profile is for Fabric/Quilt. {LOADER_LABEL[f.loader]} instances still get tuned Java flags + CDS.
              </div>
            }>
              <div class="glass-flat p-4">
                <div class="flex items-center gap-2 mb-1">
                  <Gauge size={16} class="text-accent" />
                  <span class="font-semibold">Performance profile</span>
                  <span class="chip chip-ok">client-side · server-safe</span>
                </div>
                <p class="text-muted text-[12px] mb-3">Pulled from Modrinth, matched to {f.mc}. Safe on Hypixel, Donut SMP and other public servers.</p>
                <div class="grid grid-cols-2 gap-x-4 gap-y-2.5">
                  <For each={PERF_STACK}>
                    {(m) => (
                      <div class="flex items-center gap-2.5">
                        <Toggle on={f.perf[m.id]} onChange={(v) => setF("perf", m.id, v)} label={m.name} />
                        <div class="min-w-0">
                          <div class="text-[12.5px] font-semibold">{m.name}</div>
                          <div class="text-faint text-[11px] truncate" title={m.why}>{m.why}</div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
                <Show when={f.loader === "fabric"}>
                  <div class="flex items-center gap-2.5 mt-3 pt-3 border-t" style={{ "border-color": "var(--border)" }}>
                    <Toggle on={f.hud} onChange={(v) => setF("hud", v)} label="Halo HUD" />
                    <Sparkles size={14} class="text-accent" />
                    <div class="text-[12.5px]"><b>Halo HUD</b> <span class="text-faint">FPS/CPS, keystrokes, zoom, armor HUD</span></div>
                  </div>
                </Show>
              </div>
            </Show>

            <div class="flex justify-end gap-2 mt-1">
              <button class="btn" onClick={props.onClose}>Cancel</button>
              <button class="btn btn-primary" onClick={create}><Plus size={15} /> Create instance</button>
            </div>
          </div>
        </Show>

        <Show when={tab() === "import"}>
          <div class="grid grid-cols-2 gap-3">
            <For each={[
              { t: "Modrinth pack", d: ".mrpack file", icon: FileArchive },
              { t: "Prism Launcher", d: "Import instances", icon: Import },
              { t: "MultiMC", d: "Import instances", icon: Import },
              { t: "Vanilla .minecraft", d: "Worlds, packs, options", icon: Import },
            ]}>
              {(o) => (
                <button class="glass-flat glass-hover lift p-4 text-left cursor-pointer" onClick={() => { toast(`${o.t} import: file picker opens in the real app`, "info"); }}>
                  <o.icon size={20} class="text-accent mb-2" />
                  <div class="font-semibold">{o.t}</div>
                  <div class="text-faint text-[12px]">{o.d}</div>
                </button>
              )}
            </For>
          </div>
          <p class="text-faint text-[11.5px] mt-4">CurseForge .zip import is coming once a CurseForge API key is configured.</p>
        </Show>
      </div>
    </Modal>
  );
}

export default function Instances() {
  const nav = useNavigate();
  const [q, setQ] = createSignal("");
  const [open, setOpen] = createSignal(false);
  const list = createMemo(() => app.instances.filter((i) => i.name.toLowerCase().includes(q().toLowerCase())));
  return (
    <div class="page-enter pb-2">
      <PageHeader title="Instances" sub={`${app.instances.length} instances · shared cache saves duplicate downloads`}>
        <div class="relative">
          <Search size={15} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
          <input class="input pl-9 w-56" placeholder="Search instances" value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
        </div>
        <button class="btn btn-primary" onClick={() => setOpen(true)}><Plus size={16} /> New instance</button>
      </PageHeader>

      <div class="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-3">
        <For each={list()}>
          {(i) => (
            <div class="glass glass-hover lift p-4 cursor-pointer flex flex-col gap-3.5" onClick={() => nav(`/instances/${i.id}`)}>
              <div class="flex items-center gap-3">
                <IconTile icon={i.icon} hue={i.hue} size={48} />
                <div class="min-w-0 flex-1">
                  <div class="font-semibold text-[15px] truncate">{i.name}</div>
                  <div class="text-faint text-[12px]">{i.mcVersion} · {LOADER_LABEL[i.loader]} {i.loaderVersion}</div>
                </div>
              </div>
              <div class="flex flex-wrap gap-1.5">
                <span class="chip">{i.mods.length} mods</span>
                <Show when={i.perfProfile}><span class="chip chip-accent"><Gauge size={11} /> Perf</span></Show>
                <Show when={i.lastBootMs}><span class="chip"><Timer size={11} /> {(i.lastBootMs! / 1000).toFixed(1)}s boot</span></Show>
                <Show when={i.mods.some((m) => m.update)}><span class="chip chip-warn">Updates</span></Show>
              </div>
              <div class="flex items-center">
                <span class="text-faint text-[12px]">{i.playtimeH}h · {i.lastPlayed}</span>
                <div class="flex-1" />
                <PlayButton instanceId={i.id} size="md" />
              </div>
            </div>
          )}
        </For>
        <button class="glass-flat glass-hover lift flex flex-col items-center justify-center gap-2 min-h-[150px] cursor-pointer text-muted" style={{ "border-style": "dashed" }} onClick={() => setOpen(true)}>
          <Plus size={24} />
          <span class="font-semibold text-[13px]">New instance</span>
        </button>
      </div>
      <NewInstance open={open()} onClose={() => setOpen(false)} />
    </div>
  );
}
