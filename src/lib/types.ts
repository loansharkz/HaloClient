// Types shared with the Rust backend (serde camelCase). Keep in sync with src-tauri/src/*.rs.

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";
export type ContentKind = "mod" | "resourcepack" | "shader" | "datapack" | "modpack";

export interface Settings {
  theme: "dark" | "light";
  accentHue: number;
  animatedBackground: boolean;
  defaultRamMb: number;
  downloadThreads: number;
  closeOnLaunch: boolean;
  discordRpc: boolean;
  crashReports: boolean;
  crashReportsPrompted: boolean;
  autoJava: boolean;
  preflight: boolean;
  showReservedSlots: boolean;
  browseView: "grid" | "list";
  activeAccountId: string | null;
  lastInstanceId: string | null;
  msaClientId: string | null;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  platform: string;
  msaConfigured: boolean;
  curseforge: boolean;
  discord: boolean;
  sentry: boolean;
  hud: { version: string; minecraft: string[]; file: string } | null;
}

export interface Cape {
  id: string;
  alias: string;
  url: string;
  active: boolean;
}

export interface Account {
  id: string;
  kind: "microsoft" | "offline";
  username: string;
  uuid: string;
  skinUrl?: string | null;
  skinVariant?: string | null;
  capes: Cape[];
  expiresAt: number;
}

export interface Textures {
  skin: string | null;
  cape: string | null;
  slim: boolean;
}

export interface BootSample {
  ms: number;
  at: number;
  cds: boolean;
  perf: boolean;
}

export interface Instance {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion: string | null;
  versionId: string | null;
  icon: string;
  iconServer: string | null;
  iconPath: string | null;
  hue: number;
  group: string | null;
  created: number;
  lastPlayed: number | null;
  playtimeSecs: number;
  ramMb: number | null;
  javaPath: string | null;
  jvmArgs: string;
  width: number | null;
  height: number | null;
  preLaunch: string | null;
  postExit: string | null;
  accountId: string | null;
  perfProfile: boolean;
  cds: boolean;
  tunedFlags: boolean;
  lastBootMs: number | null;
  bootHistory: BootSample[];
  // view-only
  running: boolean;
  vanillaBootMs: number | null;
  hudSupported: boolean;
}

export interface ContentMeta {
  source: string;
  projectId?: string | null;
  versionId?: string | null;
  slug?: string | null;
  title?: string | null;
  versionNumber?: string | null;
  iconUrl?: string | null;
  perf: boolean;
  hud: boolean;
  dependency: boolean;
}

export interface ContentItem {
  key: string;
  kind: "mod" | "resourcepack" | "shader";
  fileName: string;
  name: string;
  version: string | null;
  enabled: boolean;
  size: number;
  meta: ContentMeta | null;
}

export interface UpdateInfo {
  key: string;
  projectId: string;
  current: string;
  latest: string;
  latestVersionId: string;
}

export interface SearchHit {
  source: "modrinth" | "curseforge";
  id: string;
  slug: string;
  title: string;
  author: string;
  description: string;
  iconUrl: string | null;
  bannerUrl: string | null;
  downloads: number;
  follows: number;
  updated: string;
  categories: string[];
  clientSide: string;
  serverSide: string;
  projectType: string;
}

export interface SearchQuery {
  source: "modrinth" | "curseforge";
  query: string;
  projectType: ContentKind;
  gameVersion: string | null;
  loader: string | null;
  categories: string[];
  environment: "client" | "server" | null;
  sort: "relevance" | "downloads" | "follows" | "newest" | "updated";
  offset: number;
  limit: number;
}

export interface SearchResult {
  hits: SearchHit[];
  total: number;
}

export interface InstallResult {
  installed: string[];
  warnings: string[];
  needsShaderLoader: string | null;
  manualUrl: string | null;
}

export interface RecentServer {
  address: string;
  name: string;
  instanceId: string;
  lastPlayed: number;
  joins: number;
}

export interface ServerStatus {
  online: boolean;
  motd: string[];
  playersOnline: number;
  playersMax: number;
  favicon: string | null;
  version: string | null;
  pingMs: number | null;
}

export interface ServerEntry {
  name: string;
  ip: string;
  icon?: string | null;
}

export interface GameVersion {
  id: string;
  kind: string;
  releaseTime: string;
}

export interface LoaderVersion {
  version: string;
  recommended: boolean;
}

export interface Screenshot {
  path: string;
  name: string;
  taken: number;
}

export interface LaunchState {
  instanceId: string;
  phase: "preparing" | "running" | "stopped" | "error";
  message: string;
}

export interface CrashFix {
  kind: "install_dependency" | "disable_mod" | "increase_ram" | "auto_java" | "update_drivers" | "remove_duplicate" | "reset_jvm_args";
  arg: string | null;
  label: string;
}

export interface CrashReport {
  title: string;
  detail: string;
  fix: CrashFix | null;
  crashFile: string | null;
}

export interface GameExit {
  instanceId: string;
  code: number | null;
  playedSecs: number;
  bootMs: number | null;
  crash: CrashReport | null;
  killed: boolean;
}

export interface TaskProgress {
  task: string;
  label: string;
  done: number;
  total: number;
}

export interface Preflight {
  accountOk: boolean;
  accountMessage: string | null;
  instanceId: string | null;
  instanceReady: boolean;
}

export interface UpdateCheck {
  current: string;
  latest: string | null;
  available: boolean;
  url: string | null;
}

export interface CreateSpec {
  name: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion: string | null;
  perfMods: string[];
  haloHud: boolean;
  icon?: string;
  hue?: number;
}

export interface AuthStart {
  flowId: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
}
