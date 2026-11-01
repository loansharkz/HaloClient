import { useNavigate, useSearchParams } from "@solidjs/router";
import { For, Show, createEffect, createMemo, createResource, createSignal, on } from "solid-js";
import { Check, Download, Heart, LayoutGrid, LoaderCircle, RefreshCw, Rows3, Search, X } from "lucide-solid";
import { IconTile, InstanceIcon, hueOf } from "../components/Icons";
import { AdSlot, Empty, Modal, Toggle } from "../components/Shell";
import { api } from "../lib/api";
import { LOADER_LABEL, ago, fmt } from "../lib/format";
import { app, attempt, instanceById, refreshInstances, selectedInstance, setApp, settings, setSettings, toast } from "../lib/store";
import type { ContentKind, InstallResult, SearchHit, SearchQuery } from "../lib/types";

export const TYPES: { id: ContentKind; label: string }[] = [
  { id: "mod", label: "Mods" },
  { id: "modpack", label: "Modpacks" },
  { id: "resourcepack", label: "Resource Packs" },
  { id: "shader", label: "Shaders" },
  { id: "datapack", label: "Data Packs" },
];

const CATEGORIES: Record<ContentKind, string[]> = {
  mod: ["adventure", "decoration", "economy", "equipment", "food", "game-mechanics", "library", "magic", "management", "minigame", "mobs", "optimization", "social", "storage", "technology", "transportation", "utility", "worldgen"],
  modpack: ["adventure", "challenging", "combat", "kitchen-sink", "lightweight", "magic", "multiplayer", "optimization", "quests", "technology"],
  resourcepack: ["16x", "32x", "64x", "combat", "decoration", "realistic", "simplistic", "themed", "tweaks", "vanilla-like", "gui", "fonts", "audio"],
  shader: ["cartoon", "fantasy", "realistic", "semi-realistic", "vanilla-like", "atmosphere", "colored-lighting", "path-tracing", "potato", "low", "medium", "high"],
  datapack: ["adventure", "decoration", "equipment", "food", "game-mechanics", "magic", "mobs", "technology", "utility", "worldgen"],
};

const label = (s: string) => s.replace(/-/g, " ").replace(/^\w/, (c) => c.toUpperCase());

/** Install action shared by the results list and the project page. */
export function useInstaller() {
  const nav = useNavigate();
  const [busy, setBusy] = createSignal<string | null>(null);
  const [worldPick, setWorldPick] = createSignal<{ hit: Pick<SearchHit, "id" | "source" | "title">; worlds: string[]; instanceId: string; versionId?: string | null } | null>(null);

  function report(r: InstallResult, title: string, instanceId: string) {
    r.warnings.forEach((w) => toast(w, "warn"));
    if (r.manualUrl) {
      toast(`${title} must be downloaded from CurseForge (author's choice).`, "info", { label: "Open", run: () => api.openUrl(r.manualUrl!) });
      return;
    }
    const extra = r.installed.length > 1 ? ` + ${r.installed.length - 1} dependenc${r.installed.length === 2 ? "y" : "ies"}` : "";
    toast(`Installed ${title}${extra} → ${instanceById(instanceId)?.name ?? "instance"}`, "ok");
    if (r.needsShaderLoader) {
      toast(`Shaders need ${r.needsShaderLoader === "iris" ? "Iris" : "Oculus"} to load.`, "info", {
        label: "Install it",
        run: () => install({ id: r.needsShaderLoader!, source: "modrinth", title: r.needsShaderLoader === "iris" ? "Iris Shaders" : "Oculus", projectType: "mod" }, instanceId),
      });
    }
  }

  async function install(hit: Pick<SearchHit, "id" | "source" | "title" | "projectType">, instanceId: string | null, versionId?: string | null, world?: string | null) {
    const kind = hit.projectType as ContentKind;
    setBusy(hit.id);
    try {
      if (kind === "modpack") {
        const inst = await attempt(() => api.installModpack(hit.id, versionId));
        if (inst) {
          await refreshInstances();
          setApp("selectedInstanceId", inst.id);
          toast(`Installed ${hit.title} as a new instance`, "ok");
          nav(`/instance/${inst.id}`);
        }
        return;
      }
      if (!instanceId) {
        toast("Create an instance first", "warn");
        return;
      }
      if (kind === "datapack" && !world) {
        const worlds = await api.worlds(instanceId);
        if (!worlds.length) {
          toast("Data packs go into a world. Create a world in this instance first.", "warn");
          return;
        }
        setWorldPick({ hit, worlds, instanceId, versionId });
        return;
      }
      const r = await attempt(() => api.installProject({ instanceId, source: hit.source, projectId: hit.id, versionId: versionId ?? null, kind, world: world ?? null }));
      if (r) report(r, hit.title, instanceId);
    } finally {
      setBusy(null);
    }
  }

  const WorldPicker = () => (
    <Modal open={!!worldPick()} onClose={() => setWorldPick(null)} width={400} label="Choose a world">
      <div class="p-6">
        <h2 class="text-lg font-bold">Add {worldPick()?.hit.title} to which world?</h2>
        <div class="flex flex-col gap-1.5 mt-4">
          <For each={worldPick()?.worlds ?? []}>
            {(w) => (
              <button class="glass-flat glass-hover p-3 text-left cursor-pointer font-semibold" onClick={() => { const p = worldPick()!; setWorldPick(null); install({ ...p.hit, projectType: "datapack" }, p.instanceId, p.versionId, w); }}>
                {w}
              </button>
            )}
          </For>
        </div>
      </div>
    </Modal>
  );

  return { install, busy, WorldPicker };
}

export default function Browse() {
  const nav = useNavigate();
  const [params, setParams] = useSearchParams();
  const type = () => (params.type as ContentKind) ?? "mod";
  const [source, setSource] = createSignal<"modrinth" | "curseforge">("modrinth");
  const [query, setQuery] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  const [cats, setCats] = createSignal<string[]>([]);
  const [env, setEnv] = createSignal<"client" | "server" | null>(null);
  const [sort, setSort] = createSignal<SearchQuery["sort"]>("relevance");
  const [matchInstance, setMatchInstance] = createSignal(true);
  const [hideInstalled, setHideInstalled] = createSignal(false);
  const [page, setPage] = createSignal(0);
  const [hits, setHits] = createSignal<SearchHit[]>([]);
  const PAGE = 20;
  const { install, busy, WorldPicker } = useInstaller();

  const targetId = () => (params.instance as string) ?? selectedInstance()?.id ?? null;
  const target = () => (targetId() ? instanceById(targetId()!) : null);

  let t: ReturnType<typeof setTimeout>;
  createEffect(on(query, (q) => { clearTimeout(t); t = setTimeout(() => setDebounced(q), 280); }));
  createEffect(on(type, () => { setCats([]); setPage(0); }, { defer: true }));

  const q = createMemo<SearchQuery>(() => ({
    source: source(),
    query: debounced(),
    projectType: type(),
    gameVersion: matchInstance() && type() !== "modpack" ? target()?.mcVersion ?? null : null,
    loader: matchInstance() && type() === "mod" ? target()?.loader ?? null : null,
    categories: cats(),
    environment: env(),
    sort: sort(),
    offset: 0,
    limit: PAGE,
  }));
  createEffect(on(q, () => { setPage(0); setHits([]); }));
  const [result] = createResource(() => ({ ...q(), offset: page() * PAGE }), (k) => api.search(k).catch((e) => { toast(String(e), "warn"); return { hits: [], total: 0 }; }));
  createEffect(() => {
    const r = result();
    if (r) setHits((h) => (page() === 0 ? r.hits : [...h, ...r.hits]));
  });

  const [installed, { refetch }] = createResource(() => targetId(), (id) => api.listContent(id).catch(() => []));
  const installedIds = createMemo(() => new Set((installed() ?? []).flatMap((c) => [c.meta?.projectId, c.meta?.slug]).filter(Boolean) as string[]));
  const isInstalled = (h: SearchHit) => installedIds().has(h.id) || installedIds().has(h.slug);
  const shown = () => (hideInstalled() ? hits().filter((h) => !isInstalled(h)) : hits());

  async function doInstall(h: SearchHit) {
    await install(h, targetId());
    refetch();
  }

  const InstallBtn = (p: { h: SearchHit; compact?: boolean }) => (
    <button
      class="btn"
      classList={{ "btn-primary": !isInstalled(p.h), "h-8 px-3": p.compact }}
      disabled={busy() === p.h.id || (isInstalled(p.h) && p.h.projectType !== "modpack")}
      onClick={(e) => { e.stopPropagation(); doInstall(p.h); }}
    >
      <Show when={busy() === p.h.id} fallback={isInstalled(p.h) && p.h.projectType !== "modpack" ? <><Check size={14} /> Installed</> : <><Download size={14} /> {p.h.projectType === "modpack" ? "Install pack" : "Install"}</>}>
        <LoaderCircle size={14} class="spin" /> Installing
      </Show>
    </button>
  );

  const open = (h: SearchHit) => nav(`/project/${h.source}/${h.id}${targetId() ? `?instance=${targetId()}` : ""}`);

  return (
    <div class="page-enter pb-2">
      <div class="flex items-center gap-3 mb-4 flex-wrap">
        <h1 class="text-[26px] font-bold tracking-tight mr-2">Browse</h1>
        <div class="seg">
          <For each={TYPES}>{(t) => <button classList={{ active: type() === t.id }} onClick={() => setParams({ type: t.id })}>{t.label}</button>}</For>
        </div>
        <div class="flex-1" />
        <div class="seg">
          <button classList={{ active: source() === "modrinth" }} onClick={() => setSource("modrinth")}>Modrinth</button>
          <button classList={{ active: source() === "curseforge" }} disabled={!app.info?.curseforge} title={app.info?.curseforge ? "" : "Needs a CurseForge API key in this build"} onClick={() => setSource("curseforge")}>CurseForge</button>
        </div>
      </div>

      <div class="flex gap-4 items-start">
        {/* Filters (Modrinth-style facet sidebar) */}
        <aside class="glass p-4 w-[220px] shrink-0 sticky top-0 flex flex-col gap-4" aria-label="Filters">
          <Show when={type() !== "modpack"}>
            <div>
              <div class="field-label mb-2">Installing to</div>
              <select id="browse-target" class="input w-full" value={targetId() ?? ""} onChange={(e) => setParams({ instance: e.currentTarget.value })}>
                <For each={app.instances}>{(i) => <option value={i.id}>{i.name}</option>}</For>
              </select>
              <Show when={target()}>
                <div class="flex items-center gap-2 mt-2">
                  <InstanceIcon inst={target()!} size={22} />
                  <span class="text-[12px] text-muted">{target()!.mcVersion} · {LOADER_LABEL[target()!.loader]}</span>
                </div>
                <label class="flex items-center gap-2 mt-2.5 text-[12.5px] cursor-pointer">
                  <Toggle on={matchInstance()} onChange={setMatchInstance} label="Match instance" /> Only compatible
                </label>
                <label class="flex items-center gap-2 mt-2 text-[12.5px] cursor-pointer">
                  <Toggle on={hideInstalled()} onChange={setHideInstalled} label="Hide installed" /> Hide installed
                </label>
              </Show>
            </div>
          </Show>
          <div>
            <div class="field-label mb-2">Environment</div>
            <div class="flex flex-col gap-1">
              <For each={[["client", "Client"], ["server", "Server"]] as const}>
                {([k, l]) => (
                  <label class="facet">
                    <input type="checkbox" checked={env() === k} onChange={(e) => setEnv(e.currentTarget.checked ? k : null)} /> {l}
                  </label>
                )}
              </For>
            </div>
          </div>
          <div>
            <div class="flex items-center mb-2"><span class="field-label">Categories</span><div class="flex-1" /><Show when={cats().length}><button class="text-[11px] text-accent cursor-pointer" onClick={() => setCats([])}>Clear</button></Show></div>
            <div class="flex flex-col gap-0.5 max-h-[340px] scroll pr-1">
              <For each={CATEGORIES[type()]}>
                {(c) => (
                  <label class="facet">
                    <input type="checkbox" checked={cats().includes(c)} onChange={(e) => setCats((l) => (e.currentTarget.checked ? [...l, c] : l.filter((x) => x !== c)))} /> {label(c)}
                  </label>
                )}
              </For>
            </div>
          </div>
        </aside>

        <div class="flex-1 min-w-0">
          <div class="flex gap-2 mb-3">
            <div class="relative flex-1">
              <Search size={15} class="absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
              <input id="browse-search" class="input pl-9 w-full h-10" placeholder={`Search ${TYPES.find((t) => t.id === type())!.label.toLowerCase()}…`} value={query()} onInput={(e) => setQuery(e.currentTarget.value)} />
              <Show when={query()}><button class="absolute right-2 top-1/2 -translate-y-1/2 btn btn-ghost btn-icon h-7" aria-label="Clear search" onClick={() => setQuery("")}><X size={14} /></button></Show>
            </div>
            <select id="browse-sort" class="input h-10" value={sort()} onChange={(e) => setSort(e.currentTarget.value as any)} aria-label="Sort by">
              <option value="relevance">Relevance</option>
              <option value="downloads">Downloads</option>
              <option value="follows">Follows</option>
              <option value="newest">Newest</option>
              <option value="updated">Updated</option>
            </select>
            <div class="seg">
              <button classList={{ active: settings.browseView === "list" }} onClick={() => setSettings("browseView", "list")} aria-label="List view"><Rows3 size={14} /></button>
              <button classList={{ active: settings.browseView === "grid" }} onClick={() => setSettings("browseView", "grid")} aria-label="Grid view"><LayoutGrid size={14} /></button>
            </div>
          </div>
          <Show when={result()}><div class="text-faint text-[12px] mb-2 tabular-nums">{fmt(result()!.total)} results</div></Show>

          <Show when={!(result.loading && page() === 0)} fallback={<div class="flex flex-col gap-2"><For each={[0, 1, 2, 3, 4]}>{() => <div class="glass h-[104px] skeleton" />}</For></div>}>
            <Show when={shown().length > 0} fallback={<Empty icon={<Search size={26} />} title="No results">Try a different search, or turn off "Only compatible" to see versions for other instances.</Empty>}>
              <Show when={settings.browseView === "list"}>
                <div class="flex flex-col gap-2">
                  <For each={shown()}>
                    {(h) => (
                      <article class="glass glass-hover p-4 flex gap-4 cursor-pointer" onClick={() => open(h)}>
                        <IconTile icon="package" hue={hueOf(h.title)} size={84} radius={16} src={h.iconUrl} />
                        <div class="min-w-0 flex-1">
                          <div class="flex items-baseline gap-2 flex-wrap">
                            <h3 class="font-bold text-[16px]">{h.title}</h3>
                            <span class="text-faint text-[12.5px]">by {h.author}</span>
                          </div>
                          <p class="text-muted text-[13px] mt-1 line-clamp-2">{h.description}</p>
                          <div class="flex gap-1.5 mt-2 flex-wrap">
                            <For each={h.categories.slice(0, 5)}>{(c) => <span class="chip">{label(c)}</span>}</For>
                          </div>
                        </div>
                        <div class="flex flex-col items-end justify-between shrink-0 text-[12.5px] min-w-[130px]">
                          <div class="text-right tabular-nums">
                            <div class="flex items-center gap-1.5 justify-end"><Download size={13} class="text-faint" /> <b>{fmt(h.downloads)}</b> <span class="text-faint">downloads</span></div>
                            <div class="flex items-center gap-1.5 justify-end mt-0.5"><Heart size={13} class="text-faint" /> <b>{fmt(h.follows)}</b> <span class="text-faint">followers</span></div>
                            <div class="flex items-center gap-1.5 justify-end mt-0.5 text-faint"><RefreshCw size={12} /> {ago(h.updated)}</div>
                          </div>
                          <InstallBtn h={h} compact />
                        </div>
                      </article>
                    )}
                  </For>
                </div>
              </Show>
              <Show when={settings.browseView === "grid"}>
                <div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3">
                  <For each={shown()}>
                    {(h) => (
                      <article class="glass glass-hover lift overflow-hidden cursor-pointer flex flex-col" onClick={() => open(h)}>
                        <div class="h-24 relative bg-cover bg-center" style={{ "background-image": h.bannerUrl ? `url("${h.bannerUrl}")` : `linear-gradient(135deg, oklch(0.55 0.2 ${hueOf(h.title)}), oklch(0.32 0.15 ${hueOf(h.title) + 60}))` }}>
                          <div class="absolute -bottom-5 left-4 p-0.5 rounded-xl" style={{ background: "var(--bg)" }}><IconTile icon="package" hue={hueOf(h.title)} size={48} src={h.iconUrl} /></div>
                        </div>
                        <div class="p-4 pt-7 flex flex-col flex-1">
                          <div class="font-bold">{h.title}</div>
                          <div class="text-faint text-[11.5px]">by {h.author}</div>
                          <p class="text-muted text-[12px] mt-2 line-clamp-2 flex-1">{h.description}</p>
                          <div class="flex items-center mt-3 gap-3 text-[11.5px] text-faint tabular-nums">
                            <span class="flex items-center gap-1"><Download size={12} /> {fmt(h.downloads)}</span>
                            <span class="flex items-center gap-1"><Heart size={12} /> {fmt(h.follows)}</span>
                            <div class="flex-1" />
                            <InstallBtn h={h} compact />
                          </div>
                        </div>
                      </article>
                    )}
                  </For>
                </div>
              </Show>
              <Show when={result() && hits().length < result()!.total}>
                <div class="flex justify-center mt-4">
                  <button class="btn" disabled={result.loading} onClick={() => setPage(page() + 1)}>
                    <Show when={result.loading} fallback="Load more"><LoaderCircle size={14} class="spin" /> Loading</Show>
                  </button>
                </div>
              </Show>
            </Show>
          </Show>
        </div>
        <AdSlot name="browse sidebar" class="w-[170px] shrink-0 h-[420px] sticky top-0" />
      </div>
      <WorldPicker />
    </div>
  );
}
