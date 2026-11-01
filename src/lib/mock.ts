// Mock data for the UI mockup phase. Every shape here mirrors what the Rust
// backend will return over Tauri IPC, so pages won't change when it's wired up.

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";

export interface Account {
  id: string;
  kind: "microsoft" | "offline";
  username: string;
  uuid: string;
  skinSeed: number;
  capes: string[];
  activeCape: string | null;
  status: "ok" | "refresh" | "expired";
}

export interface InstalledMod {
  id: string;
  /** Modrinth slug, used for the real project icon. */
  slug?: string;
  name: string;
  version: string;
  enabled: boolean;
  perf?: boolean;
  hud?: boolean;
  update?: string;
}

export interface Instance {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion: string;
  hue: number;
  icon: string;
  /** Use a server's own favicon as the instance icon. */
  iconServer?: string;
  playtimeH: number;
  lastPlayed: string;
  ramMb: number;
  java: string;
  lastBootMs: number | null;
  vanillaBootMs: number | null;
  perfProfile: boolean;
  mods: InstalledMod[];
  screenshots: number;
  group?: string;
}

/**
 * A server the player actually joined. The backend records these from
 * quick-joins and from "Connecting to <host>" lines in game logs. Icon, MOTD
 * and player count come from a live Server List Ping, never from this list.
 */
export interface RecentServer {
  id: string;
  name: string;
  address: string;
  instanceId: string;
  lastPlayed: string;
  tag: string;
}

export interface NewsItem {
  id: string;
  title: string;
  body: string;
  date: string;
  tag: "Halo" | "Update" | "Snapshot" | "Featured";
  hue: number;
  /** Use this Modrinth project's gallery image as the article picture. */
  imageProject?: string;
}

export interface Project {
  id: string;
  slug: string;
  title: string;
  author: string;
  description: string;
  downloads: number;
  follows: number;
  updated: string;
  categories: string[];
  loaders: Loader[];
  hue: number;
  icon: string;
  client: "required" | "optional" | "unsupported";
  server: "required" | "optional" | "unsupported";
  deps?: string[];
  source: "modrinth" | "curseforge";
}

export const PERF_STACK = [
  { id: "sodium", slug: "sodium", name: "Sodium", why: "Modern rendering engine. Huge FPS gains." },
  { id: "lithium", slug: "lithium", name: "Lithium", why: "Optimizes game logic, physics and AI." },
  { id: "ferritecore", slug: "ferrite-core", name: "FerriteCore", why: "Cuts memory use by a lot." },
  { id: "modernfix", slug: "modernfix", name: "ModernFix", why: "Big load-time win: faster boot and world join." },
  { id: "immediatelyfast", slug: "immediatelyfast", name: "ImmediatelyFast", why: "Speeds up immediate-mode rendering (HUD, text)." },
  { id: "entityculling", slug: "entityculling", name: "Entity Culling", why: "Skips rendering entities you can't see." },
] as const;

const perfMods = (on = true): InstalledMod[] =>
  PERF_STACK.map((m) => ({ id: m.id, slug: m.slug, name: m.name, version: "latest", enabled: on, perf: true }));

export const ACCOUNTS: Account[] = [
  { id: "a1", kind: "microsoft", username: "LeoPlays", uuid: "8f1c…a92e", skinSeed: 275, capes: ["Migrator", "15th Anniversary"], activeCape: "Migrator", status: "ok" },
  { id: "a2", kind: "microsoft", username: "HaloTester", uuid: "1b07…44d0", skinSeed: 190, capes: [], activeCape: null, status: "refresh" },
  { id: "a3", kind: "offline", username: "LocalDev", uuid: "offline", skinSeed: 330, capes: [], activeCape: null, status: "ok" },
];

export const INSTANCES: Instance[] = [
  {
    id: "i1", name: "Halo PvP", mcVersion: "1.21.10", loader: "fabric", loaderVersion: "0.17.3", hue: 275, icon: "swords",
    playtimeH: 142, lastPlayed: "2 hours ago", ramMb: 4096, java: "Java 21 (managed)", lastBootMs: 6800, vanillaBootMs: 19400,
    perfProfile: true, screenshots: 38, group: "PvP",
    mods: [...perfMods(), { id: "halo-hud", name: "Halo HUD", version: "0.1.0", enabled: true, hud: true }, { id: "modmenu", slug: "modmenu", name: "Mod Menu", version: "16.0.0", enabled: true }],
  },
  {
    id: "i2", name: "Donut SMP", mcVersion: "1.21.10", loader: "fabric", loaderVersion: "0.17.3", hue: 200, icon: "coins", iconServer: "donutsmp.net",
    playtimeH: 87, lastPlayed: "Yesterday", ramMb: 5120, java: "Java 21 (managed)", lastBootMs: 7300, vanillaBootMs: 19400,
    perfProfile: true, screenshots: 12, group: "SMP",
    mods: [...perfMods(), { id: "iris", slug: "iris", name: "Iris Shaders", version: "1.9.6", enabled: true, update: "1.9.7" }, { id: "xaeros", slug: "xaeros-minimap", name: "Xaero's Minimap", version: "25.2.10", enabled: true }],
  },
  {
    id: "i3", name: "All the Mods 10", mcVersion: "1.21.1", loader: "neoforge", loaderVersion: "21.1.209", hue: 150, icon: "package",
    playtimeH: 31, lastPlayed: "3 days ago", ramMb: 8192, java: "Java 21 (managed)", lastBootMs: 71200, vanillaBootMs: null,
    perfProfile: false, screenshots: 6, group: "Modpacks",
    mods: [
      { id: "mekanism", name: "Mekanism", version: "10.7.15", enabled: true },
      { id: "ae2", slug: "ae2", name: "Applied Energistics 2", version: "19.2.17", enabled: true },
      { id: "modernfix-nf", slug: "modernfix", name: "ModernFix", version: "5.24.4", enabled: true, perf: true },
      { id: "jei", slug: "jei", name: "Just Enough Items", version: "19.25.0", enabled: true },
      { id: "create", slug: "create", name: "Create", version: "6.0.8", enabled: false },
    ],
  },
  {
    id: "i4", name: "Vanilla Latest", mcVersion: "1.21.10", loader: "vanilla", loaderVersion: "", hue: 30, icon: "box",
    playtimeH: 12, lastPlayed: "Last week", ramMb: 4096, java: "Java 21 (managed)", lastBootMs: 15900, vanillaBootMs: 19400,
    perfProfile: false, screenshots: 3, mods: [],
  },
  {
    id: "i5", name: "Quilt Testbed", mcVersion: "1.20.1", loader: "quilt", loaderVersion: "0.29.2", hue: 330, icon: "flask",
    playtimeH: 2, lastPlayed: "2 weeks ago", ramMb: 3072, java: "Java 17 (managed)", lastBootMs: null, vanillaBootMs: null,
    perfProfile: true, screenshots: 0, mods: perfMods(),
  },
  {
    id: "i6", name: "Forge 1.20.1 Pack", mcVersion: "1.20.1", loader: "forge", loaderVersion: "47.4.0", hue: 10, icon: "hammer",
    playtimeH: 9, lastPlayed: "Last month", ramMb: 6144, java: "Java 17 (managed)", lastBootMs: 48300, vanillaBootMs: null,
    perfProfile: false, screenshots: 4, mods: [{ id: "jei-f", slug: "jei", name: "Just Enough Items", version: "15.20.0", enabled: true }],
  },
];

// Servers this player has actually joined, newest first (from launcher join history).
export const RECENT_SERVERS: RecentServer[] = [
  { id: "s1", name: "Hypixel", address: "mc.hypixel.net", instanceId: "i1", lastPlayed: "2 hours ago", tag: "PvP" },
  { id: "s2", name: "Donut SMP", address: "donutsmp.net", instanceId: "i2", lastPlayed: "Yesterday", tag: "SMP" },
  { id: "s3", name: "Minemen Club", address: "minemen.club", instanceId: "i1", lastPlayed: "4 days ago", tag: "PvP" },
];

export const NEWS: NewsItem[] = [
  { id: "n1", title: "Halo Client 0.1 preview", body: "First look at the new glass UI, multi-account switching and the Performance profile. Tell us what you think.", date: "Nov 1", tag: "Halo", hue: 275 },
  { id: "n2", title: "Performance profile explained", body: "Why ModernFix, Sodium and friends cut boot time, and why every mod in the stack is safe on public servers.", date: "Oct 29", tag: "Featured", hue: 310, imageProject: "sodium" },
  { id: "n3", title: "Halo HUD companion mod", body: "FPS, CPS, keystrokes, zoom and armor HUD as a lightweight Fabric mod. Auto-installed when you want it.", date: "Oct 25", tag: "Halo", hue: 240 },
  { id: "n4", title: "Java 25 runtimes now managed", body: "New Minecraft versions pull the right Java automatically, verified by hash.", date: "Oct 20", tag: "Update", hue: 180 },
];

const P = (p: Omit<Project, "source"> & { source?: Project["source"] }): Project => ({ source: "modrinth", ...p });

export const PROJECTS: Project[] = [
  P({ id: "p1", slug: "sodium", title: "Sodium", author: "jellysquid3", description: "The fastest and most compatible rendering optimization mod for Minecraft.", downloads: 98_400_000, follows: 31_200, updated: "3 days ago", categories: ["optimization"], loaders: ["fabric", "quilt", "neoforge"], hue: 200, icon: "zap", client: "required", server: "unsupported" }),
  P({ id: "p2", slug: "iris", title: "Iris Shaders", author: "coderbot", description: "A modern shaders mod compatible with OptiFine shaders.", downloads: 71_000_000, follows: 22_300, updated: "1 week ago", categories: ["decoration", "optimization"], loaders: ["fabric", "quilt", "neoforge"], hue: 280, icon: "sparkles", client: "required", server: "unsupported", deps: ["Sodium"] }),
  P({ id: "p3", slug: "modernfix", title: "ModernFix", author: "embeddedt", description: "All-in-one mod that improves performance, reduces memory usage, and fixes many bugs.", downloads: 41_900_000, follows: 9_800, updated: "5 days ago", categories: ["optimization", "utility"], loaders: ["fabric", "quilt", "forge", "neoforge"], hue: 150, icon: "rocket", client: "optional", server: "optional" }),
  P({ id: "p4", slug: "lithium", title: "Lithium", author: "jellysquid3", description: "No-compromises game logic/server optimization mod.", downloads: 54_000_000, follows: 14_100, updated: "2 weeks ago", categories: ["optimization"], loaders: ["fabric", "quilt", "neoforge"], hue: 60, icon: "atom", client: "optional", server: "optional" }),
  P({ id: "p5", slug: "xaeros-minimap", title: "Xaero's Minimap", author: "xaero96", description: "Displays the nearby world terrain, players, mobs and entities on a minimap.", downloads: 38_200_000, follows: 8_400, updated: "4 days ago", categories: ["utility", "adventure"], loaders: ["fabric", "forge", "neoforge"], hue: 120, icon: "map", client: "required", server: "optional" }),
  P({ id: "p6", slug: "modmenu", title: "Mod Menu", author: "Prospector", description: "Adds a mod menu to view the list of mods you have installed.", downloads: 62_500_000, follows: 12_700, updated: "1 month ago", categories: ["utility", "library"], loaders: ["fabric", "quilt"], hue: 20, icon: "list", client: "required", server: "unsupported" }),
  P({ id: "p7", slug: "entityculling", title: "Entity Culling", author: "tr7zw", description: "Uses async path-tracing to hide block/entities that are not visible.", downloads: 35_600_000, follows: 6_300, updated: "6 days ago", categories: ["optimization"], loaders: ["fabric", "forge", "neoforge"], hue: 340, icon: "eye-off", client: "required", server: "unsupported" }),
  P({ id: "p8", slug: "appleskin", title: "AppleSkin", author: "squeek502", description: "Adds various food-related HUD improvements.", downloads: 44_800_000, follows: 7_900, updated: "3 weeks ago", categories: ["food", "utility"], loaders: ["fabric", "forge", "neoforge"], hue: 0, icon: "apple", client: "optional", server: "optional" }),
  P({ id: "p9", slug: "jei", title: "Just Enough Items", author: "mezz", description: "View items and recipes.", downloads: 300_000_000, follows: 20_100, updated: "2 days ago", categories: ["utility"], loaders: ["fabric", "forge", "neoforge"], hue: 220, icon: "book", client: "required", server: "optional", source: "curseforge" }),
];

export const MC_VERSIONS = ["1.21.10", "1.21.8", "1.21.5", "1.21.4", "1.21.1", "1.20.6", "1.20.4", "1.20.1", "1.19.4", "1.18.2", "1.16.5", "1.12.2", "1.8.9"];

export const LOADER_LABEL: Record<Loader, string> = { vanilla: "Vanilla", fabric: "Fabric", quilt: "Quilt", forge: "Forge", neoforge: "NeoForge" };

export const MOCK_LOG = `[22:41:03] [main/INFO]: Loading Minecraft 1.21.10 with Fabric Loader 0.17.3
[22:41:03] [main/INFO]: Loading 9 mods:
	- fabricloader 0.17.3
	- sodium 0.7.2
	- lithium 0.20.0
	- ferritecore 8.0.0
	- modernfix 5.24.4
	- immediatelyfast 1.12.1
	- entityculling 1.9.0
	- halo-hud 0.1.0
	- modmenu 16.0.0
[22:41:04] [main/INFO]: [ModernFix] Dynamic resources enabled
[22:41:05] [Render thread/INFO]: Environment: Environment[sessionHost=https://sessionserver.mojang.com, servicesHost=https://api.minecraftservices.com, name=PROD]
[22:41:06] [Render thread/INFO]: Setting user: LeoPlays
[22:41:07] [Render thread/INFO]: Backend library: LWJGL version 3.3.3
[22:41:08] [Render thread/INFO]: [Sodium] Using renderer: OpenGL 4.6 (NVIDIA)
[22:41:09] [Render thread/INFO]: [Halo HUD] Loaded 4 HUD modules
[22:41:09] [Render thread/INFO]: Reloading ResourceManager: vanilla, fabric
[22:41:10] [Render thread/INFO]: [Halo] Title screen reached in 6.8s
[22:41:12] [Render thread/INFO]: Connecting to mc.hypixel.net, 25565
[22:41:13] [Render thread/WARN]: Ignoring unknown attribute 'halo:test'`;

export const fmt = (n: number) =>
  n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(n >= 1e4 ? 0 : 1)}k` : String(n);
