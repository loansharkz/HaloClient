export const fmt = (n: number) =>
  n >= 1e9 ? `${(n / 1e9).toFixed(1)}B` : n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(n >= 1e4 ? 0 : 1)}k` : String(n);

export function ago(ts: number | string | null | undefined): string {
  if (ts == null || ts === "") return "Never";
  const t = typeof ts === "string" ? Date.parse(ts) : ts;
  if (!Number.isFinite(t)) return "—";
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 60) return "Just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  if (s < 86400 * 2) return "Yesterday";
  if (s < 86400 * 30) return `${Math.floor(s / 86400)} days ago`;
  if (s < 86400 * 365) return `${Math.floor(s / (86400 * 30))} months ago`;
  return `${Math.floor(s / (86400 * 365))} years ago`;
}

export const hours = (secs: number) => (secs < 3600 ? `${Math.round(secs / 60)} min` : `${(secs / 3600).toFixed(secs < 36000 ? 1 : 0)} h`);
export const seconds = (ms: number | null | undefined) => (ms == null ? "—" : `${(ms / 1000).toFixed(1)}s`);
export const bytes = (n: number) => (n > 1e6 ? `${(n / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);

export const LOADER_LABEL: Record<string, string> = { vanilla: "Vanilla", fabric: "Fabric", quilt: "Quilt", forge: "Forge", neoforge: "NeoForge" };

export const PERF_STACK = [
  { slug: "sodium", name: "Sodium", why: "Modern renderer, big FPS gains" },
  { slug: "lithium", name: "Lithium", why: "Faster game logic and physics" },
  { slug: "ferrite-core", name: "FerriteCore", why: "Much lower memory use" },
  { slug: "modernfix", name: "ModernFix", why: "Faster boot and world join" },
  { slug: "immediatelyfast", name: "ImmediatelyFast", why: "Faster HUD and text rendering" },
  { slug: "entityculling", name: "Entity Culling", why: "Skips entities you can't see" },
] as const;
