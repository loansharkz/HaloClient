import { A, useParams, useSearchParams } from "@solidjs/router";
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { ChevronRight, Download, ExternalLink, Heart, LoaderCircle, RefreshCw, Scale } from "lucide-solid";
import { IconTile, InstanceIcon, hueOf } from "../components/Icons";
import { Empty, Modal } from "../components/Shell";
import { useInstaller, TYPES } from "./Browse";
import { api } from "../lib/api";
import { LOADER_LABEL, ago, fmt } from "../lib/format";
import { app, instanceById, selectedInstance } from "../lib/store";
import type { ContentKind } from "../lib/types";

// Project descriptions are untrusted HTML/Markdown: sanitize, and never allow scripts, styles, forms or iframes.
function renderBody(md: string | undefined, html: string | undefined): string {
  const raw = html ?? (md ? (marked.parse(md, { async: false, gfm: true, breaks: false }) as string) : "");
  return DOMPurify.sanitize(raw, { FORBID_TAGS: ["style", "script", "iframe", "form", "input", "object", "embed"], FORBID_ATTR: ["style", "onerror", "onclick"] });
}

export default function Project() {
  const params = useParams();
  const [search, setSearch] = useSearchParams();
  const [tab, setTab] = createSignal<"description" | "gallery" | "versions">("description");
  const [lightbox, setLightbox] = createSignal<string | null>(null);
  const { install, busy, WorldPicker } = useInstaller();

  const [p] = createResource(() => ({ s: params.source ?? "modrinth", id: params.id ?? "" }), (k) => api.project(k.s, k.id));
  const targetId = () => (search.instance as string) ?? selectedInstance()?.id ?? null;
  const target = () => (targetId() ? instanceById(targetId()!) : null);
  const isCf = () => params.source === "curseforge";

  // Normalized view over Modrinth / CurseForge project shapes.
  const v = createMemo(() => {
    const d = p();
    if (!d) return null;
    if (isCf()) {
      return {
        id: String(d.id), title: d.name as string, description: d.summary as string, author: d.authors?.[0]?.name ?? "", icon: d.logo?.thumbnailUrl as string | null,
        banner: d.screenshots?.[0]?.url as string | null, downloads: d.downloadCount as number, follows: d.thumbsUpCount as number, updated: d.dateModified as string,
        type: "mod" as ContentKind, body: renderBody(undefined, d.bodyHtml), gallery: (d.screenshots ?? []).map((s: any) => ({ url: s.url, title: s.title })),
        loaders: [] as string[], versions: [] as string[], client: "", server: "", license: null as string | null,
        links: [{ label: "CurseForge", url: d.links?.websiteUrl }, { label: "Source", url: d.links?.sourceUrl }, { label: "Issues", url: d.links?.issuesUrl }].filter((l) => l.url),
        categories: (d.categories ?? []).map((c: any) => c.name),
      };
    }
    const gallery = (d.gallery ?? []).map((g: any) => ({ url: g.url as string, title: g.title as string }));
    return {
      id: d.id as string, title: d.title as string, description: d.description as string, author: (d.author as string) ?? "", icon: d.icon_url as string | null,
      banner: ((d.gallery ?? []).find((g: any) => g.featured) ?? d.gallery?.[0])?.url ?? null, downloads: d.downloads as number, follows: d.followers as number,
      updated: d.updated as string, type: (d.project_type === "shader" ? "shader" : d.project_type) as ContentKind, body: renderBody(d.body, undefined), gallery,
      loaders: (d.loaders ?? []) as string[], versions: ((d.game_versions ?? []) as string[]).slice(-12).reverse(), client: d.client_side, server: d.server_side,
      license: d.license?.name ?? d.license?.id ?? null,
      links: [{ label: "Modrinth page", url: `https://modrinth.com/project/${d.slug ?? d.id}` }, { label: "Source", url: d.source_url }, { label: "Issues", url: d.issues_url }, { label: "Discord", url: d.discord_url }, { label: "Wiki", url: d.wiki_url }].filter((l) => l.url),
      categories: (d.categories ?? []) as string[],
    };
  });

  const [versions] = createResource(
    () => (tab() === "versions" && !isCf() && v() ? { id: v()!.id, loaders: v()!.type === "mod" && target() && target()!.loader !== "vanilla" ? [target()!.loader === "quilt" ? "fabric" : target()!.loader] : [], gv: [] as string[] } : null),
    (k) => api.projectVersions(k.id, k.loaders, k.gv).catch(() => []),
  );

  const doInstall = (versionId?: string) => v() && install({ id: v()!.id, source: params.source as any, title: v()!.title, projectType: v()!.type }, targetId(), versionId);

  // Links inside descriptions open in the system browser.
  function onBodyClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (a?.href?.startsWith("https://")) {
      e.preventDefault();
      api.openUrl(a.href);
    }
  }

  return (
    <div class="page-enter pb-2">
      <div class="flex items-center gap-1.5 text-[12px] text-faint mb-3">
        <A href="/browse" class="hover:text-[var(--text)]">Browse</A> <ChevronRight size={12} />
        <Show when={v()}><A href={`/browse?type=${v()!.type}`} class="hover:text-[var(--text)]">{TYPES.find((t) => t.id === v()!.type)?.label ?? "Projects"}</A> <ChevronRight size={12} /> <span class="text-muted">{v()!.title}</span></Show>
      </div>
      <Show when={!p.loading} fallback={<div class="glass h-[320px] skeleton" />}>
        <Show when={v()} fallback={<Empty title="Couldn't load this project">Check your connection and try again.</Empty>}>
          <section class="glass overflow-hidden">
            <div class="h-36 bg-cover bg-center relative" style={{ "background-image": v()!.banner ? `linear-gradient(to bottom, transparent 40%, rgb(0 0 0 / .45)), url("${v()!.banner}")` : `linear-gradient(135deg, oklch(0.55 0.2 ${hueOf(v()!.title)}), oklch(0.32 0.15 ${hueOf(v()!.title) + 60}))` }} />
            <div class="px-6 pb-5 relative flex gap-5 flex-wrap">
              <div class="-mt-12 p-1 rounded-[20px] self-start" style={{ background: "var(--bg)" }}><IconTile icon="package" hue={hueOf(v()!.title)} size={96} radius={18} src={v()!.icon} /></div>
              <div class="pt-4 flex-1 min-w-[240px]">
                <h1 class="text-[28px] font-bold tracking-tight leading-tight">{v()!.title}</h1>
                <p class="text-muted text-[13.5px] mt-1">{v()!.description}</p>
                <div class="flex gap-4 mt-2 text-[12.5px] text-muted tabular-nums flex-wrap">
                  <Show when={v()!.author}><span>by <b class="text-[var(--text)]">{v()!.author}</b></span></Show>
                  <span class="flex items-center gap-1"><Download size={13} /> <b class="text-[var(--text)]">{fmt(v()!.downloads ?? 0)}</b></span>
                  <span class="flex items-center gap-1"><Heart size={13} /> <b class="text-[var(--text)]">{fmt(v()!.follows ?? 0)}</b></span>
                  <span class="flex items-center gap-1"><RefreshCw size={12} /> {ago(v()!.updated)}</span>
                </div>
              </div>
              <div class="flex flex-col items-end gap-2 pt-4">
                <Show when={v()!.type !== "modpack"}>
                  <select id="proj-target" class="input" value={targetId() ?? ""} onChange={(e) => setSearch({ instance: e.currentTarget.value })} aria-label="Install to">
                    <For each={app.instances}>{(i) => <option value={i.id}>{i.name} · {i.mcVersion}</option>}</For>
                  </select>
                </Show>
                <button class="btn btn-primary h-10 px-5" disabled={busy() === v()!.id} onClick={() => doInstall()}>
                  <Show when={busy() === v()!.id} fallback={<Download size={16} />}><LoaderCircle size={16} class="spin" /></Show>
                  {v()!.type === "modpack" ? "Install as new instance" : target() ? `Install to ${target()!.name}` : "Install"}
                </button>
              </div>
            </div>
          </section>

          <div class="flex gap-4 mt-4 items-start">
            <div class="flex-1 min-w-0">
              <div class="tabs mb-3" role="tablist">
                <button role="tab" aria-selected={tab() === "description"} classList={{ active: tab() === "description" }} onClick={() => setTab("description")}>Description</button>
                <button role="tab" aria-selected={tab() === "gallery"} classList={{ active: tab() === "gallery" }} onClick={() => setTab("gallery")}>Gallery <span class="text-faint">{v()!.gallery.length}</span></button>
                <Show when={!isCf()}><button role="tab" aria-selected={tab() === "versions"} classList={{ active: tab() === "versions" }} onClick={() => setTab("versions")}>Versions</button></Show>
              </div>
              <Show when={tab() === "description"}>
                <article class="glass p-6 md select-text" innerHTML={v()!.body || "<p>No description.</p>"} onClick={onBodyClick} />
              </Show>
              <Show when={tab() === "gallery"}>
                <Show when={v()!.gallery.length} fallback={<Empty title="No images">This project has no gallery images.</Empty>}>
                  <div class="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
                    <For each={v()!.gallery}>
                      {(g: { url: string; title: string }) => (
                        <button class="glass-flat lift overflow-hidden text-left cursor-pointer" onClick={() => setLightbox(g.url)}>
                          <img src={g.url} alt={g.title ?? ""} loading="lazy" class="w-full aspect-video object-cover" />
                          <Show when={g.title}><div class="px-3 py-2 text-[12px] text-muted truncate">{g.title}</div></Show>
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
              </Show>
              <Show when={tab() === "versions"}>
                <div class="glass p-1.5">
                  <Show when={!versions.loading} fallback={<div class="p-8 text-center"><LoaderCircle size={18} class="spin inline" /></div>}>
                    <div class="grid grid-cols-[1fr_120px_160px_110px_100px] gap-3 px-3 py-2 text-faint text-[11px] font-semibold uppercase tracking-wider">
                      <span>Version</span><span>Loaders</span><span>Game versions</span><span>Published</span><span />
                    </div>
                    <For each={versions() ?? []} fallback={<div class="p-6 text-center text-muted text-[13px]">No versions match {target() ? LOADER_LABEL[target()!.loader] : "this instance"}.</div>}>
                      {(ver) => (
                        <div class="grid grid-cols-[1fr_120px_160px_110px_100px] gap-3 px-3 py-2.5 rounded-xl hover:bg-[var(--glass)] items-center text-[12.5px]">
                          <div class="min-w-0">
                            <div class="font-semibold truncate">{ver.version_number}</div>
                            <div class="text-faint text-[11.5px] truncate">{ver.name}</div>
                          </div>
                          <div class="flex gap-1 flex-wrap">{(ver.loaders ?? []).slice(0, 2).map((l: string) => <span class="chip">{LOADER_LABEL[l] ?? l}</span>)}</div>
                          <div class="text-muted truncate">{(ver.game_versions ?? []).slice(-3).join(", ")}</div>
                          <div class="text-faint">{ago(ver.date_published)}</div>
                          <button class="btn h-8" classList={{ "btn-primary": ver.version_type === "release" }} onClick={() => doInstall(ver.id)}>
                            <Download size={13} /> {ver.version_type === "release" ? "Install" : ver.version_type}
                          </button>
                        </div>
                      )}
                    </For>
                  </Show>
                </div>
              </Show>
            </div>

            <aside class="w-[260px] shrink-0 flex flex-col gap-3">
              <Show when={target() && v()!.type !== "modpack"}>
                <div class="glass p-4 flex items-center gap-3">
                  <InstanceIcon inst={target()!} size={36} />
                  <div class="min-w-0 text-[12.5px]"><div class="text-faint">Installs into</div><div class="font-semibold truncate">{target()!.name}</div></div>
                </div>
              </Show>
              <Show when={!isCf()}>
                <div class="glass p-4">
                  <div class="font-semibold mb-2">Compatibility</div>
                  <div class="field-label mb-1">Minecraft</div>
                  <div class="flex gap-1 flex-wrap mb-3"><For each={v()!.versions}>{(x) => <span class="chip">{x}</span>}</For></div>
                  <Show when={v()!.loaders.length}>
                    <div class="field-label mb-1">Platforms</div>
                    <div class="flex gap-1 flex-wrap mb-3"><For each={v()!.loaders}>{(l) => <span class="chip">{LOADER_LABEL[l] ?? l}</span>}</For></div>
                  </Show>
                  <div class="field-label mb-1">Environment</div>
                  <div class="text-[12.5px] text-muted">Client: {v()!.client} · Server: {v()!.server}</div>
                </div>
              </Show>
              <Show when={v()!.links.length}>
                <div class="glass p-4">
                  <div class="font-semibold mb-2">Links</div>
                  <div class="flex flex-col gap-1">
                    <For each={v()!.links}>{(l) => <button class="flex items-center gap-2 text-[12.5px] text-muted hover:text-[var(--text)] cursor-pointer text-left" onClick={() => api.openUrl(l.url)}><ExternalLink size={13} /> {l.label}</button>}</For>
                  </div>
                </div>
              </Show>
              <Show when={v()!.license}>
                <div class="glass p-4 text-[12.5px] flex items-center gap-2 text-muted"><Scale size={14} /> {v()!.license}</div>
              </Show>
            </aside>
          </div>
        </Show>
      </Show>
      <Modal open={!!lightbox()} onClose={() => setLightbox(null)} width={1100} label="Image">
        <img src={lightbox()!} alt="" class="w-full rounded-2xl" />
      </Modal>
      <WorldPicker />
    </div>
  );
}
