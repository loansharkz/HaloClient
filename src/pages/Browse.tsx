import { For, Show, createMemo, createSignal } from "solid-js";
import { Check, Download, ExternalLink, Heart, LayoutGrid, LoaderCircle, Rows3, Search, X } from "lucide-solid";
import { IconTile } from "../components/Icons";
import { projectMedia } from "../lib/media";
import { AdSlot, Modal, PageHeader } from "../components/Shell";
import { LOADER_LABEL, MC_VERSIONS, PROJECTS, fmt, type Project } from "../lib/mock";
import { app, selectedInstance, setApp, setSettings, settings, toast } from "../lib/store";

const TYPES = ["Mods", "Modpacks", "Resource Packs", "Shaders", "Data Packs"] as const;
const CATEGORIES = ["optimization", "utility", "decoration", "adventure", "library", "food"];

function InstallButton(props: { p: Project; compact?: boolean }) {
  const [state, setState] = createSignal<"idle" | "busy" | "done">("idle");
  const installed = () => selectedInstance().mods.some((m) => m.name === props.p.title) || state() === "done";
  const install = (e: MouseEvent) => {
    e.stopPropagation();
    if (installed() || state() === "busy") return;
    setState("busy");
    setTimeout(() => {
      const inst = selectedInstance();
      const deps = (props.p.deps ?? []).filter((d) => !inst.mods.some((m) => m.name === d));
      setApp("instances", (i) => i.id === inst.id, "mods", (l) => [
        ...l,
        ...deps.map((d) => ({ id: d.toLowerCase(), name: d, version: "latest", enabled: true })),
        { id: props.p.slug, name: props.p.title, version: "latest", enabled: true },
      ]);
      setState("done");
      toast(`Installed ${props.p.title}${deps.length ? ` + dependency ${deps.join(", ")}` : ""} → ${inst.name}`, "ok");
    }, 700);
  };
  return (
    <button class="btn" classList={{ "btn-primary": !installed(), "h-8 px-3": props.compact }} onClick={install} disabled={state() === "busy"}>
      <Show when={state() === "busy"} fallback={installed() ? <><Check size={14} /> Installed</> : <><Download size={14} /> Install</>}>
        <LoaderCircle size={14} class="spin" /> Installing
      </Show>
    </button>
  );
}

function ProjectModal(props: { p: Project | null; onClose: () => void }) {
  return (
    <Modal open={!!props.p} onClose={props.onClose} width={680}>
      <Show when={props.p}>
        {(p) => (
          <div>
            <div class="h-40 relative bg-cover bg-center" style={{ "background-image": projectMedia(p().slug)?.banner ? `linear-gradient(to bottom, transparent 55%, rgb(0 0 0 / .35)), url(${projectMedia(p().slug)!.banner})` : `linear-gradient(135deg, oklch(0.55 0.2 ${p().hue}), oklch(0.35 0.18 ${p().hue + 60}))` }}>
              <button class="btn btn-icon absolute top-3 right-3 !bg-black/30 !border-transparent text-white" onClick={props.onClose}><X size={16} /></button>
            </div>
            <div class="px-6 pb-6 -mt-8 relative">
              <div class="flex items-end gap-4">
                <div class="p-1 rounded-2xl" style={{ background: "var(--bg)" }}><IconTile icon={p().icon} hue={p().hue} size={72} src={projectMedia(p().slug)?.icon} /></div>
                <div class="pb-1 flex-1 min-w-0">
                  <h2 class="text-xl font-bold">{p().title}</h2>
                  <div class="text-muted text-[12.5px]">by {p().author} · {p().source === "modrinth" ? "Modrinth" : "CurseForge"}</div>
                </div>
                <InstallButton p={p()} />
              </div>
              <div class="flex gap-4 mt-4 text-[12.5px]">
                <span><b>{fmt(p().downloads)}</b> <span class="text-faint">downloads</span></span>
                <span><b>{fmt(p().follows)}</b> <span class="text-faint">followers</span></span>
                <span class="text-faint">Updated {p().updated}</span>
              </div>
              <div class="flex flex-wrap gap-1.5 mt-3">
                <For each={p().loaders}>{(l) => <span class="chip">{LOADER_LABEL[l]}</span>}</For>
                <span class="chip">Client: {p().client}</span>
                <span class="chip">Server: {p().server}</span>
              </div>
              <p class="mt-4 text-[13.5px] leading-relaxed">{p().description}</p>
              <Show when={(projectMedia(p().slug)?.gallery.length ?? 0) > 1}>
                <div class="grid grid-cols-3 gap-2 mt-4">
                  <For each={projectMedia(p().slug)!.gallery}>
                    {(src) => <img src={src} alt="" class="rounded-xl aspect-video object-cover w-full" style={{ background: "var(--glass-strong)" }} />}
                  </For>
                </div>
              </Show>
              <p class="mt-3 text-muted text-[12.5px] leading-relaxed">
                Full description, versions list and changelog render here (sanitized markdown from the Modrinth API).
              </p>
              <Show when={p().deps?.length}>
                <div class="glass-flat p-3 mt-4 text-[12.5px]">
                  <b>Required dependencies:</b> {p().deps!.join(", ")}. Installed automatically.
                </div>
              </Show>
              <div class="mt-4 text-faint text-[11.5px]">Installs into <b class="text-muted">{selectedInstance().name}</b> ({selectedInstance().mcVersion}, {LOADER_LABEL[selectedInstance().loader]})</div>
            </div>
          </div>
        )}
      </Show>
    </Modal>
  );
}

export default function Browse() {
  const [type, setType] = createSignal<(typeof TYPES)[number]>("Mods");
  const [source, setSource] = createSignal<"modrinth" | "curseforge" | "both">("modrinth");
  const [q, setQ] = createSignal("");
  const [cat, setCat] = createSignal<string | null>(null);
  const [sort, setSort] = createSignal("relevance");
  const [open, setOpen] = createSignal<Project | null>(null);
  const cfAvailable = false; // backend reports whether CURSEFORGE_API_KEY is configured

  const results = createMemo(() => {
    let r = PROJECTS.filter((p) => (source() === "both" ? true : p.source === source()));
    if (q()) r = r.filter((p) => (p.title + p.description).toLowerCase().includes(q().toLowerCase()));
    if (cat()) r = r.filter((p) => p.categories.includes(cat()!));
    if (sort() === "downloads") r = [...r].sort((a, b) => b.downloads - a.downloads);
    return r;
  });

  return (
    <div class="page-enter pb-2">
      <PageHeader title="Browse" sub={`Installing into ${selectedInstance().name} · ${selectedInstance().mcVersion} · ${LOADER_LABEL[selectedInstance().loader]}`}>
        <select class="input" value={app.selectedInstanceId} onChange={(e) => setApp("selectedInstanceId", e.currentTarget.value)}>
          <For each={app.instances}>{(i) => <option value={i.id}>{i.name}</option>}</For>
        </select>
      </PageHeader>

      <div class="flex items-center gap-3 mb-3">
        <div class="seg">
          <For each={TYPES}>{(t) => <button classList={{ active: type() === t }} onClick={() => setType(t)}>{t}</button>}</For>
        </div>
        <div class="flex-1" />
        <div class="seg">
          <button classList={{ active: source() === "modrinth" }} onClick={() => setSource("modrinth")}>Modrinth</button>
          <button classList={{ active: source() === "curseforge" }} disabled={!cfAvailable} title="Add a CurseForge API key to enable" onClick={() => setSource("curseforge")}>CurseForge</button>
          <button classList={{ active: source() === "both" }} disabled={!cfAvailable} title="Add a CurseForge API key to enable" onClick={() => setSource("both")}>Both</button>
        </div>
      </div>

      <div class="flex gap-4">
        <div class="flex-1 min-w-0">
          <div class="flex gap-2 mb-3">
            <div class="relative flex-1">
              <Search size={15} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
              <input class="input pl-9 w-full" placeholder={`Search ${type().toLowerCase()} on Modrinth`} value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
            </div>
            <select class="input" value={selectedInstance().mcVersion}>
              <For each={MC_VERSIONS}>{(v) => <option>{v}</option>}</For>
            </select>
            <select class="input" value={sort()} onChange={(e) => setSort(e.currentTarget.value)}>
              <option value="relevance">Relevance</option>
              <option value="downloads">Downloads</option>
              <option value="updated">Recently updated</option>
            </select>
            <div class="seg">
              <button classList={{ active: settings.browseView === "grid" }} onClick={() => setSettings("browseView", "grid")} title="Grid"><LayoutGrid size={14} /></button>
              <button classList={{ active: settings.browseView === "list" }} onClick={() => setSettings("browseView", "list")} title="List"><Rows3 size={14} /></button>
            </div>
          </div>
          <div class="flex gap-1.5 mb-4 flex-wrap">
            <button class="chip cursor-pointer" classList={{ "chip-accent": !cat() }} onClick={() => setCat(null)}>All</button>
            <For each={CATEGORIES}>{(c) => <button class="chip cursor-pointer capitalize" classList={{ "chip-accent": cat() === c }} onClick={() => setCat(c)}>{c}</button>}</For>
          </div>

          <Show when={settings.browseView === "grid"}>
            <div class="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-3">
              <For each={results()}>
                {(p) => (
                  <div class="glass glass-hover lift overflow-hidden cursor-pointer flex flex-col" onClick={() => setOpen(p)}>
                    <div class="h-24 relative bg-cover bg-center" style={{ "background-image": projectMedia(p.slug)?.banner ? `url(${projectMedia(p.slug)!.banner})` : `linear-gradient(135deg, oklch(0.55 0.2 ${p.hue}), oklch(0.32 0.15 ${p.hue + 60}))` }}>
                      <div class="absolute -bottom-5 left-4 p-0.5 rounded-xl" style={{ background: "var(--bg)" }}><IconTile icon={p.icon} hue={p.hue} size={44} src={projectMedia(p.slug)?.icon} /></div>
                    </div>
                    <div class="p-4 pt-7 flex flex-col flex-1">
                      <div class="font-semibold">{p.title}</div>
                      <div class="text-faint text-[11.5px]">by {p.author}</div>
                      <p class="text-muted text-[12px] mt-2 line-clamp-2 flex-1">{p.description}</p>
                      <div class="flex items-center mt-3 gap-3 text-[11.5px] text-faint">
                        <span class="flex items-center gap-1"><Download size={12} /> {fmt(p.downloads)}</span>
                        <span class="flex items-center gap-1"><Heart size={12} /> {fmt(p.follows)}</span>
                        <div class="flex-1" />
                        <InstallButton p={p} compact />
                      </div>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
          <Show when={settings.browseView === "list"}>
            <div class="glass p-1.5">
              <For each={results()}>
                {(p) => (
                  <div class="flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-[var(--glass)] cursor-pointer transition-colors" onClick={() => setOpen(p)}>
                    <IconTile icon={p.icon} hue={p.hue} size={36} src={projectMedia(p.slug)?.icon} />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2"><span class="font-semibold text-[13px]">{p.title}</span><span class="text-faint text-[11.5px]">by {p.author}</span></div>
                      <div class="text-muted text-[12px] truncate">{p.description}</div>
                    </div>
                    <span class="text-faint text-[11.5px] w-16 text-right">{fmt(p.downloads)}</span>
                    <span class="text-faint text-[11.5px] w-24 text-right">{p.updated}</span>
                    <InstallButton p={p} compact />
                  </div>
                )}
              </For>
            </div>
          </Show>
          <Show when={results().length === 0}>
            <div class="glass p-10 text-center text-muted">
              Nothing matched. <Show when={source() !== "modrinth"}><span class="flex items-center justify-center gap-1 mt-1"><ExternalLink size={13} /> CurseForge needs an API key.</span></Show>
            </div>
          </Show>
        </div>
        <AdSlot name="browse sidebar" class="w-[180px] shrink-0 h-[420px] sticky top-0" />
      </div>
      <ProjectModal p={open()} onClose={() => setOpen(null)} />
    </div>
  );
}
