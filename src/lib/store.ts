import { createEffect, on as onChange } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { api, on } from "./api";
import type * as T from "./types";

const DEFAULT_SETTINGS: T.Settings = {
  theme: "dark",
  accentHue: 275,
  animatedBackground: true,
  defaultRamMb: 4096,
  downloadThreads: 12,
  closeOnLaunch: false,
  discordRpc: true,
  crashReports: true,
  crashReportsPrompted: true,
  autoUpdate: true,
  instantPlay: true,
  runInBackground: true,
  startWithWindows: true,
  classDataSharing: false,
  autoJava: true,
  preflight: true,
  showReservedSlots: false,
  browseView: "list",
  activeAccountId: null,
  lastInstanceId: null,
  msaClientId: null,
};

export const [settings, setSettings] = createStore<T.Settings>({ ...DEFAULT_SETTINGS });

interface LaunchInfo {
  phase: T.LaunchState["phase"];
  message: string;
}

interface Toast {
  id: number;
  text: string;
  kind: "ok" | "info" | "warn";
  action?: { label: string; run: () => void };
}

interface AppState {
  ready: boolean;
  info: T.AppInfo | null;
  accounts: T.Account[];
  activeAccountId: string | null;
  instances: T.Instance[];
  selectedInstanceId: string | null;
  recentServers: T.RecentServer[];
  serverStatus: Record<string, T.ServerStatus | undefined>;
  launch: Record<string, LaunchInfo | undefined>;
  tasks: Record<string, T.TaskProgress | undefined>;
  logs: Record<string, string[] | undefined>;
  toasts: Toast[];
  exit: T.GameExit | null;
  preflight: T.Preflight | null;
  update: T.UpdateReady | null;
}

export const [app, setApp] = createStore<AppState>({
  ready: false,
  info: null,
  accounts: [],
  activeAccountId: null,
  instances: [],
  selectedInstanceId: null,
  recentServers: [],
  serverStatus: {},
  launch: {},
  tasks: {},
  logs: {},
  toasts: [],
  exit: null,
  preflight: null,
  update: null,
});

export const activeAccount = () => app.accounts.find((a) => a.id === app.activeAccountId) ?? app.accounts[0] ?? null;
export const selectedInstance = () => app.instances.find((i) => i.id === app.selectedInstanceId) ?? app.instances[0] ?? null;
export const instanceById = (id: string) => app.instances.find((i) => i.id === id) ?? null;
export const isBusy = (id: string) => {
  const p = app.launch[id]?.phase;
  return p === "preparing" || p === "running";
};

let toastId = 0;
export function toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"]) {
  const id = ++toastId;
  setApp("toasts", (t) => [...t.slice(-3), { id, text, kind, action }]);
  setTimeout(() => setApp("toasts", (t) => t.filter((x) => x.id !== id)), action ? 8000 : 4200);
}

/** Runs an API call and turns failures into a toast. Returns undefined on error. */
export async function attempt<R>(fn: () => Promise<R>, okText?: string): Promise<R | undefined> {
  try {
    const r = await fn();
    if (okText) toast(okText, "ok");
    return r;
  } catch (e) {
    toast(typeof e === "string" ? e : e instanceof Error ? e.message : String(e), "warn");
    return undefined;
  }
}

export async function refreshInstances() {
  const list = await api.listInstances();
  setApp("instances", reconcile(list, { key: "id" }));
  if (!app.selectedInstanceId || !list.some((i) => i.id === app.selectedInstanceId)) {
    setApp("selectedInstanceId", settings.lastInstanceId && list.some((i) => i.id === settings.lastInstanceId) ? settings.lastInstanceId : list[0]?.id ?? null);
  }
}

export async function refreshAccounts() {
  const { accounts, activeId } = await api.listAccounts();
  setApp({ accounts, activeAccountId: activeId });
}

export async function refreshRecentServers() {
  setApp("recentServers", await api.recentServers());
}

const pinging = new Set<string>();
export async function pingServer(address: string, force = false) {
  if (pinging.has(address) || (!force && app.serverStatus[address])) return;
  pinging.add(address);
  try {
    setApp("serverStatus", address, await api.pingServer(address));
  } finally {
    pinging.delete(address);
  }
}

export async function launchInstance(id: string, server?: string | null) {
  if (app.launch[id]?.phase === "running") {
    await attempt(() => api.stop(id));
    return;
  }
  if (isBusy(id)) return;
  if (!app.accounts.length) {
    toast("Add an account first", "warn");
    return;
  }
  const instant = app.launch[id]?.phase === "ready" || app.launch[id]?.phase === "prewarming";
  setApp("launch", id, { phase: "preparing", message: instant ? "Opening" : "Starting" });
  if (!instant) setApp("logs", id, []);
  try {
    await api.launch(id, server);
  } catch (e) {
    setApp("launch", id, { phase: "error", message: String(e) });
    toast(String(e), "warn");
  }
}

export async function setActiveAccount(id: string) {
  await attempt(() => api.setActiveAccount(id));
  setApp("activeAccountId", id);
}

/** Loads everything from the backend and subscribes to events. Call once. */
export async function init() {
  const root = document.documentElement;
  createEffect(() => {
    root.dataset.theme = settings.theme;
    root.style.setProperty("--accent-h", String(settings.accentHue));
  });

  try {
    const [s, info] = await Promise.all([api.getSettings(), api.appInfo()]);
    setSettings(reconcile({ ...DEFAULT_SETTINGS, ...s }));
    setApp("info", info);
  } catch (e) {
    toast(`Couldn't load settings: ${e}`, "warn");
  }
  await Promise.all([refreshAccounts(), refreshInstances(), refreshRecentServers()]).catch((e) => toast(String(e), "warn"));
  // Restore live game states (the window may have been reopened from the tray).
  for (const s of await api.launchStates().catch(() => [])) setApp("launch", s.instanceId, { phase: s.phase, message: "" });
  setApp("ready", true);

  // Persist settings (debounced) whenever they change after load.
  let t: ReturnType<typeof setTimeout> | undefined;
  createEffect(
    onChange(
      () => JSON.stringify(settings),
      () => {
        clearTimeout(t);
        t = setTimeout(() => api.saveSettings({ ...settings }).catch(() => {}), 300);
      },
      { defer: true },
    ),
  );

  on<T.TaskProgress>("task-progress", (p) => {
    setApp("tasks", p.task, p);
    if (p.total > 0 && p.done >= p.total) setTimeout(() => setApp("tasks", p.task, undefined), 900);
  });
  on<T.LaunchState>("launch-state", (s) => {
    setApp("launch", s.instanceId, { phase: s.phase, message: s.message });
    if (s.phase === "running") setApp("instances", (i) => i.id === s.instanceId, "running", true);
    if (s.phase === "stopped") setApp("instances", (i) => i.id === s.instanceId, "running", false);
  });
  on<{ instanceId: string; lines: string[] }>("game-log", (l) => {
    setApp("logs", l.instanceId, (old) => {
      const next = [...(old ?? []), ...l.lines];
      return next.length > 5000 ? next.slice(-5000) : next;
    });
  });
  on<{ instanceId: string; bootMs: number; background?: boolean }>("game-boot", (b) => {
    setApp("instances", (i) => i.id === b.instanceId, "lastBootMs", b.bootMs);
    if (b.background) return; // Instant Play pre-load: stay quiet
    toast(`${instanceById(b.instanceId)?.name ?? "Game"} reached the title screen in ${(b.bootMs / 1000).toFixed(1)}s`, "ok");
  });
  on<T.GameExit>("game-exit", (e) => {
    if (e.crash) setApp("exit", e);
    refreshInstances();
  });
  on("recent-servers-changed", () => refreshRecentServers());
  on<T.UpdateReady>("update-ready", (u) => {
    setApp("update", u);
    toast(`Halo Client ${u.version} is ready to install`, "info", { label: "Restart now", run: () => attempt(() => api.applyUpdate()) });
  });
  on<{ instanceId: string; message: string; logPath: string }>("launch-warning", (w) => {
    toast(w.message, "warn", { label: "Open launch log", run: () => attempt(() => api.openPath(w.logPath)) });
  });

  if (settings.preflight) {
    api
      .preflight()
      .then((p) => {
        setApp("preflight", p);
        if (!p.accountOk && p.accountMessage && app.accounts.length) toast(p.accountMessage, "warn");
      })
      .catch(() => {});
  }
}
