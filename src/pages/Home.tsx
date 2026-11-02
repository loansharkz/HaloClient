import { A, useNavigate } from "@solidjs/router";
import { For, Show, createEffect, createResource, createSignal, lazy, onCleanup } from "solid-js";
import { ChevronDown, ChevronRight, Clock, Gauge, LoaderCircle, Plus, Server as ServerIcon, ShieldCheck, Timer, UserPlus, X } from "lucide-solid";
import { InstanceIcon } from "../components/Icons";
import { AdSlot } from "../components/Shell";
import { PlayButton } from "../components/Overlays";
import Motd from "../components/Motd";
import { api } from "../lib/api";
import { LOADER_LABEL, ago, fmt, hours, seconds } from "../lib/format";
import { activeAccount, app, attempt, instanceById, launchInstance, pingServer, refreshRecentServers, selectedInstance, setApp, setSettings } from "../lib/store";

const SkinViewer = lazy(() => import("../components/SkinViewer"));

/** Lunar-style launch cluster: big Play + instance picker as one control. */
function LaunchCluster() {
  const [open, setOpen] = createSignal(false);
  const inst = selectedInstance;
  createEffect(() => {
    if (!open()) return;
    const h = () => setOpen(false);
    const t = setTimeout(() => window.addEventListener("click", h, { once: true }));
    onCleanup(() => {
      clearTimeout(t);
      window.removeEventListener("click", h);
    });
  });
  return (
    <div class="relative flex items-stretch">
      <PlayButton instanceId={inst()!.id} />
      <button
        class="glass-flat glass-hover ml-2 px-3.5 flex items-center gap-3 rounded-2xl cursor-pointer min-w-[220px] text-left"
        style={{ height: "56px" }}
        onClick={(e) => {
          e.stopPropagation();
          setOpen(!open());
        }}
        aria-haspopup="listbox"
        aria-expanded={open()}
      >
        <InstanceIcon inst={inst()!} size={32} />
        <div class="min-w-0 flex-1">
          <div class="font-semibold text-[13px] truncate">{inst()!.name}</div>
          <div class="text-faint text-[11.5px]">{inst()!.mcVersion} · {LOADER_LABEL[inst()!.loader]}</div>
        </div>
        <ChevronDown size={16} class="text-muted" />
      </button>
      <Show when={open()}>
        <div class="glass absolute left-[calc(100%-220px)] bottom-[64px] w-[280px] p-1.5 z-30 max-h-[300px] scroll" role="listbox" style={{ background: "var(--glass-strong)" }}>
          <For each={app.instances}>
            {(i) => (
              <button
                role="option"
                aria-selected={i.id === inst()!.id}
                class="w-full flex items-center gap-2.5 p-2 rounded-xl hover:bg-[var(--glass)] text-left cursor-pointer"
                onClick={() => {
                  setApp("selectedInstanceId", i.id);
                  setSettings("lastInstanceId", i.id);
                  setOpen(false);
                }}
              >
                <InstanceIcon inst={i} size={28} />
                <div class="min-w-0">
                  <div class="text-[13px] font-semibold truncate">{i.name}</div>
                  <div class="text-faint text-[11px]">{i.mcVersion} · {LOADER_LABEL[i.loader]}</div>
                </div>
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

function PreflightStatus() {
  const inst = selectedInstance;
  const launch = () => app.launch[inst()?.id ?? ""];
  const p = () => app.preflight;
  const ok = () => !!p()?.accountOk;
  return (
    <div class="flex items-center gap-2 text-[12px] font-semibold uppercase tracking-wider text-muted">
      <Show
        when={launch()?.phase === "preparing"}
        fallback={
          <Show when={p()} fallback={<><LoaderCircle size={12} class="spin" /> Pre-flight check</>}>
            <span class="ping-dot" style={{ background: ok() ? "var(--ok)" : "var(--warn)", "box-shadow": `0 0 8px ${ok() ? "var(--ok)" : "var(--warn)"}` }} />
            {ok() ? (p()!.instanceReady && p()!.instanceId === inst()?.id ? "Ready to play · pre-flight passed" : "Account ready") : p()!.accountMessage ?? "Sign in to play"}
          </Show>
        }
      >
        <LoaderCircle size={12} class="spin text-accent" /> {launch()!.message}
      </Show>
      <Show when={launch()?.phase === "ready"}>
        <span class="text-accent">· Instant Play ready</span>
      </Show>
      <Show when={launch()?.phase === "prewarming"}>
        <span class="text-faint">· pre-loading in the background</span>
      </Show>
    </div>
  );
}

function Hero() {
  const nav = useNavigate();
  const inst = selectedInstance;
  const [textures] = createResource(() => activeAccount()?.id, (id) => api.accountTextures(id).catch(() => null));
  const speedup = () => {
    const i = inst();
    return i?.lastBootMs && i.vanillaBootMs && i.vanillaBootMs > i.lastBootMs ? (i.vanillaBootMs / i.lastBootMs).toFixed(1) : null;
  };
  return (
    <section class="glass relative overflow-hidden flex min-h-[320px]">
      <div class="absolute inset-0 opacity-70 pointer-events-none" style={{ background: `radial-gradient(120% 100% at 0% 100%, oklch(0.55 0.2 ${inst()?.hue ?? 275} / .4), transparent 60%)` }} />
      <div class="relative flex-1 p-7 flex flex-col min-w-0">
        <Show
          when={inst()}
          fallback={
            <div class="my-auto">
              <h2 class="text-[34px] font-bold tracking-tight leading-tight">Welcome to Halo Client</h2>
              <p class="text-muted mt-2 max-w-[440px]">Create an instance to start. New Fabric instances come with the Performance profile so they boot fast and run smooth.</p>
              <div class="flex gap-2 mt-5">
                <button class="btn btn-primary play-btn" onClick={() => nav("/library?new=1")}><Plus size={20} /> New instance</button>
                <Show when={!activeAccount()}>
                  <A href="/accounts" class="btn h-[56px] px-5 rounded-2xl"><UserPlus size={18} /> Add account</A>
                </Show>
              </div>
            </div>
          }
        >
          <PreflightStatus />
          <div class="mt-4 text-muted text-[13px]">Welcome back{activeAccount() ? `, ${activeAccount()!.username}` : ""}</div>
          <h2 class="text-[40px] font-bold tracking-tight leading-none mt-1 truncate">{inst()!.name}</h2>
          <div class="flex gap-1.5 mt-3 flex-wrap">
            <span class="chip">{inst()!.mcVersion}</span>
            <span class="chip">{LOADER_LABEL[inst()!.loader]} {inst()!.loaderVersion ?? ""}</span>
            <Show when={inst()!.perfProfile}><span class="chip chip-accent"><Gauge size={12} /> Performance</span></Show>
          </div>
          <div class="flex gap-7 mt-5 text-[13px]">
            <div><div class="stat-label">Playtime</div><div class="font-semibold mt-0.5">{hours(inst()!.playtimeSecs)}</div></div>
            <div><div class="stat-label">Last played</div><div class="font-semibold mt-0.5">{ago(inst()!.lastPlayed)}</div></div>
            <div>
              <div class="stat-label">Boot time</div>
              <div class="font-semibold mt-0.5 flex items-center gap-1.5">
                <Timer size={13} class="text-accent" /> {seconds(inst()!.lastBootMs)}
                <Show when={speedup()}><span class="chip chip-ok">{speedup()}× vanilla</span></Show>
              </div>
            </div>
          </div>
          <div class="flex-1 min-h-6" />
          <LaunchCluster />
        </Show>
      </div>

      <div class="relative w-[260px] shrink-0 flex flex-col items-center justify-end pb-4">
        <div class="absolute bottom-6 w-40 h-8 rounded-[50%] blur-xl" style={{ background: "var(--accent-soft)" }} />
        <SkinViewer seed={(activeAccount()?.username.charCodeAt(0) ?? 39) * 7} skin={textures()?.skin} cape={textures()?.cape} slim={textures()?.slim} width={240} height={280} />
        <A href="/accounts" class="glass-flat glass-hover px-3 py-1.5 flex items-center gap-2 text-[12.5px] font-semibold -mt-2">
          <Show when={activeAccount()} fallback={<><UserPlus size={14} /> Add account</>}>
            <ShieldCheck size={14} color={activeAccount()!.kind === "offline" ? "var(--faint)" : "var(--ok)"} />
            {activeAccount()!.username}
            <span class="text-faint font-medium">{activeAccount()!.kind === "offline" ? "Offline" : "Microsoft"}</span>
          </Show>
        </A>
      </div>
    </section>
  );
}

function QuickJoin() {
  createEffect(() => app.recentServers.forEach((s) => pingServer(s.address)));
  const t = setInterval(() => app.recentServers.forEach((s) => pingServer(s.address, true)), 60_000);
  onCleanup(() => clearInterval(t));
  return (
    <section class="glass p-5">
      <div class="flex items-center gap-2 mb-3">
        <ServerIcon size={16} class="text-accent" />
        <h3 class="font-semibold">Quick join</h3>
        <div class="flex-1" />
        <span class="text-faint text-[12px]">Servers you played recently</span>
      </div>
      <Show when={app.recentServers.length > 0} fallback={<div class="glass-flat p-6 text-center text-muted text-[12.5px]">Servers you join show up here for one-click rejoin.</div>}>
        <div class="flex flex-col gap-1.5">
          <For each={app.recentServers.slice(0, 5)}>
            {(s) => {
              const live = () => app.serverStatus[s.address];
              const inst = () => instanceById(s.instanceId);
              return (
                <div class="glass-flat glass-hover flex items-center gap-3 px-3 py-2.5 group">
                  <Show
                    when={live()?.favicon}
                    fallback={<div class="w-12 h-12 rounded-lg shrink-0 flex items-center justify-center font-bold text-[14px]" style={{ background: "var(--glass-strong)" }}>{s.name.slice(0, 2).toUpperCase()}</div>}
                  >
                    <img src={live()!.favicon!} alt="" width={48} height={48} class="rounded-lg shrink-0" style={{ "image-rendering": "pixelated" }} />
                  </Show>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-semibold text-[13px] truncate">{s.name}</span>
                      <Show when={live() && !live()!.online}><span class="chip" style={{ height: "18px", "font-size": "10px" }}>No response</span></Show>
                    </div>
                    <Show when={live()?.motd.length} fallback={<div class="text-faint text-[11.5px] mono truncate">{s.address}</div>}>
                      <Motd lines={live()!.motd} class="mono text-[11px] leading-[1.35] mt-0.5" />
                    </Show>
                    <div class="text-faint text-[11px] mt-0.5">{ago(s.lastPlayed)} · {inst()?.name ?? "deleted instance"}</div>
                  </div>
                  <Show when={live()?.online}>
                    <div class="text-right text-[11.5px] shrink-0 tabular-nums">
                      <div class="font-semibold">{fmt(live()!.playersOnline)}</div>
                      <div class="text-faint">{live()!.pingMs != null ? `${live()!.pingMs} ms` : "online"}</div>
                    </div>
                  </Show>
                  <div class="flex gap-1 shrink-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
                    <button class="btn btn-ghost btn-icon h-8" title="Remove from Quick join" onClick={async () => { await attempt(() => api.forgetRecentServer(s.address)); refreshRecentServers(); }}>
                      <X size={14} />
                    </button>
                    <button class="btn btn-primary h-8 px-3" disabled={!inst()} onClick={() => launchInstance(s.instanceId, s.address)}>Join</button>
                  </div>
                </div>
              );
            }}
          </For>
        </div>
      </Show>
    </section>
  );
}

const NEWS = [
  { tag: "Halo", title: "Welcome to Halo Client 0.1", body: "Multi-account switching, a Modrinth browser built in, and the Performance profile on every new Fabric instance.", hue: 275 },
  { tag: "Guide", title: "Why Halo boots faster", body: "ModernFix, a tuned Java runtime and class-data sharing cut the time from Play to title screen. Every mod in the stack is client-side and safe on public servers.", hue: 310 },
  { tag: "Halo HUD", title: "Companion HUD mod", body: "FPS, CPS, ping, keystrokes, armor and potion timers, plus zoom. Turn it on per instance.", hue: 240 },
];

function News() {
  return (
    <section class="glass p-5 flex flex-col">
      <h3 class="font-semibold mb-3">News</h3>
      <div class="flex flex-col gap-2.5">
        <For each={NEWS}>
          {(n, i) => (
            <>
              <article class="glass-flat flex gap-3 p-3">
                <div class="w-14 h-14 rounded-xl shrink-0" style={{ background: `linear-gradient(135deg, oklch(0.6 0.18 ${n.hue}), oklch(0.4 0.18 ${n.hue + 60}))` }} />
                <div class="min-w-0">
                  <span class="chip chip-accent" style={{ height: "18px" }}>{n.tag}</span>
                  <div class="font-semibold text-[13.5px] mt-1">{n.title}</div>
                  <p class="text-muted text-[12px] mt-0.5 line-clamp-2">{n.body}</p>
                </div>
              </article>
              <Show when={i() === 0}><AdSlot name="home news card" class="h-[90px]" /></Show>
            </>
          )}
        </For>
      </div>
    </section>
  );
}

export default function Home() {
  const nav = useNavigate();
  return (
    <div class="page-enter flex flex-col gap-4 pb-2">
      <Hero />
      <Show when={app.instances.length > 0}>
        <section>
          <div class="flex items-center mb-2.5 px-1">
            <h3 class="font-semibold flex items-center gap-2"><Clock size={15} class="text-accent" /> Jump back in</h3>
            <div class="flex-1" />
            <A href="/library" class="text-muted text-[12px] flex items-center gap-1 hover:text-[var(--text)]">Library <ChevronRight size={13} /></A>
          </div>
          <div class="grid grid-cols-4 gap-3">
            <For each={app.instances.slice(0, 4)}>
              {(i) => (
                <div
                  class="glass glass-hover lift p-3.5 flex flex-col gap-3 cursor-pointer"
                  classList={{ "!border-[var(--accent)]": app.selectedInstanceId === i.id }}
                  onClick={() => setApp("selectedInstanceId", i.id)}
                  onDblClick={() => nav(`/instance/${i.id}`)}
                >
                  <div class="flex items-center gap-3">
                    <InstanceIcon inst={i} size={40} />
                    <div class="min-w-0">
                      <div class="font-semibold truncate">{i.name}</div>
                      <div class="text-faint text-[11.5px]">{i.mcVersion} · {LOADER_LABEL[i.loader]}</div>
                    </div>
                  </div>
                  <div class="flex items-center">
                    <span class="text-faint text-[11.5px]">{ago(i.lastPlayed)}</span>
                    <div class="flex-1" />
                    <PlayButton instanceId={i.id} size="md" />
                  </div>
                </div>
              )}
            </For>
          </div>
        </section>
      </Show>
      <div class="grid grid-cols-[1.25fr_1fr] gap-4">
        <QuickJoin />
        <News />
      </div>
    </div>
  );
}
