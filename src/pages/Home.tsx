import { A, useNavigate } from "@solidjs/router";
import { For, Show, createSignal, lazy, onCleanup } from "solid-js";
import { ChevronRight, Clock, Gauge, LoaderCircle, Play, Power, Server as ServerIcon, ShieldCheck, Timer } from "lucide-solid";
import { IconTile } from "../components/Icons";
import { AdSlot } from "../components/Shell";
import { LOADER_LABEL, NEWS, SERVERS, fmt } from "../lib/mock";
import { activeAccount, app, launchInstance, selectedInstance, setApp, toast } from "../lib/store";

const SkinViewer = lazy(() => import("../components/SkinViewer"));

export function PlayButton(props: { instanceId: string; size?: "lg" | "md" }) {
  const phase = () => (app.launch.instanceId === props.instanceId ? app.launch.phase : "idle");
  const busyElsewhere = () => app.launch.phase !== "idle" && app.launch.instanceId !== props.instanceId;
  return (
    <button
      class="btn btn-primary"
      classList={{ "play-btn": props.size !== "md", "h-8 px-3": props.size === "md", running: phase() === "running" }}
      disabled={busyElsewhere()}
      onClick={(e) => {
        e.stopPropagation();
        e.preventDefault();
        launchInstance(props.instanceId);
      }}
    >
      <Show when={phase() === "idle"}><Play size={props.size === "md" ? 15 : 20} fill="currentColor" /> Play</Show>
      <Show when={phase() === "preflight"}><LoaderCircle size={18} class="spin" /> Pre-flight</Show>
      <Show when={phase() === "launching"}><LoaderCircle size={18} class="spin" /> {Math.round(app.launch.progress)}%</Show>
      <Show when={phase() === "running"}><Power size={18} /> Running</Show>
    </button>
  );
}

function Hero() {
  const inst = selectedInstance;
  const speedup = () => {
    const i = inst();
    return i.lastBootMs && i.vanillaBootMs ? (i.vanillaBootMs / i.lastBootMs).toFixed(1) : null;
  };
  return (
    <section class="glass relative overflow-hidden flex min-h-[300px]">
      <div
        class="absolute inset-0 opacity-60 pointer-events-none"
        style={{ background: `radial-gradient(120% 90% at 0% 100%, oklch(0.55 0.2 ${inst().hue} / .45), transparent 60%)` }}
      />
      <div class="relative flex-1 p-7 flex flex-col">
        <div class="flex items-center gap-2 text-muted text-[12px] font-semibold uppercase tracking-wider">
          <span class="ping-dot" style={{ background: "var(--ok)", "box-shadow": "0 0 8px var(--ok)" }} />
          Ready to play · pre-flight passed
        </div>
        <div class="flex items-center gap-4 mt-4">
          <IconTile icon={inst().icon} hue={inst().hue} size={64} />
          <div>
            <h2 class="text-[34px] font-bold tracking-tight leading-none">{inst().name}</h2>
            <div class="flex gap-1.5 mt-2.5">
              <span class="chip">{inst().mcVersion}</span>
              <span class="chip">{LOADER_LABEL[inst().loader]} {inst().loaderVersion}</span>
              <Show when={inst().perfProfile}><span class="chip chip-accent"><Gauge size={12} /> Performance</span></Show>
            </div>
          </div>
        </div>

        <div class="flex gap-6 mt-6 text-[13px]">
          <div><div class="text-faint text-[11px] font-semibold uppercase tracking-wider">Playtime</div><div class="font-semibold mt-0.5">{inst().playtimeH}h</div></div>
          <div><div class="text-faint text-[11px] font-semibold uppercase tracking-wider">Last played</div><div class="font-semibold mt-0.5">{inst().lastPlayed}</div></div>
          <Show when={inst().lastBootMs}>
            <div>
              <div class="text-faint text-[11px] font-semibold uppercase tracking-wider">Boot time</div>
              <div class="font-semibold mt-0.5 flex items-center gap-1.5">
                <Timer size={13} class="text-accent" /> {(inst().lastBootMs! / 1000).toFixed(1)}s
                <Show when={speedup()}><span class="chip chip-ok">{speedup()}× vanilla</span></Show>
              </div>
            </div>
          </Show>
        </div>

        <div class="flex-1" />
        <div class="flex items-center gap-3 mt-6">
          <PlayButton instanceId={inst().id} />
          <select
            class="input h-[56px] rounded-2xl min-w-[180px] font-medium"
            value={inst().id}
            onChange={(e) => setApp("selectedInstanceId", e.currentTarget.value)}
          >
            <For each={app.instances}>{(i) => <option value={i.id}>{i.name}</option>}</For>
          </select>
          <Show when={app.launch.phase === "launching" && app.launch.instanceId === inst().id}>
            <div class="flex-1 max-w-[220px]">
              <div class="text-[11px] text-muted mb-1.5">Verifying files · spawning JVM (CDS on)</div>
              <div class="progress"><div style={{ width: `${app.launch.progress}%` }} /></div>
            </div>
          </Show>
        </div>
      </div>

      <div class="relative w-[260px] shrink-0 flex flex-col items-center justify-end pb-4">
        <div class="absolute bottom-6 w-40 h-8 rounded-[50%] blur-xl" style={{ background: "var(--accent-soft)" }} />
        <SkinViewer seed={activeAccount().skinSeed} cape={!!activeAccount().activeCape} width={240} height={280} />
        <A href="/accounts" class="glass-flat glass-hover px-3 py-1.5 flex items-center gap-2 text-[12.5px] font-semibold -mt-2">
          <ShieldCheck size={14} color={activeAccount().kind === "offline" ? "var(--faint)" : "var(--ok)"} />
          {activeAccount().username}
          <span class="text-faint font-medium">{activeAccount().kind === "offline" ? "Offline" : "Microsoft"}</span>
        </A>
      </div>
    </section>
  );
}

function Servers() {
  const [pinging, setPinging] = createSignal(false);
  const t = setInterval(() => {
    setPinging(true);
    setTimeout(() => setPinging(false), 700);
  }, 15000);
  onCleanup(() => clearInterval(t));
  return (
    <section class="glass p-5">
      <div class="flex items-center gap-2 mb-3">
        <ServerIcon size={16} class="text-accent" />
        <h3 class="font-semibold">Quick join</h3>
        <Show when={pinging()}><LoaderCircle size={13} class="spin text-faint" /></Show>
        <div class="flex-1" />
        <span class="text-faint text-[12px]">{selectedInstance().name}</span>
      </div>
      <div class="flex flex-col gap-1.5">
        <For each={SERVERS}>
          {(s) => (
            <div class="glass-flat glass-hover flex items-center gap-3 px-3 py-2.5 group">
              <div class="w-9 h-9 rounded-lg flex items-center justify-center font-bold text-[13px]" style={{ background: "var(--glass-strong)" }}>
                {s.name.slice(0, 2).toUpperCase()}
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="font-semibold text-[13px]">{s.name}</span>
                  <span class="chip" style={{ height: "18px", "font-size": "10px" }}>{s.tag}</span>
                </div>
                <div class="text-faint text-[11.5px] truncate">{s.motd}</div>
              </div>
              <div class="text-right text-[11.5px]">
                <div class="font-semibold">{fmt(s.players)} <span class="text-faint font-normal">online</span></div>
                <div class="flex items-center gap-1 justify-end text-faint">
                  <span class="ping-dot" style={{ background: s.ping < 60 ? "var(--ok)" : "var(--warn)" }} />
                  {s.ping}ms
                </div>
              </div>
              <button
                class="btn btn-primary h-8 px-3 opacity-0 group-hover:opacity-100 transition-opacity"
                onClick={() => {
                  toast(`Launching ${selectedInstance().name} → ${s.address} (quickPlayMultiplayer)`, "info");
                  launchInstance(selectedInstance().id);
                }}
              >
                Join
              </button>
            </div>
          )}
        </For>
      </div>
    </section>
  );
}

function News() {
  return (
    <section class="glass p-5 flex flex-col">
      <div class="flex items-center mb-3">
        <h3 class="font-semibold">News</h3>
        <div class="flex-1" />
        <button class="btn btn-ghost h-7 px-2 text-[12px] text-muted">All <ChevronRight size={13} /></button>
      </div>
      <div class="flex flex-col gap-2.5">
        <For each={NEWS.slice(0, 3)}>
          {(n, i) => (
            <>
              <article class="glass-flat glass-hover flex gap-3 p-3 cursor-pointer">
                <div class="w-16 h-16 rounded-xl shrink-0" style={{ background: `linear-gradient(135deg, oklch(0.6 0.18 ${n.hue}), oklch(0.4 0.18 ${n.hue + 60}))` }} />
                <div class="min-w-0">
                  <div class="flex items-center gap-2 text-[11px]">
                    <span class="chip chip-accent" style={{ height: "18px" }}>{n.tag}</span>
                    <span class="text-faint">{n.date}</span>
                  </div>
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
  const recent = () => app.instances.slice(0, 4);
  return (
    <div class="page-enter flex flex-col gap-4 pb-2">
      <Hero />
      <section>
        <div class="flex items-center mb-2.5 px-1">
          <h3 class="font-semibold flex items-center gap-2"><Clock size={15} class="text-accent" /> Recent</h3>
          <div class="flex-1" />
          <A href="/instances" class="text-muted text-[12px] flex items-center gap-1 hover:text-[var(--text)]">All instances <ChevronRight size={13} /></A>
        </div>
        <div class="grid grid-cols-4 gap-3">
          <For each={recent()}>
            {(i) => (
              <div
                class="glass glass-hover lift p-3.5 flex flex-col gap-3 cursor-pointer"
                classList={{ "!border-[var(--accent)]": app.selectedInstanceId === i.id }}
                onClick={() => setApp("selectedInstanceId", i.id)}
                onDblClick={() => nav(`/instances/${i.id}`)}
              >
                <div class="flex items-center gap-3">
                  <IconTile icon={i.icon} hue={i.hue} size={40} />
                  <div class="min-w-0">
                    <div class="font-semibold truncate">{i.name}</div>
                    <div class="text-faint text-[11.5px]">{i.mcVersion} · {LOADER_LABEL[i.loader]}</div>
                  </div>
                </div>
                <div class="flex items-center">
                  <span class="text-faint text-[11.5px]">{i.lastPlayed}</span>
                  <div class="flex-1" />
                  <PlayButton instanceId={i.id} size="md" />
                </div>
              </div>
            )}
          </For>
        </div>
      </section>
      <div class="grid grid-cols-[1.25fr_1fr] gap-4">
        <Servers />
        <News />
      </div>
    </div>
  );
}
