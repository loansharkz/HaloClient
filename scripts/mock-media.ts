// Vite plugin: provides `virtual:mock-media` for the UI mockup.
//
// At build/dev start it pulls REAL public media: Modrinth project icons +
// gallery images, and each server's own favicon + MOTD (via a server-status
// API). Results are inlined as data URIs (so the single-file preview works
// under a strict CSP) and cached in node_modules/.cache. If the network
// blocks a host, that part is simply empty and the UI falls back to gradient
// tiles. Nothing is ever invented.
//
// In the real app this is replaced by the Rust backend: Modrinth API for
// projects, and a native Server List Ping for favicons/MOTD/player counts.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import type { Plugin } from "vite";

const ID = "virtual:mock-media";
const RESOLVED = "\0" + ID;
const UA = "loansharkz/HaloClient/0.1.0 (github.com/loansharkz/haloclient)";
const CACHE = resolve("node_modules/.cache/halo-mock-media.json");
const SOURCES = resolve("src/lib/mock-sources.json");

interface Media {
  fetchedAt: string | null;
  projects: Record<string, { icon?: string; banner?: string; gallery: string[] }>;
  servers: Record<string, { icon?: string; motd: string[]; players?: { online: number; max: number } }>;
}

const timeout = () => AbortSignal.timeout(15000);

async function dataUri(url: string): Promise<string | undefined> {
  try {
    const r = await fetch(url, { headers: { "User-Agent": UA }, signal: timeout() });
    if (!r.ok) return undefined;
    const type = r.headers.get("content-type")?.split(";")[0] ?? "image/png";
    if (!type.startsWith("image/")) return undefined;
    const buf = Buffer.from(await r.arrayBuffer());
    if (buf.length > 400_000) return undefined; // keep the preview light
    return `data:${type};base64,${buf.toString("base64")}`;
  } catch {
    return undefined;
  }
}

async function fetchProjects(slugs: string[], into: Media["projects"]) {
  const url = `https://api.modrinth.com/v2/projects?ids=${encodeURIComponent(JSON.stringify(slugs))}`;
  const r = await fetch(url, { headers: { "User-Agent": UA }, signal: timeout() });
  if (!r.ok) throw new Error(`Modrinth ${r.status}`);
  const list = (await r.json()) as any[];
  await Promise.all(
    list.map(async (p) => {
      const gallery = [...(p.gallery ?? [])].sort((a, b) => Number(b.featured) - Number(a.featured) || a.ordering - b.ordering);
      const imgs = (await Promise.all(gallery.slice(0, 3).map((g: any) => dataUri(g.url)))).filter(Boolean) as string[];
      const entry = { icon: p.icon_url ? await dataUri(p.icon_url) : undefined, banner: imgs[0], gallery: imgs };
      // Index by slug; the requested slug may differ in case, so also index by id.
      into[p.slug] = entry;
      into[p.id] = entry;
    }),
  );
}

async function fetchServer(address: string): Promise<Media["servers"][string] | undefined> {
  const r = await fetch(`https://api.mcsrvstat.us/3/${address}`, { headers: { "User-Agent": UA }, signal: timeout() });
  if (!r.ok) throw new Error(`mcsrvstat ${r.status}`);
  const d = (await r.json()) as any;
  if (!d.online) return { motd: [] };
  return {
    icon: typeof d.icon === "string" && d.icon.startsWith("data:image/") ? d.icon : undefined,
    motd: (d.motd?.raw ?? []).map((l: string) => l.trim()).filter(Boolean).slice(0, 2),
    players: d.players ? { online: d.players.online, max: d.players.max } : undefined,
  };
}

async function build(): Promise<Media> {
  const src = JSON.parse(readFileSync(SOURCES, "utf8")) as { projects: string[]; servers: string[] };
  const cached: Media | null = existsSync(CACHE) ? JSON.parse(readFileSync(CACHE, "utf8")) : null;
  if (cached?.fetchedAt && src.projects.every((s) => s in cached.projects) && src.servers.every((s) => s in cached.servers)) return cached;

  const media: Media = { fetchedAt: null, projects: {}, servers: {} };
  const errors: string[] = [];
  await fetchProjects(src.projects, media.projects).catch((e) => errors.push(`projects: ${e.cause?.message ?? e.message}`));
  await Promise.all(
    src.servers.map(async (a) => {
      try {
        const s = await fetchServer(a);
        if (s) media.servers[a] = s;
      } catch (e: any) {
        errors.push(`${a}: ${e.cause?.message ?? e.message}`);
      }
    }),
  );
  if (errors.length) {
    console.warn(`[mock-media] some public media could not be fetched; the UI will use fallbacks.\n  ${errors.join("\n  ")}`);
  } else {
    media.fetchedAt = new Date().toISOString();
    mkdirSync(dirname(CACHE), { recursive: true });
    writeFileSync(CACHE, JSON.stringify(media));
  }
  return media;
}

export default function mockMedia(): Plugin {
  let pending: Promise<Media> | null = null;
  return {
    name: "halo-mock-media",
    resolveId: (id) => (id === ID ? RESOLVED : null),
    async load(id) {
      if (id !== RESOLVED) return null;
      pending ??= build();
      return `export default ${JSON.stringify(await pending)};`;
    },
  };
}
