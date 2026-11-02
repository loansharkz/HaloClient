import { useNavigate } from "@solidjs/router";
import { Show, createSignal } from "solid-js";
import { Bug, FileText, LoaderCircle, Play, Power, ShieldCheck, TriangleAlert, Wrench, Zap } from "lucide-solid";
import { Modal } from "./Shell";
import { api } from "../lib/api";
import { app, attempt, instanceById, isBusy, launchInstance, refreshInstances, setApp, setSettings, settings, toast } from "../lib/store";

/** Play / progress / stop button for one instance. */
export function PlayButton(props: { instanceId: string; size?: "lg" | "md"; server?: string | null; label?: string }) {
  const l = () => app.launch[props.instanceId];
  const phase = () => l()?.phase ?? "idle";
  const task = () => Object.values(app.tasks).find((t) => t && t.done < t.total);
  const pct = () => {
    const t = task();
    return t && t.total > 1 ? Math.round((t.done / t.total) * 100) : null;
  };
  return (
    <button
      class="btn btn-primary"
      classList={{ "play-btn": props.size !== "md", "h-8 px-3": props.size === "md", running: phase() === "running" }}
      title={phase() === "preparing" ? l()?.message : phase() === "ready" ? "Instant Play: opens in about a second" : undefined}
      onClick={(e) => {
        e.stopPropagation();
        e.preventDefault();
        launchInstance(props.instanceId, props.server);
      }}
      disabled={phase() === "preparing"}
    >
      <Show when={phase() === "ready"}>
        <Zap size={props.size === "md" ? 14 : 20} fill="currentColor" /> {props.label ?? "Play"}
      </Show>
      <Show when={phase() === "idle" || phase() === "stopped" || phase() === "error" || phase() === "prewarming"}>
        <Play size={props.size === "md" ? 14 : 20} fill="currentColor" /> {props.label ?? "Play"}
      </Show>
      <Show when={phase() === "preparing"}>
        <LoaderCircle size={props.size === "md" ? 14 : 18} class="spin" /> {pct() != null ? `${pct()}%` : props.size === "md" ? "" : "Starting"}
      </Show>
      <Show when={phase() === "running"}>
        <Power size={props.size === "md" ? 14 : 18} /> {props.size === "md" ? "Stop" : "Running · Stop"}
      </Show>
    </button>
  );
}

const DRIVER_HELP = "https://support.microsoft.com/windows/update-drivers-manually-in-windows-ec62f46c-ff14-c91d-eead-d7126dc1f7b6";

/** Shown when a game exits with a crash: cause + one-click fix. */
export function CrashDialog() {
  const nav = useNavigate();
  const [busy, setBusy] = createSignal(false);
  const exit = () => app.exit;
  const inst = () => (exit() ? instanceById(exit()!.instanceId) : null);
  const close = () => setApp("exit", null);

  async function fix() {
    const e = exit();
    const i = inst();
    if (!e?.crash?.fix || !i) return;
    const f = e.crash.fix;
    setBusy(true);
    try {
      switch (f.kind) {
        case "install_dependency":
          await attempt(() => api.installProject({ instanceId: i.id, source: "modrinth", projectId: f.arg!, kind: "mod" }), `Installed ${f.arg}`);
          break;
        case "disable_mod": {
          const items = await api.listContent(i.id);
          const needle = (f.arg ?? "").toLowerCase();
          const m = items.find((c) => c.meta?.slug === needle || c.name.toLowerCase().includes(needle) || c.fileName.toLowerCase().includes(needle));
          if (m) await attempt(() => api.toggleContent(i.id, m.key, false), `Disabled ${m.name}`);
          else toast(`Couldn't find '${f.arg}' in this instance`, "warn");
          break;
        }
        case "increase_ram":
          await attempt(() => api.updateInstance({ ...i, ramMb: Math.min(16384, (i.ramMb ?? settings.defaultRamMb) + 2048) }), "Gave the instance 2 GB more RAM");
          break;
        case "auto_java":
          setSettings("autoJava", true);
          await attempt(() => api.updateInstance({ ...i, javaPath: null }), "Switched to Halo's managed Java");
          break;
        case "reset_jvm_args":
          await attempt(() => api.updateInstance({ ...i, jvmArgs: "" }), "JVM arguments reset");
          break;
        case "update_drivers":
          await attempt(() => api.openUrl(DRIVER_HELP));
          break;
        case "remove_duplicate":
          close();
          nav(`/instance/${i.id}`);
          return;
      }
      await refreshInstances();
      close();
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={!!exit()?.crash} onClose={close} width={480} label="Game crashed">
      <Show when={exit()?.crash}>
        {(c) => (
          <div class="p-6">
            <div class="w-11 h-11 rounded-2xl flex items-center justify-center mb-3" style={{ background: "rgb(248 113 113 / .15)" }}>
              <TriangleAlert size={22} color="var(--bad)" />
            </div>
            <h2 class="text-lg font-bold">{c().title}</h2>
            <p class="text-muted text-[13px] mt-1">{c().detail}</p>
            <p class="text-faint text-[12px] mt-2">{inst()?.name} · exit code {exit()?.code ?? "?"}</p>
            <div class="flex gap-2 mt-5 flex-wrap">
              <Show when={c().fix}>
                <button class="btn btn-primary" disabled={busy()} onClick={fix}>
                  <Show when={busy()} fallback={<Wrench size={14} />}><LoaderCircle size={14} class="spin" /></Show>
                  {c().fix!.label}
                </button>
              </Show>
              <button class="btn" onClick={() => { close(); nav(`/instance/${exit()!.instanceId}?tab=logs`); }}>
                <FileText size={14} /> Open log
              </button>
              <Show when={c().crashFile}>
                <button class="btn" onClick={() => attempt(() => api.openPath(c().crashFile!))}><Bug size={14} /> Crash report</button>
              </Show>
              <div class="flex-1" />
              <button class="btn btn-ghost" onClick={() => { const id = exit()!.instanceId; close(); if (!isBusy(id)) launchInstance(id); }}>
                <Play size={14} /> Try again
              </button>
            </div>
          </div>
        )}
      </Show>
    </Modal>
  );
}

/** One-time notice that crash reports are on, with an easy opt-out. */
export function FirstRunBanner() {
  return (
    <Show when={app.ready && app.info?.sentry && !settings.crashReportsPrompted}>
      <div class="glass fixed bottom-5 left-[92px] z-[150] p-4 max-w-[440px] flex gap-3 items-start toast" style={{ background: "var(--glass-strong)" }}>
        <ShieldCheck size={20} class="text-accent shrink-0 mt-0.5" />
        <div class="text-[13px]">
          <div class="font-semibold">Help fix launcher crashes?</div>
          <p class="text-muted mt-0.5">Halo sends anonymous launcher crash reports. Never your username, tokens or files. You can change this any time in Settings.</p>
          <div class="flex gap-2 mt-3">
            <button class="btn btn-primary h-8" onClick={() => setSettings({ crashReportsPrompted: true, crashReports: true })}>Keep on</button>
            <button class="btn h-8" onClick={() => { setSettings({ crashReportsPrompted: true, crashReports: false }); toast("Crash reports off. Takes effect after a restart.", "info"); }}>Turn off</button>
          </div>
        </div>
      </div>
    </Show>
  );
}
