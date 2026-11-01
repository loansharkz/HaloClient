import { A, useLocation, useNavigate } from "@solidjs/router";
import { For, Show, createMemo, type JSX } from "solid-js";
import { CircleCheck, Compass, House, Info, Layers, LoaderCircle, Minus, Moon, Plus, Settings, Square, Sun, TriangleAlert, Users, X } from "lucide-solid";
import { HaloLogo, InstanceIcon } from "./Icons";
import { activeAccount, app, settings, setSettings } from "../lib/store";
import { ADS_ENABLED, IS_TAURI } from "../lib/config";

export { default as Background } from "./Starfield";

const NAV = [
  { href: "/", label: "Home", icon: House, end: true },
  { href: "/library", label: "Library", icon: Layers },
  { href: "/browse", label: "Browse", icon: Compass },
  { href: "/accounts", label: "Accounts", icon: Users },
];

export function Sidebar() {
  const loc = useLocation();
  const nav = useNavigate();
  const isActive = (href: string, end?: boolean) => (end ? loc.pathname === href : loc.pathname.startsWith(href));
  // Modrinth-app style: recent instances pinned under the main nav.
  const pinned = createMemo(() => app.instances.slice(0, 4));
  return (
    <nav class="glass flex flex-col items-center py-3 gap-1.5 w-[64px] shrink-0" style={{ "border-radius": "18px" }} aria-label="Main">
      <For each={NAV}>
        {(n) => (
          <A href={n.href} class="nav-item" classList={{ active: isActive(n.href, n.end) }} aria-label={n.label}>
            <n.icon size={20} />
            <span class="tip">{n.label}</span>
          </A>
        )}
      </For>
      <div class="w-8 h-px my-1.5" style={{ background: "var(--border)" }} />
      <For each={pinned()}>
        {(i) => (
          <button class="nav-item cursor-pointer" classList={{ active: loc.pathname === `/instance/${i.id}` }} onClick={() => nav(`/instance/${i.id}`)} aria-label={i.name}>
            <div class="relative">
              <InstanceIcon inst={i} size={30} />
              <Show when={app.launch[i.id]?.phase === "running"}>
                <span class="absolute -bottom-1 -right-1 w-2.5 h-2.5 rounded-full border-2" style={{ background: "var(--ok)", "border-color": "var(--bg)" }} />
              </Show>
            </div>
            <span class="tip">{i.name}</span>
          </button>
        )}
      </For>
      <button class="nav-item cursor-pointer" onClick={() => nav("/library?new=1")} aria-label="New instance">
        <Plus size={19} />
        <span class="tip">New instance</span>
      </button>
      <div class="flex-1" />
      <button class="nav-item cursor-pointer" onClick={() => setSettings("theme", settings.theme === "dark" ? "light" : "dark")} aria-label="Toggle theme">
        {settings.theme === "dark" ? <Sun size={19} /> : <Moon size={19} />}
        <span class="tip">{settings.theme === "dark" ? "Light mode" : "Dark mode"}</span>
      </button>
      <A href="/settings" class="nav-item" classList={{ active: isActive("/settings") }} aria-label="Settings">
        <Settings size={20} />
        <span class="tip">Settings</span>
      </A>
      <A href="/accounts" class="mt-1 relative" title={activeAccount()?.username ?? "Add account"}>
        <div
          class="w-9 h-9 rounded-xl flex items-center justify-center font-bold text-white text-sm"
          style={{ background: activeAccount() ? `linear-gradient(135deg, var(--accent), var(--accent-2))` : "var(--glass-strong)" }}
        >
          {activeAccount()?.username[0] ?? "+"}
        </div>
        <Show when={activeAccount()}>
          <span class="absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2" style={{ background: activeAccount()!.kind === "offline" ? "var(--faint)" : "var(--ok)", "border-color": "var(--bg)" }} />
        </Show>
      </A>
    </nav>
  );
}

/** Custom frameless titlebar with a live task indicator. */
export function TitleBar() {
  const win = async (op: "minimize" | "toggleMaximize" | "close") => {
    if (!IS_TAURI) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    getCurrentWindow()[op]();
  };
  const task = createMemo(() => Object.values(app.tasks).find((t) => t && t.done < t.total) ?? null);
  const runningCount = () => Object.values(app.launch).filter((l) => l?.phase === "running").length;
  return (
    <header class="titlebar flex items-center pl-4 pr-1.5 gap-2.5" data-tauri-drag-region>
      <HaloLogo size={22} />
      <span class="font-semibold tracking-tight text-[13px]" data-tauri-drag-region>
        Halo <span class="text-muted font-medium">Client</span>
      </span>
      <span class="chip ml-1" data-tauri-drag-region>v{app.info?.version ?? "0.1.0"}</span>
      <div class="flex-1 h-full" data-tauri-drag-region />
      <Show when={task()}>
        {(t) => (
          <div class="chip chip-accent mr-1 gap-2 max-w-[340px]" title={t().label}>
            <LoaderCircle size={12} class="spin shrink-0" />
            <span class="truncate">{t().label}</span>
            <span class="tabular-nums shrink-0">{t().total > 1 ? `${Math.round((t().done / t().total) * 100)}%` : ""}</span>
          </div>
        )}
      </Show>
      <Show when={runningCount() > 0}>
        <span class="chip chip-ok mr-2">
          <span class="ping-dot pulse" style={{ background: "var(--ok)" }} />
          {runningCount() === 1 ? "Game running" : `${runningCount()} games running`}
        </span>
      </Show>
      <button class="btn btn-ghost btn-icon h-8" onClick={() => win("minimize")} aria-label="Minimize"><Minus size={15} /></button>
      <button class="btn btn-ghost btn-icon h-8" onClick={() => win("toggleMaximize")} aria-label="Maximize"><Square size={12} /></button>
      <button class="btn btn-ghost btn-icon h-8 hover:!bg-red-500/80 hover:!text-white" onClick={() => win("close")} aria-label="Close"><X size={16} /></button>
    </header>
  );
}

export function Toasts() {
  return (
    <div class="fixed bottom-5 right-5 flex flex-col gap-2 z-[200] max-w-[420px]" role="status" aria-live="polite">
      <For each={app.toasts}>
        {(t) => (
          <div class="toast glass px-4 py-2.5 flex items-center gap-2.5 text-[13px] font-medium" style={{ background: "var(--glass-strong)" }}>
            {t.kind === "ok" ? <CircleCheck size={16} color="var(--ok)" class="shrink-0" /> : t.kind === "warn" ? <TriangleAlert size={16} color="var(--warn)" class="shrink-0" /> : <Info size={16} color="var(--accent)" class="shrink-0" />}
            <span class="min-w-0 break-words">{t.text}</span>
            <Show when={t.action}>
              <button class="btn h-7 px-2.5 ml-1 shrink-0" onClick={() => t.action!.run()}>{t.action!.label}</button>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
}

/** Reserved monetization slot (Phase 7). Renders nothing unless ADS_ENABLED, or the dev "show reserved slots" toggle is on. */
export function AdSlot(props: { name: string; class?: string; style?: JSX.CSSProperties }) {
  return (
    <Show when={ADS_ENABLED || settings.showReservedSlots}>
      <div class={`reserved-slot ${props.class ?? ""}`} style={props.style}>
        Reserved slot: {props.name}
        <br />
        (ADS_ENABLED=false)
      </div>
    </Show>
  );
}

export function Toggle(props: { on: boolean; onChange: (v: boolean) => void; label?: string; disabled?: boolean }) {
  return (
    <button
      role="switch"
      aria-checked={props.on}
      aria-label={props.label}
      disabled={props.disabled}
      class="toggle disabled:opacity-40"
      classList={{ on: props.on }}
      onClick={(e) => {
        e.stopPropagation();
        props.onChange(!props.on);
      }}
    />
  );
}

export function Modal(props: { open: boolean; onClose: () => void; width?: number; children: JSX.Element; label?: string }) {
  return (
    <Show when={props.open}>
      <div class="modal-backdrop" onClick={props.onClose} onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
        <div
          role="dialog"
          aria-modal="true"
          aria-label={props.label}
          class="modal glass scroll"
          style={{ width: `min(${props.width ?? 520}px, 94vw)`, background: "var(--glass-strong)" }}
          onClick={(e) => e.stopPropagation()}
        >
          {props.children}
        </div>
      </div>
    </Show>
  );
}

export function PageHeader(props: { title: string; sub?: JSX.Element; children?: JSX.Element }) {
  return (
    <div class="flex items-end gap-4 mb-5 flex-wrap">
      <div class="min-w-0">
        <h1 class="text-[26px] font-bold tracking-tight leading-tight">{props.title}</h1>
        <Show when={props.sub}><p class="text-muted mt-0.5">{props.sub}</p></Show>
      </div>
      <div class="flex-1" />
      <div class="flex items-center gap-2 flex-wrap">{props.children}</div>
    </div>
  );
}

export function Empty(props: { icon?: JSX.Element; title: string; children?: JSX.Element }) {
  return (
    <div class="glass p-10 flex flex-col items-center text-center gap-2">
      <Show when={props.icon}><div class="text-accent mb-1">{props.icon}</div></Show>
      <div class="font-semibold text-[15px]">{props.title}</div>
      <div class="text-muted text-[13px] max-w-[420px]">{props.children}</div>
    </div>
  );
}
