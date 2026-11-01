import { createEffect } from "solid-js";
import { createStore } from "solid-js/store";
import { ACCOUNTS, INSTANCES, type Account, type Instance } from "./mock";

export type ThemeMode = "dark" | "light";

export interface Settings {
  theme: ThemeMode;
  accentHue: number;
  animatedBackground: boolean;
  defaultRamMb: number;
  downloadThreads: number;
  closeOnLaunch: boolean;
  discordRpc: boolean;
  crashReports: boolean;
  autoJava: boolean;
  preflight: boolean;
  showReservedSlots: boolean;
  browseView: "grid" | "list";
}

const DEFAULTS: Settings = {
  theme: "dark",
  accentHue: 275,
  animatedBackground: true,
  defaultRamMb: 4096,
  downloadThreads: 12,
  closeOnLaunch: false,
  discordRpc: true,
  crashReports: true,
  autoJava: true,
  preflight: true,
  showReservedSlots: false,
  browseView: "grid",
};

const KEY = "halo.settings.v1";

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...DEFAULTS, ...JSON.parse(raw) };
  } catch {
    /* private mode / blocked storage: fall back to defaults */
  }
  return { ...DEFAULTS };
}

export const [settings, setSettings] = createStore<Settings>(load());

export function initThemeSync() {
  createEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = settings.theme;
    root.style.setProperty("--accent-h", String(settings.accentHue));
    try {
      localStorage.setItem(KEY, JSON.stringify(settings));
    } catch {
      /* ignore */
    }
  });
}

// ---- App state (mock until the Rust backend lands) ----
interface AppState {
  accounts: Account[];
  activeAccountId: string;
  instances: Instance[];
  selectedInstanceId: string;
  launch: { instanceId: string | null; phase: "idle" | "preflight" | "launching" | "running"; progress: number; startedAt: number; bootMs: number | null };
  toasts: { id: number; text: string; kind: "ok" | "info" | "warn" }[];
}

export const [app, setApp] = createStore<AppState>({
  accounts: ACCOUNTS,
  activeAccountId: ACCOUNTS[0].id,
  instances: INSTANCES,
  selectedInstanceId: INSTANCES[0].id,
  launch: { instanceId: null, phase: "idle", progress: 0, startedAt: 0, bootMs: null },
  toasts: [],
});

export const activeAccount = () => app.accounts.find((a) => a.id === app.activeAccountId) ?? app.accounts[0];
export const selectedInstance = () => app.instances.find((i) => i.id === app.selectedInstanceId) ?? app.instances[0];

let toastId = 0;
export function toast(text: string, kind: "ok" | "info" | "warn" = "info") {
  const id = ++toastId;
  setApp("toasts", (t) => [...t, { id, text, kind }]);
  setTimeout(() => setApp("toasts", (t) => t.filter((x) => x.id !== id)), 3400);
}

/** Mock launch sequence: preflight → launching (progress) → running. */
export function launchInstance(id: string) {
  if (app.launch.phase !== "idle") {
    if (app.launch.phase === "running" && app.launch.instanceId === id) {
      setApp("launch", { instanceId: null, phase: "idle", progress: 0 });
      toast("Game closed", "info");
    }
    return;
  }
  const inst = app.instances.find((i) => i.id === id);
  setApp("launch", { instanceId: id, phase: "preflight", progress: 0, startedAt: performance.now(), bootMs: null });
  setTimeout(() => {
    setApp("launch", "phase", "launching");
    const t = setInterval(() => {
      setApp("launch", "progress", (p) => Math.min(100, p + 7 + Math.random() * 9));
      if (app.launch.progress >= 100) {
        clearInterval(t);
        const boot = inst?.lastBootMs ?? 4200;
        setApp("launch", { phase: "running", bootMs: boot });
        setApp("instances", (i) => i.id === id, "lastPlayed", "Just now");
        toast(`${inst?.name ?? "Instance"} reached title screen in ${(boot / 1000).toFixed(1)}s`, "ok");
      }
    }, 90);
  }, 450);
}
