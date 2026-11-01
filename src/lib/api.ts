// Typed bridge to the Rust backend. Inside Tauri every call is a real IPC
// command; in a plain browser (the shareable preview, `pnpm dev`) the same
// calls are answered by an in-memory mock so the UI stays explorable.

import { IS_TAURI } from "./config";
import type * as T from "./types";

type Handler = (payload: any) => void;

async function call<R>(cmd: string, args?: Record<string, unknown>): Promise<R> {
  if (IS_TAURI) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<R>(cmd, args);
  }
  const { mockCall } = await import("./mockApi");
  return mockCall(cmd, args ?? {}) as Promise<R>;
}

/** Subscribe to a backend event. Returns an unsubscribe function. */
export async function on<P>(event: string, cb: (payload: P) => void): Promise<() => void> {
  if (IS_TAURI) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<P>(event, (e) => cb(e.payload));
  }
  const { mockOn } = await import("./mockApi");
  return mockOn(event, cb as Handler);
}

/** URL for showing a local file (screenshot, icon) in the webview. */
export async function fileSrc(path: string): Promise<string> {
  if (!IS_TAURI) return path;
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return convertFileSrc(path);
}

export async function pickFile(opts: { title: string; extensions?: string[]; directory?: boolean }): Promise<string | null> {
  if (!IS_TAURI) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({
    title: opts.title,
    directory: !!opts.directory,
    multiple: false,
    filters: opts.extensions ? [{ name: opts.title, extensions: opts.extensions }] : undefined,
  });
  return typeof r === "string" ? r : null;
}

export async function pickSavePath(title: string, defaultPath: string, ext: string): Promise<string | null> {
  if (!IS_TAURI) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ title, defaultPath, filters: [{ name: ext, extensions: [ext] }] });
}

export const api = {
  appInfo: () => call<T.AppInfo>("app_info"),
  getSettings: () => call<T.Settings>("get_settings"),
  saveSettings: (settings: T.Settings) => call<void>("save_settings", { settings }),
  setDataDir: (path: string | null) => call<void>("set_data_dir", { path }),
  openPath: (path: string) => call<void>("open_path", { path }),
  openUrl: (url: string) => call<void>("open_url", { url }),
  checkUpdate: () => call<T.UpdateCheck>("check_update"),
  preflight: () => call<T.Preflight>("preflight"),

  gameVersions: () => call<T.GameVersion[]>("game_versions"),
  loaderVersions: (loader: string, mc: string) => call<T.LoaderVersion[]>("loader_versions", { loader, mc }),

  listAccounts: () => call<{ accounts: T.Account[]; activeId: string | null }>("list_accounts"),
  setActiveAccount: (id: string) => call<void>("set_active_account", { id }),
  authStart: () => call<T.AuthStart>("auth_start"),
  authPoll: (flowId: string) => call<T.Account>("auth_poll", { flowId }),
  authCancel: (flowId: string) => call<void>("auth_cancel", { flowId }),
  addOffline: (name: string) => call<T.Account>("add_offline", { name }),
  removeAccount: (id: string) => call<void>("remove_account", { id }),
  accountTextures: (id: string) => call<T.Textures>("account_textures", { id }),
  uploadSkin: (id: string, path: string, slim: boolean) => call<T.Account>("upload_skin", { id, path, slim }),
  setCape: (id: string, capeId: string | null) => call<T.Account>("set_cape", { id, capeId }),

  listInstances: () => call<T.Instance[]>("list_instances"),
  createInstance: (spec: T.CreateSpec) => call<{ instance: T.Instance; warnings: string[] }>("create_instance", { spec }),
  updateInstance: (instance: T.Instance) => call<T.Instance>("update_instance", { instance }),
  deleteInstance: (id: string) => call<void>("delete_instance", { id }),
  duplicateInstance: (id: string) => call<T.Instance>("duplicate_instance", { id }),
  setInstanceIcon: (id: string, path: string) => call<void>("set_instance_icon", { id, path }),
  listContent: (id: string) => call<T.ContentItem[]>("list_content", { id }),
  toggleContent: (id: string, key: string, enabled: boolean) => call<void>("toggle_content", { id, key, enabled }),
  removeContent: (id: string, key: string) => call<void>("remove_content", { id, key }),
  checkContentUpdates: (id: string) => call<T.UpdateInfo[]>("check_content_updates", { id }),
  applyContentUpdates: (id: string, updates: [string, string][]) => call<T.InstallResult>("apply_content_updates", { id, updates }),
  installProject: (req: { instanceId: string; source: string; projectId: string; versionId?: string | null; kind: string; world?: string | null }) =>
    call<T.InstallResult>("install_project", { req }),
  installModpack: (projectId: string, versionId?: string | null) => call<T.Instance>("install_modpack", { projectId, versionId: versionId ?? null }),
  installPerf: (id: string, mods?: string[]) => call<T.InstallResult>("install_perf", { id, mods: mods ?? null }),
  installHud: (id: string) => call<void>("install_hud", { id }),
  screenshots: (id: string) => call<T.Screenshot[]>("screenshots", { id }),
  worlds: (id: string) => call<string[]>("worlds", { id }),
  instanceDir: (id: string) => call<string>("instance_dir", { id }),
  exportMrpack: (id: string, path: string) => call<void>("export_mrpack", { id, path }),
  importMrpack: (path: string) => call<T.Instance>("import_mrpack", { path }),
  importPrism: (path: string) => call<T.Instance>("import_prism", { path }),
  importVanilla: (path: string, mcVersion: string, name: string) => call<T.Instance>("import_vanilla", { path, mcVersion, name }),
  defaultMinecraftDir: () => call<string | null>("default_minecraft_dir"),
  repairInstance: (id: string) => call<void>("repair_instance", { id }),

  launch: (id: string, server?: string | null) => call<void>("launch_instance", { id, server: server ?? null }),
  stop: (id: string) => call<void>("stop_instance", { id }),
  running: () => call<string[]>("running_instances"),
  gameLog: (id: string) => call<string[]>("game_log", { id }),

  recentServers: () => call<T.RecentServer[]>("recent_servers"),
  forgetRecentServer: (address: string) => call<void>("forget_recent_server", { address }),
  pingServer: (address: string) => call<T.ServerStatus>("ping_server", { address }),
  serverList: (id: string) => call<T.ServerEntry[]>("server_list", { id }),
  saveServerList: (id: string, serversList: T.ServerEntry[]) => call<void>("save_server_list", { id, serversList }),

  search: (query: T.SearchQuery) => call<T.SearchResult>("search_content", { query }),
  project: (source: string, id: string) => call<any>("get_project", { source, id }),
  projectVersions: (id: string, loaders: string[], gameVersions: string[]) => call<any[]>("get_project_versions", { id, loaders, gameVersions }),
};
