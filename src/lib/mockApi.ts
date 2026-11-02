// In-memory stand-in for the Rust backend, used only outside Tauri (browser
// preview / `pnpm dev`). Data is clearly sample data; images come from the
// build-time public media snapshot when available.

import media from "virtual:mock-media";
import type * as T from "./types";

type Handler = (payload: any) => void;
const listeners = new Map<string, Set<Handler>>();
export function mockOn(event: string, cb: Handler) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => listeners.get(event)!.delete(cb);
}
function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb(payload));
}

const now = Date.now();
const H = 3600_000;
const MC = "26.3";

function inst(p: Partial<T.Instance> & Pick<T.Instance, "id" | "name">): T.Instance {
  return {
    mcVersion: MC, loader: "fabric", loaderVersion: "0.20.1", versionId: null, icon: "box", iconServer: null, iconPath: null, hue: 275, group: null,
    created: now - 30 * 24 * H, lastPlayed: null, playtimeSecs: 0, ramMb: 4096, javaPath: null, jvmArgs: "", width: null, height: null,
    preLaunch: null, postExit: null, accountId: null, perfProfile: false, cds: true, tunedFlags: true, lastBootMs: null, bootHistory: [],
    running: false, vanillaBootMs: null, hudSupported: true, ...p,
  };
}

const instances: T.Instance[] = [
  inst({ id: "halo-pvp", name: "Halo PvP", icon: "swords", hue: 275, lastPlayed: now - 2 * H, playtimeSecs: 142 * 3600, perfProfile: true, lastBootMs: 6800, vanillaBootMs: 19400, group: "PvP", bootHistory: [9100, 8400, 7900, 7200, 7000, 6800].map((ms, i) => ({ ms, at: now - (6 - i) * 24 * H, cds: i > 0, perf: true })) }),
  inst({ id: "donut-smp", name: "Donut SMP", icon: "coins", hue: 200, iconServer: "donutsmp.net", lastPlayed: now - 26 * H, playtimeSecs: 87 * 3600, perfProfile: true, lastBootMs: 7300, vanillaBootMs: 19400, ramMb: 5120, group: "SMP" }),
  inst({ id: "atm10", name: "All the Mods 10", icon: "package", hue: 150, mcVersion: "1.21.1", loader: "neoforge", loaderVersion: "21.1.209", lastPlayed: now - 72 * H, playtimeSecs: 31 * 3600, lastBootMs: 71200, ramMb: 8192, hudSupported: false, group: "Modpacks" }),
  inst({ id: "vanilla", name: "Vanilla", icon: "box", hue: 30, loader: "vanilla", loaderVersion: null, lastPlayed: now - 7 * 24 * H, playtimeSecs: 12 * 3600, lastBootMs: 19400, cds: false, tunedFlags: false, hudSupported: false }),
];

const PERF = [
  ["sodium", "Sodium", "0.8.0"], ["lithium", "Lithium", "0.21.0"], ["ferrite-core", "FerriteCore", "8.1.0"],
  ["modernfix", "ModernFix", "5.26.0"], ["immediatelyfast", "ImmediatelyFast", "1.14.0"], ["entityculling", "Entity Culling", "1.10.0"],
] as const;
const mod = (slug: string, name: string, version: string, extra: Partial<T.ContentMeta> = {}, enabled = true): T.ContentItem => ({
  key: `mods/${slug}-${version}.jar`, kind: "mod", fileName: `${slug}-${version}.jar${enabled ? "" : ".disabled"}`, name, version, enabled, size: 400_000,
  meta: { source: "modrinth", slug, title: name, versionNumber: version, iconUrl: media.projects[slug]?.icon ?? null, perf: false, hud: false, dependency: false, ...extra },
});
const perfContent = () => PERF.map(([s, n, v]) => mod(s, n, v, { perf: true }));
const content: Record<string, T.ContentItem[]> = {
  "halo-pvp": [...perfContent(), mod("halo-hud", "Halo HUD", "0.1.0", { source: "halo", hud: true }), mod("modmenu", "Mod Menu", "17.0.0")],
  "donut-smp": [...perfContent(), mod("iris", "Iris Shaders", "1.10.0"), mod("xaeros-minimap", "Xaero's Minimap", "25.3.0")],
  atm10: [mod("ae2", "Applied Energistics 2", "19.2.17"), mod("modernfix", "ModernFix", "5.24.4", { perf: true }), mod("jei", "Just Enough Items", "19.25.0"), mod("create", "Create", "6.0.8", {}, false)],
  vanilla: [],
};

const accounts: T.Account[] = [
  { id: "a1", kind: "microsoft", username: "LeoPlays", uuid: "8f1c0e2ba92e", capes: [{ id: "c1", alias: "Migrator", url: "", active: true }, { id: "c2", alias: "15th Anniversary", url: "", active: false }], expiresAt: now + 20 * H },
  { id: "a2", kind: "microsoft", username: "HaloTester", uuid: "1b0744d0aa11", capes: [], expiresAt: now - H },
  { id: "a3", kind: "offline", username: "LocalDev", uuid: "offline", capes: [], expiresAt: Number.MAX_SAFE_INTEGER },
];
let activeId = "a1";

const recent: T.RecentServer[] = [
  { address: "mc.hypixel.net", name: "Hypixel", instanceId: "halo-pvp", lastPlayed: now - 2 * H, joins: 41 },
  { address: "donutsmp.net", name: "Donut SMP", instanceId: "donut-smp", lastPlayed: now - 26 * H, joins: 18 },
  { address: "minemen.club", name: "Minemen Club", instanceId: "halo-pvp", lastPlayed: now - 96 * H, joins: 6 },
];

type P = { slug: string; title: string; author: string; description: string; downloads: number; follows: number; days: number; categories: string[]; client: string; server: string; type: T.ContentKind; source?: "modrinth" | "curseforge" };
const PROJECTS: P[] = [
  { slug: "sodium", title: "Sodium", author: "jellysquid3", description: "The fastest and most compatible rendering optimization mod for Minecraft. Now available for both NeoForge and Fabric!", downloads: 104_200_000, follows: 33_100, days: 3, categories: ["optimization"], client: "required", server: "unsupported", type: "mod" },
  { slug: "iris", title: "Iris Shaders", author: "coderbot", description: "A modern shader pack loader for Minecraft intended to be compatible with existing OptiFine shader packs.", downloads: 75_400_000, follows: 23_900, days: 6, categories: ["decoration", "optimization"], client: "required", server: "unsupported", type: "mod" },
  { slug: "modernfix", title: "ModernFix", author: "embeddedt", description: "All-in-one mod that improves performance, reduces memory usage, and fixes many bugs. Compatible with ModernFix's own launch-time optimizations.", downloads: 45_100_000, follows: 10_400, days: 5, categories: ["optimization", "utility"], client: "optional", server: "optional", type: "mod" },
  { slug: "lithium", title: "Lithium", author: "jellysquid3", description: "No-compromises game logic optimization mod. Well suited for clients and servers of all kinds.", downloads: 58_300_000, follows: 15_000, days: 12, categories: ["optimization"], client: "optional", server: "optional", type: "mod" },
  { slug: "xaeros-minimap", title: "Xaero's Minimap", author: "thexaero", description: "Displays the nearby world terrain, players, mobs, entities in the corner of your screen. Lets you create waypoints.", downloads: 40_900_000, follows: 9_100, days: 4, categories: ["utility", "adventure"], client: "required", server: "optional", type: "mod" },
  { slug: "modmenu", title: "Mod Menu", author: "Prospector", description: "Adds a mod menu to view the list of mods you have installed.", downloads: 66_800_000, follows: 13_500, days: 30, categories: ["utility", "library"], client: "required", server: "unsupported", type: "mod" },
  { slug: "entityculling", title: "Entity Culling", author: "tr7zw", description: "Using async path-tracing to hide Block-/Entities that are not visible.", downloads: 38_400_000, follows: 6_800, days: 7, categories: ["optimization"], client: "required", server: "unsupported", type: "mod" },
  { slug: "appleskin", title: "AppleSkin", author: "squeek502", description: "Adds various food-related HUD improvements.", downloads: 47_900_000, follows: 8_300, days: 21, categories: ["food", "utility"], client: "optional", server: "optional", type: "mod" },
  { slug: "immediatelyfast", title: "ImmediatelyFast", author: "RaphiMC", description: "Speed up immediate mode rendering in Minecraft.", downloads: 39_000_000, follows: 5_200, days: 9, categories: ["optimization"], client: "required", server: "unsupported", type: "mod" },
  { slug: "ferrite-core", title: "FerriteCore", author: "malte0811", description: "Memory usage optimizations.", downloads: 61_000_000, follows: 9_900, days: 40, categories: ["optimization"], client: "optional", server: "optional", type: "mod" },
];

const asHit = (p: P): T.SearchHit => ({
  source: p.source ?? "modrinth", id: p.slug, slug: p.slug, title: p.title, author: p.author, description: p.description,
  iconUrl: media.projects[p.slug]?.icon ?? null, bannerUrl: media.projects[p.slug]?.banner ?? null,
  downloads: p.downloads, follows: p.follows, updated: new Date(now - p.days * 24 * H).toISOString(), categories: p.categories,
  clientSide: p.client, serverSide: p.server, projectType: p.type,
});

let settings: T.Settings = {
  theme: "dark", accentHue: 275, animatedBackground: true, defaultRamMb: 4096, downloadThreads: 12, closeOnLaunch: false, discordRpc: true,
  crashReports: true, crashReportsPrompted: true, autoUpdate: true, autoJava: true, preflight: true, showReservedSlots: false, browseView: "list",
  activeAccountId: "a1", lastInstanceId: "halo-pvp", msaClientId: null,
};
try {
  const raw = localStorage.getItem("halo.settings.v2");
  if (raw) settings = { ...settings, ...JSON.parse(raw) };
} catch {
  /* storage blocked */
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const running = new Set<string>();

async function simulateLaunch(id: string, server: string | null) {
  const i = instances.find((x) => x.id === id)!;
  const t0 = performance.now();
  emit("launch-state", { instanceId: id, phase: "preparing", message: "Checking account" });
  await sleep(250);
  emit("launch-state", { instanceId: id, phase: "preparing", message: "Checking game files" });
  for (let d = 0; d <= 40; d += 8) {
    emit("task-progress", { task: "game-files", label: "Checking game files", done: d, total: 40 });
    await sleep(70);
  }
  emit("launch-state", { instanceId: id, phase: "preparing", message: "Starting Minecraft" });
  await sleep(250);
  running.add(id);
  i.running = true;
  i.lastPlayed = Date.now();
  emit("launch-state", { instanceId: id, phase: "running", message: "Game running" });
  const lines = [
    `[main/INFO]: Loading Minecraft ${i.mcVersion} with ${i.loader === "vanilla" ? "vanilla" : `${i.loader} ${i.loaderVersion}`}`,
    "[main/INFO]: [ModernFix] Dynamic resources enabled",
    "[Render thread/INFO]: Setting user: LeoPlays",
    "[Render thread/INFO]: Backend library: LWJGL version 3.4.0",
    "[Render thread/INFO]: [Halo HUD] registered HUD element halohud:hud",
    "[Render thread/INFO]: Reloading ResourceManager: vanilla, fabric",
    "[Render thread/INFO]: Sound engine started",
  ];
  for (const l of lines) {
    emit("game-log", { instanceId: id, lines: [`[${new Date().toLocaleTimeString()}] ${l}`] });
    await sleep(160);
  }
  const boot = Math.round(performance.now() - t0 + (i.lastBootMs ?? 6000) - 1600);
  i.lastBootMs = boot;
  i.bootHistory = [...i.bootHistory, { ms: boot, at: Date.now(), cds: i.cds, perf: i.perfProfile }].slice(-30);
  emit("game-boot", { instanceId: id, bootMs: boot });
  if (server) {
    emit("game-log", { instanceId: id, lines: [`[${new Date().toLocaleTimeString()}] [Render thread/INFO]: Connecting to ${server}, 25565`] });
    const r = recent.find((x) => x.address === server);
    if (r) r.lastPlayed = Date.now();
  }
}

export async function mockCall(cmd: string, a: Record<string, any>): Promise<unknown> {
  await sleep(40);
  switch (cmd) {
    case "app_info":
      return { version: "0.1.0", dataDir: "%APPDATA%\\HaloClient", platform: "windows", msaConfigured: true, curseforge: false, discord: true, sentry: false, hud: { version: "0.1.0", minecraft: [MC], file: "halo-hud-0.1.0.jar" } } satisfies T.AppInfo;
    case "get_settings":
      return settings;
    case "save_settings":
      settings = { ...a.settings };
      try {
        localStorage.setItem("halo.settings.v2", JSON.stringify(settings));
      } catch {
        /* ignore */
      }
      return;
    case "preflight":
      await sleep(500);
      return { accountOk: true, accountMessage: null, instanceId: settings.lastInstanceId, instanceReady: true } satisfies T.Preflight;
    case "prepare_update":
      return null;
    case "apply_update":
      return;
    case "check_update":
      return { current: "0.1.0", latest: "0.1.0", available: false, url: null } satisfies T.UpdateCheck;
    case "game_versions":
      return [MC, "26.2", "26.1", "1.21.11", "1.21.10", "1.21.8", "1.21.5", "1.21.4", "1.21.1", "1.20.6", "1.20.4", "1.20.1", "1.19.4", "1.18.2", "1.16.5", "1.12.2", "1.8.9"].map((id) => ({ id, kind: "release", releaseTime: "" }));
    case "loader_versions":
      return a.loader === "vanilla" ? [] : [{ version: { fabric: "0.20.1", quilt: "0.30.0", forge: "64.0.2", neoforge: "26.3.4" }[a.loader as string] ?? "1.0", recommended: true }, { version: "older", recommended: false }];
    case "list_accounts":
      return { accounts, activeId };
    case "set_active_account":
      activeId = a.id;
      return;
    case "auth_start":
      return { flowId: "f1", userCode: "HALO-7Q2K", verificationUri: "https://www.microsoft.com/link", expiresIn: 900 } satisfies T.AuthStart;
    case "auth_poll": {
      await sleep(2500);
      const acc: T.Account = { id: `a${Date.now()}`, kind: "microsoft", username: "NewAlt", uuid: "c0def00d", capes: [], expiresAt: Date.now() + 20 * H };
      accounts.push(acc);
      activeId = acc.id;
      return acc;
    }
    case "auth_cancel":
      return;
    case "add_offline": {
      if (!/^[A-Za-z0-9_]{3,16}$/.test(a.name)) throw "Usernames are 3–16 characters: letters, numbers and _.";
      const acc: T.Account = { id: `offline-${a.name.toLowerCase()}`, kind: "offline", username: a.name, uuid: "offline", capes: [], expiresAt: Number.MAX_SAFE_INTEGER };
      accounts.push(acc);
      return acc;
    }
    case "remove_account": {
      const i = accounts.findIndex((x) => x.id === a.id);
      if (i >= 0) accounts.splice(i, 1);
      if (activeId === a.id) activeId = accounts[0]?.id ?? "";
      return;
    }
    case "account_textures":
      return { skin: null, cape: null, slim: false } satisfies T.Textures;
    case "upload_skin":
    case "set_cape": {
      const acc = accounts.find((x) => x.id === a.id)!;
      if (cmd === "set_cape") acc.capes.forEach((c) => (c.active = c.id === a.capeId));
      return acc;
    }
    case "list_instances":
      return instances.map((i) => ({ ...i, running: running.has(i.id) }));
    case "create_instance": {
      const s = a.spec as T.CreateSpec;
      const id = `${s.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-${Math.random().toString(16).slice(2, 6)}`;
      const i = inst({ id, name: s.name, mcVersion: s.mcVersion, loader: s.loader, loaderVersion: s.loaderVersion, icon: s.icon ?? "box", hue: s.hue ?? 275, perfProfile: s.perfMods.length > 0, hudSupported: s.loader === "fabric" && s.mcVersion === MC, created: Date.now() });
      instances.unshift(i);
      content[id] = [...PERF.filter(([slug]) => s.perfMods.includes(slug)).map(([sl, n, v]) => mod(sl, n, v, { perf: true })), ...(s.haloHud && i.hudSupported ? [mod("halo-hud", "Halo HUD", "0.1.0", { source: "halo", hud: true })] : [])];
      for (let d = 0; d <= s.perfMods.length; d++) {
        emit("task-progress", { task: "content", label: "Installing Performance profile", done: d, total: s.perfMods.length || 1 });
        await sleep(120);
      }
      return { instance: i, warnings: [] };
    }
    case "update_instance": {
      const idx = instances.findIndex((x) => x.id === a.instance.id);
      instances[idx] = { ...instances[idx], ...a.instance };
      return instances[idx];
    }
    case "delete_instance": {
      const idx = instances.findIndex((x) => x.id === a.id);
      if (idx >= 0) instances.splice(idx, 1);
      return;
    }
    case "duplicate_instance": {
      const src = instances.find((x) => x.id === a.id)!;
      const copy = { ...src, id: `${src.id}-copy${Math.random().toString(16).slice(2, 5)}`, name: `${src.name} (copy)`, playtimeSecs: 0, lastPlayed: null };
      instances.push(copy);
      content[copy.id] = [...(content[src.id] ?? [])];
      return copy;
    }
    case "list_content":
      return content[a.id] ?? [];
    case "toggle_content": {
      const c = content[a.id]?.find((x) => x.key === a.key);
      if (c) {
        c.enabled = a.enabled;
        c.fileName = c.key.split("/")[1] + (a.enabled ? "" : ".disabled");
      }
      return;
    }
    case "remove_content":
      content[a.id] = (content[a.id] ?? []).filter((x) => x.key !== a.key);
      return;
    case "check_content_updates":
      await sleep(600);
      return (content[a.id] ?? []).filter((c) => c.meta?.slug === "iris").map((c) => ({ key: c.key, projectId: "iris", current: c.version ?? "", latest: "1.10.1", latestVersionId: "v2" })) satisfies T.UpdateInfo[];
    case "apply_content_updates":
      (content[a.id] ?? []).forEach((c) => c.meta?.slug === "iris" && (c.version = "1.10.1"));
      return { installed: ["Iris Shaders"], warnings: [], needsShaderLoader: null, manualUrl: null };
    case "install_project": {
      const r = a.req;
      const p = PROJECTS.find((x) => x.slug === r.projectId);
      emit("task-progress", { task: "content", label: `Installing ${p?.title ?? r.projectId}`, done: 0, total: 1 });
      await sleep(700);
      emit("task-progress", { task: "content", label: `Installing ${p?.title ?? r.projectId}`, done: 1, total: 1 });
      const list = (content[r.instanceId] ??= []);
      const installed = [p?.title ?? r.projectId];
      if (r.projectId === "iris" && !list.some((c) => c.meta?.slug === "sodium")) {
        list.push(mod("sodium", "Sodium", "0.8.0", { dependency: true }));
        installed.push("Sodium");
      }
      list.push(mod(r.projectId, p?.title ?? r.projectId, "latest"));
      return { installed, warnings: [], needsShaderLoader: null, manualUrl: null } satisfies T.InstallResult;
    }
    case "install_perf":
    case "install_hud":
      return { installed: [], warnings: [], needsShaderLoader: null, manualUrl: null };
    case "screenshots":
      return [];
    case "worlds":
      return ["New World", "Skyblock"];
    case "instance_dir":
      return `%APPDATA%\\HaloClient\\instances\\${a.id}\\minecraft`;
    case "default_minecraft_dir":
      return "%APPDATA%\\.minecraft";
    case "launch_instance":
      if (running.has(a.id)) throw "This instance is already running.";
      simulateLaunch(a.id, a.server);
      return;
    case "stop_instance": {
      running.delete(a.id);
      const i = instances.find((x) => x.id === a.id);
      if (i) i.running = false;
      emit("game-exit", { instanceId: a.id, code: 0, playedSecs: 60, bootMs: i?.lastBootMs ?? null, crash: null, killed: true } satisfies T.GameExit);
      emit("launch-state", { instanceId: a.id, phase: "stopped", message: "Game closed" });
      return;
    }
    case "running_instances":
      return [...running];
    case "game_log":
      return [
        `[22:41:03] [main/INFO]: Loading Minecraft ${MC} with Fabric Loader 0.20.1`,
        "[22:41:04] [main/INFO]: [ModernFix] Dynamic resources enabled",
        "[22:41:06] [Render thread/INFO]: Setting user: LeoPlays",
        "[22:41:08] [Render thread/INFO]: [Sodium] Using renderer: OpenGL 4.6",
        "[22:41:10] [Render thread/INFO]: Sound engine started",
        "[22:41:12] [Render thread/INFO]: Connecting to mc.hypixel.net, 25565",
        "[22:41:13] [Render thread/WARN]: Ignoring unknown attribute 'halo:test'",
      ];
    case "recent_servers":
      return [...recent].sort((x, y) => y.lastPlayed - x.lastPlayed);
    case "forget_recent_server": {
      const i = recent.findIndex((r) => r.address === a.address);
      if (i >= 0) recent.splice(i, 1);
      return;
    }
    case "ping_server": {
      const s = media.servers[a.address];
      if (!s) return { online: false, motd: [], playersOnline: 0, playersMax: 0, favicon: null, version: null, pingMs: null } satisfies T.ServerStatus;
      return { online: true, motd: s.motd, playersOnline: s.players?.online ?? 0, playersMax: s.players?.max ?? 0, favicon: s.icon ?? null, version: null, pingMs: null } satisfies T.ServerStatus;
    }
    case "server_list":
      return [{ name: "Hypixel", ip: "mc.hypixel.net" }, { name: "Donut SMP", ip: "donutsmp.net" }];
    case "save_server_list":
      return;
    case "search_content": {
      const q = a.query as T.SearchQuery;
      let hits = PROJECTS.filter((p) => p.type === q.projectType || (q.projectType === "mod" && p.type === "mod"));
      if (q.projectType !== "mod") hits = [];
      if (q.query) hits = hits.filter((p) => (p.title + p.description).toLowerCase().includes(q.query.toLowerCase()));
      for (const c of q.categories) hits = hits.filter((p) => p.categories.includes(c));
      if (q.environment === "server") hits = hits.filter((p) => p.server !== "unsupported");
      if (q.sort === "downloads") hits = [...hits].sort((x, y) => y.downloads - x.downloads);
      if (q.sort === "follows") hits = [...hits].sort((x, y) => y.follows - x.follows);
      if (q.sort === "updated") hits = [...hits].sort((x, y) => x.days - y.days);
      await sleep(150);
      return { hits: hits.map(asHit), total: hits.length } satisfies T.SearchResult;
    }
    case "get_project": {
      const p = PROJECTS.find((x) => x.slug === a.id) ?? PROJECTS[0];
      const m = media.projects[p.slug];
      return {
        id: p.slug, slug: p.slug, title: p.title, description: p.description, author: p.author, downloads: p.downloads, followers: p.follows,
        icon_url: m?.icon ?? null, project_type: p.type, client_side: p.client, server_side: p.server, categories: p.categories,
        loaders: ["fabric", "quilt", "neoforge"], game_versions: [MC, "1.21.11", "1.21.10", "1.21.1"], license: { id: "LGPL-3.0-only", name: "GNU LGPL v3" },
        source_url: "https://github.com/", issues_url: "https://github.com/", discord_url: null, updated: new Date(now - p.days * 24 * H).toISOString(),
        gallery: (m?.gallery ?? []).map((url, i) => ({ url, title: `${p.title} screenshot ${i + 1}`, featured: i === 0 })),
        body: `## About ${p.title}\n\n${p.description}\n\nThis is sample text for the preview. In the app this tab shows the project's real description from Modrinth, rendered from Markdown and sanitized.\n\n### Features\n- Works with the **Halo Performance profile**\n- Client-side and safe on public servers\n- Updated for Minecraft ${MC}\n\n> Install it with one click; required dependencies come along automatically.`,
      };
    }
    case "get_project_versions":
      return ["1.10.0", "1.9.6", "1.9.5"].map((v, i) => ({
        id: `v${i}`, name: `${v} for ${MC}`, version_number: v, version_type: i === 2 ? "beta" : "release", loaders: ["fabric", "quilt"], game_versions: [MC],
        date_published: new Date(now - (i + 1) * 9 * 24 * H).toISOString(), downloads: 900_000 - i * 200_000,
      }));
    default:
      return null;
  }
}
