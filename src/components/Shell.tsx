import { A, useLocation } from "@solidjs/router";
import { For, Show, type JSX } from "solid-js";
import { CircleCheck, Compass, House, Info, Layers, Minus, Moon, Settings, Square, Sun, TriangleAlert, Users, X } from "lucide-solid";
import { HaloLogo } from "./Icons";
import { app, activeAccount, setSettings, settings } from "../lib/store";
import { ADS_ENABLED, IS_TAURI } from "../lib/config";

export { default as Background } from "./Starfield";

const NAV = [
  { href: "/", label: "Home", icon: House, end: true },
  { href: "/instances", label: "Instances", icon: Layers },
  { href: "/browse", label: "Browse", icon: Compass },
  { href: "/accounts", label: "Accounts", icon: Users },
];

export function Sidebar() {
  const loc = useLocation();
  const isActive = (href: string, end?: boolean) => (end ? loc.pathname === href : loc.pathname.startsWith(href));
  return (
    <nav class="glass flex flex-col items-center py-3 gap-1.5 w-[64px] shrink-0" style={{ "border-radius": "18px" }}>
      <For each={NAV}>
        {(n) => (
          <A href={n.href} class="nav-item" classList={{ active: isActive(n.href, n.end) }}>
            <n.icon size={20} />
            <span class="tip">{n.label}</span>
          </A>
        )}
      </For>
      <div class="flex-1" />
      <button
        class="nav-item cursor-pointer"
        onClick={() => setSettings("theme", settings.theme === "dark" ? "light" : "dark")}
        title="Toggle theme"
      >
        {settings.theme === "dark" ? <Sun size={19} /> : <Moon size={19} />}
        <span class="tip">{settings.theme === "dark" ? "Light mode" : "Dark mode"}</span>
      </button>
      <A href="/settings" class="nav-item" classList={{ active: isActive("/settings") }}>
        <Settings size={20} />
        <span class="tip">Settings</span>
      </A>
      <A href="/accounts" class="mt-1 relative" title={activeAccount().username}>
        <div
          class="w-9 h-9 rounded-xl flex items-center justify-center font-bold text-white text-sm"
          style={{ background: `linear-gradient(135deg, hsl(${activeAccount().skinSeed} 70% 55%), hsl(${activeAccount().skinSeed + 50} 70% 45%))` }}
        >
          {activeAccount().username[0]}
        </div>
        <span class="absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2" style={{ background: activeAccount().kind === "offline" ? "var(--faint)" : "var(--ok)", "border-color": "var(--bg)" }} />
      </A>
    </nav>
  );
}

/** Custom frameless titlebar. In Tauri the drag region + window controls go to the backend. */
export function TitleBar() {
  const win = async (op: "minimize" | "toggleMaximize" | "close") => {
    if (!IS_TAURI) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    getCurrentWindow()[op]();
  };
  return (
    <header class="titlebar flex items-center pl-4 pr-1.5 gap-2.5" data-tauri-drag-region>
      <HaloLogo size={22} />
      <span class="font-semibold tracking-tight text-[13px]" data-tauri-drag-region>
        Halo <span class="text-muted font-medium">Client</span>
      </span>
      <span class="chip ml-1" data-tauri-drag-region>v0.1 preview</span>
      <div class="flex-1 h-full" data-tauri-drag-region />
      <Show when={app.launch.phase !== "idle"}>
        <span class="chip chip-accent mr-2">
          <span class="ping-dot pulse" style={{ background: "var(--accent)" }} />
          {app.launch.phase === "running" ? "Game running" : app.launch.phase === "preflight" ? "Pre-flight" : "Launching…"}
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
    <div class="fixed bottom-5 right-5 flex flex-col gap-2 z-[200]">
      <For each={app.toasts}>
        {(t) => (
          <div class="toast glass px-4 py-2.5 flex items-center gap-2.5 text-[13px] font-medium" style={{ "background": "var(--glass-strong)" }}>
            {t.kind === "ok" ? <CircleCheck size={16} color="var(--ok)" /> : t.kind === "warn" ? <TriangleAlert size={16} color="var(--warn)" /> : <Info size={16} color="var(--accent)" />}
            {t.text}
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

export function Toggle(props: { on: boolean; onChange: (v: boolean) => void; label?: string }) {
  return (
    <button
      role="switch"
      aria-checked={props.on}
      aria-label={props.label}
      class="toggle"
      classList={{ on: props.on }}
      onClick={(e) => {
        e.stopPropagation();
        props.onChange(!props.on);
      }}
    />
  );
}

export function Modal(props: { open: boolean; onClose: () => void; width?: number; children: JSX.Element }) {
  return (
    <Show when={props.open}>
      <div class="modal-backdrop" onClick={props.onClose}>
        <div class="modal glass scroll" style={{ width: `${props.width ?? 520}px`, "background": "var(--glass-strong)" }} onClick={(e) => e.stopPropagation()}>
          {props.children}
        </div>
      </div>
    </Show>
  );
}

export function PageHeader(props: { title: string; sub?: string; children?: JSX.Element }) {
  return (
    <div class="flex items-end gap-4 mb-5">
      <div>
        <h1 class="text-[26px] font-bold tracking-tight leading-tight">{props.title}</h1>
        <Show when={props.sub}><p class="text-muted mt-0.5">{props.sub}</p></Show>
      </div>
      <div class="flex-1" />
      {props.children}
    </div>
  );
}
