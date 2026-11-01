import { For, Show, createResource, createSignal, lazy, onCleanup } from "solid-js";
import { Check, Copy, ExternalLink, KeyRound, LoaderCircle, Shirt, Trash2, Upload, UserPlus } from "lucide-solid";
import { Modal, PageHeader } from "../components/Shell";
import { api, pickFile } from "../lib/api";
import { IS_TAURI } from "../lib/config";
import { activeAccount, app, attempt, refreshAccounts, setActiveAccount, toast } from "../lib/store";
import type { AuthStart } from "../lib/types";

const SkinViewer = lazy(() => import("../components/SkinViewer"));

function AddMicrosoft(props: { open: boolean; onClose: () => void }) {
  const [flow, setFlow] = createSignal<AuthStart | null>(null);
  const [state, setState] = createSignal<"idle" | "starting" | "waiting" | "done" | "error">("idle");
  const [error, setError] = createSignal("");
  let cancelled = false;

  async function start() {
    cancelled = false;
    setError("");
    setState("starting");
    try {
      const f = await api.authStart();
      setFlow(f);
      setState("waiting");
      await api.openUrl(f.verificationUri).catch(() => {});
      const acc = await api.authPoll(f.flowId);
      if (cancelled) return;
      setState("done");
      await refreshAccounts();
      toast(`Signed in as ${acc.username}. Java Edition ownership verified.`, "ok");
      setTimeout(close, 600);
    } catch (e) {
      if (cancelled) return;
      setError(String(e));
      setState("error");
    }
  }
  function close() {
    cancelled = true;
    const f = flow();
    if (f && state() === "waiting") api.authCancel(f.flowId).catch(() => {});
    setFlow(null);
    setState("idle");
    props.onClose();
  }
  onCleanup(() => (cancelled = true));

  return (
    <Modal open={props.open} onClose={close} width={460} label="Sign in with Microsoft">
      <div class="p-6 text-center">
        <div class="w-12 h-12 rounded-2xl mx-auto mb-3 flex items-center justify-center" style={{ background: "var(--accent-soft)" }}>
          <KeyRound size={22} class="text-accent" />
        </div>
        <h2 class="text-lg font-bold">Sign in with Microsoft</h2>
        <Show when={app.info && !app.info.msaConfigured}>
          <p class="text-muted text-[13px] mt-2">Microsoft login isn't set up in this build yet. The developer needs to register an Azure app and set <span class="mono">HALO_MSA_CLIENT_ID</span> (see the README).</p>
        </Show>
        <Show when={app.info?.msaConfigured !== false}>
          <Show when={state() === "idle" || state() === "starting"}>
            <p class="text-muted text-[13px] mt-1">You'll get a short code to enter on microsoft.com. Halo never sees your password.</p>
            <button class="btn btn-primary mt-5" disabled={state() === "starting"} onClick={start}>
              <Show when={state() === "starting"} fallback={<ExternalLink size={14} />}><LoaderCircle size={14} class="spin" /></Show> Get sign-in code
            </button>
          </Show>
          <Show when={(state() === "waiting" || state() === "done") && flow()}>
            <p class="text-muted text-[13px] mt-1">Enter this code at <b>{flow()!.verificationUri.replace("https://", "")}</b></p>
            <div class="glass-flat mt-4 py-4 mono text-[28px] font-bold tracking-[0.25em] select-text">{flow()!.userCode}</div>
            <div class="flex gap-2 justify-center mt-4">
              <button class="btn" onClick={() => navigator.clipboard.writeText(flow()!.userCode).then(() => toast("Code copied", "ok")).catch(() => toast("Select the code to copy it", "info"))}><Copy size={14} /> Copy code</button>
              <button class="btn" onClick={() => api.openUrl(flow()!.verificationUri)}><ExternalLink size={14} /> Open page</button>
            </div>
            <div class="flex items-center justify-center gap-2 text-[12.5px] text-muted mt-4">
              <Show when={state() === "waiting"} fallback={<><Check size={14} color="var(--ok)" /> Signed in</>}>
                <LoaderCircle size={14} class="spin" /> Waiting for you to finish in the browser…
              </Show>
            </div>
          </Show>
          <Show when={state() === "error"}>
            <p class="text-[13px] mt-3" style={{ color: "var(--bad)" }}>{error()}</p>
            <button class="btn btn-primary mt-4" onClick={start}>Try again</button>
          </Show>
        </Show>
        <p class="text-faint text-[11px] mt-5">Microsoft → Xbox Live → Minecraft. Tokens are kept in Windows Credential Manager.</p>
      </div>
    </Modal>
  );
}

function AddOffline(props: { open: boolean; onClose: () => void }) {
  const [name, setName] = createSignal("");
  const hasMs = () => app.accounts.some((a) => a.kind === "microsoft");
  const valid = () => /^[A-Za-z0-9_]{3,16}$/.test(name());
  return (
    <Modal open={props.open} onClose={props.onClose} width={420} label="Add offline profile">
      <div class="p-6">
        <h2 class="text-lg font-bold">Add offline profile</h2>
        <p class="text-muted text-[12.5px] mt-1">For singleplayer, LAN and offline-mode servers. Offline profiles can't join online-mode servers like Hypixel.</p>
        <Show when={hasMs()} fallback={<div class="glass-flat p-3 mt-4 text-[12.5px]">Add a Microsoft account that owns Minecraft: Java Edition first.</div>}>
          <form onSubmit={async (e) => {
            e.preventDefault();
            if (!valid()) return;
            if (await attempt(() => api.addOffline(name()), `Added offline profile ${name()}`)) {
              await refreshAccounts();
              setName("");
              props.onClose();
            }
          }}>
            <input id="offline-name" class="input w-full mt-4" placeholder="Username (3–16 characters: A–Z, 0–9, _)" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
            <div class="flex justify-end gap-2 mt-4">
              <button type="button" class="btn" onClick={props.onClose}>Cancel</button>
              <button type="submit" class="btn btn-primary" disabled={!valid()}>Add profile</button>
            </div>
          </form>
        </Show>
      </div>
    </Modal>
  );
}

export default function Accounts() {
  const [msOpen, setMsOpen] = createSignal(false);
  const [offOpen, setOffOpen] = createSignal(false);
  const [slim, setSlim] = createSignal(false);
  const [working, setWorking] = createSignal(false);
  const acc = activeAccount;
  const [textures, { refetch }] = createResource(() => acc()?.id, (id) => api.accountTextures(id).catch(() => null));

  async function uploadSkin() {
    if (!IS_TAURI) return toast("Skin upload opens a file picker in the desktop app.", "info");
    const p = await pickFile({ title: "Skin PNG", extensions: ["png"] });
    if (!p) return;
    setWorking(true);
    if (await attempt(() => api.uploadSkin(acc()!.id, p, slim()), "Skin updated")) {
      await refreshAccounts();
      refetch();
    }
    setWorking(false);
  }
  async function cape(id: string | null) {
    setWorking(true);
    if (await attempt(() => api.setCape(acc()!.id, id), id ? "Cape equipped" : "Cape hidden")) {
      await refreshAccounts();
      refetch();
    }
    setWorking(false);
  }

  return (
    <div class="page-enter pb-2">
      <PageHeader title="Accounts" sub="Switch alts in one click. Halo never asks you to make a Halo account.">
        <button class="btn" onClick={() => setOffOpen(true)}>Add offline</button>
        <button class="btn btn-primary" onClick={() => setMsOpen(true)}><UserPlus size={15} /> Add Microsoft account</button>
      </PageHeader>

      <div class="grid grid-cols-[1fr_320px] gap-4">
        <div class="flex flex-col gap-2">
          <Show when={app.accounts.length} fallback={<div class="glass p-8 text-center text-muted">No accounts yet. Add your Microsoft account to play online.</div>}>
            <For each={app.accounts}>
              {(a) => (
                <div class="glass glass-hover p-4 flex items-center gap-4 cursor-pointer" classList={{ "!border-[var(--accent)]": a.id === app.activeAccountId }} onClick={() => a.id !== app.activeAccountId && setActiveAccount(a.id).then(() => toast(`Switched to ${a.username}`, "ok"))}>
                  <div class="w-11 h-11 rounded-xl flex items-center justify-center font-bold text-white" style={{ background: a.kind === "offline" ? "var(--glass-strong)" : "linear-gradient(135deg, var(--accent), var(--accent-2))" }}>{a.username[0]}</div>
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2">
                      <span class="font-semibold">{a.username}</span>
                      <span class="chip">{a.kind === "offline" ? "Offline" : "Microsoft"}</span>
                      <Show when={a.kind === "microsoft" && a.expiresAt < Date.now()}><span class="chip chip-warn">Refreshes on next launch</span></Show>
                    </div>
                    <div class="text-faint text-[11.5px] mono mt-0.5 truncate">{a.kind === "offline" ? "Singleplayer, LAN and offline-mode servers" : a.uuid}</div>
                  </div>
                  <Show when={a.id === app.activeAccountId} fallback={<span class="text-faint text-[12px]">Click to switch</span>}>
                    <span class="chip chip-ok"><Check size={11} /> Active</span>
                  </Show>
                  <button class="btn btn-ghost btn-icon h-8 text-faint hover:!text-[var(--bad)]" aria-label={`Remove ${a.username}`} onClick={async (e) => { e.stopPropagation(); if ((await attempt(() => api.removeAccount(a.id), `Removed ${a.username}`)) !== undefined) refreshAccounts(); }}>
                    <Trash2 size={15} />
                  </button>
                </div>
              )}
            </For>
          </Show>
        </div>

        <div class="glass p-5 flex flex-col items-center">
          <div class="font-semibold self-start">Skin & cape</div>
          <div class="text-faint text-[12px] self-start">{acc()?.username ?? "No account"}</div>
          <SkinViewer seed={(acc()?.username.charCodeAt(0) ?? 39) * 7} skin={textures()?.skin} cape={textures()?.cape} slim={textures()?.slim} width={220} height={270} />
          <Show when={acc()?.kind === "microsoft"} fallback={<p class="text-faint text-[12px] text-center">Offline profiles use the default skin.</p>}>
            <div class="flex gap-2 w-full">
              <button class="btn flex-1" disabled={working()} onClick={uploadSkin}><Upload size={14} /> Upload skin</button>
              <button class="btn" classList={{ "!border-[var(--accent)]": slim() }} onClick={() => setSlim(!slim())} title="Arm model for the next upload"><Shirt size={14} /> {slim() ? "Slim" : "Classic"}</button>
            </div>
            <div class="w-full mt-4">
              <div class="field-label mb-2">Capes</div>
              <div class="flex flex-wrap gap-1.5">
                <button class="chip cursor-pointer" disabled={working()} classList={{ "chip-accent": !acc()!.capes.some((c) => c.active) }} onClick={() => cape(null)}>None</button>
                <For each={acc()!.capes}>{(c) => <button class="chip cursor-pointer" disabled={working()} classList={{ "chip-accent": c.active }} onClick={() => cape(c.id)}>{c.alias}</button>}</For>
                <Show when={acc()!.capes.length === 0}><span class="text-faint text-[12px]">No capes on this account</span></Show>
              </div>
            </div>
          </Show>
        </div>
      </div>
      <AddMicrosoft open={msOpen()} onClose={() => setMsOpen(false)} />
      <AddOffline open={offOpen()} onClose={() => setOffOpen(false)} />
    </div>
  );
}
