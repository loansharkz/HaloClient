import { For, Show, createSignal, lazy } from "solid-js";
import { Check, Copy, ExternalLink, KeyRound, LoaderCircle, Shirt, Trash2, Upload, UserPlus } from "lucide-solid";
import { Modal, PageHeader } from "../components/Shell";
import type { Account } from "../lib/mock";
import { activeAccount, app, setApp, toast } from "../lib/store";

const SkinViewer = lazy(() => import("../components/SkinViewer"));

function AddMicrosoft(props: { open: boolean; onClose: () => void }) {
  const [step, setStep] = createSignal<"code" | "waiting" | "done">("code");
  const finish = () => {
    setStep("waiting");
    setTimeout(() => {
      setStep("done");
      const acc: Account = { id: `a${Date.now()}`, kind: "microsoft", username: "NewAlt", uuid: "c0de…f00d", skinSeed: Math.floor(Math.random() * 360), capes: [], activeCape: null, status: "ok" };
      setApp("accounts", (l) => [...l, acc]);
      setTimeout(() => {
        props.onClose();
        setStep("code");
        toast("Added NewAlt. Java Edition ownership verified", "ok");
      }, 700);
    }, 1400);
  };
  return (
    <Modal open={props.open} onClose={props.onClose} width={440}>
      <div class="p-6 text-center">
        <div class="w-12 h-12 rounded-2xl mx-auto mb-3 flex items-center justify-center" style={{ background: "var(--accent-soft)" }}>
          <KeyRound size={22} class="text-accent" />
        </div>
        <h2 class="text-lg font-bold">Sign in with Microsoft</h2>
        <p class="text-muted text-[13px] mt-1">Open the link and enter this code. Halo never sees your password.</p>
        <div class="glass-flat mt-5 py-4 mono text-[28px] font-bold tracking-[0.25em] select-text">HALO-7Q2K</div>
        <div class="flex gap-2 justify-center mt-4">
          <button class="btn" onClick={() => toast("Code copied", "ok")}><Copy size={14} /> Copy code</button>
          <button class="btn btn-primary" onClick={finish} disabled={step() !== "code"}>
            <Show when={step() === "code"}><ExternalLink size={14} /> Open microsoft.com/link</Show>
            <Show when={step() === "waiting"}><LoaderCircle size={14} class="spin" /> Waiting for sign-in…</Show>
            <Show when={step() === "done"}><Check size={14} /> Signed in</Show>
          </button>
        </div>
        <p class="text-faint text-[11px] mt-5">Microsoft → Xbox Live → XSTS → Minecraft. Tokens are stored in Windows Credential Manager.</p>
      </div>
    </Modal>
  );
}

function AddOffline(props: { open: boolean; onClose: () => void }) {
  const [name, setName] = createSignal("");
  const hasMs = () => app.accounts.some((a) => a.kind === "microsoft");
  const valid = () => /^[A-Za-z0-9_]{3,16}$/.test(name());
  return (
    <Modal open={props.open} onClose={props.onClose} width={420}>
      <div class="p-6">
        <h2 class="text-lg font-bold">Add offline profile</h2>
        <p class="text-muted text-[12.5px] mt-1">For singleplayer, LAN and offline-mode servers. Offline profiles can't join online-mode servers like Hypixel.</p>
        <Show when={hasMs()} fallback={<div class="glass-flat p-3 mt-4 text-[12.5px]">Add a Microsoft account that owns Java Edition first.</div>}>
          <input class="input w-full mt-4" placeholder="Username (3–16 chars, A–Z 0–9 _)" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
          <div class="flex justify-end gap-2 mt-4">
            <button class="btn" onClick={props.onClose}>Cancel</button>
            <button class="btn btn-primary" disabled={!valid()} onClick={() => {
              setApp("accounts", (l) => [...l, { id: `a${Date.now()}`, kind: "offline", username: name(), uuid: "offline", skinSeed: 330, capes: [], activeCape: null, status: "ok" }]);
              toast(`Added offline profile ${name()}`, "ok");
              setName("");
              props.onClose();
            }}>Add profile</button>
          </div>
        </Show>
      </div>
    </Modal>
  );
}

export default function Accounts() {
  const [msOpen, setMsOpen] = createSignal(false);
  const [offOpen, setOffOpen] = createSignal(false);
  const acc = activeAccount;
  return (
    <div class="page-enter pb-2">
      <PageHeader title="Accounts" sub="Switch alts in one click. No Halo account, ever.">
        <button class="btn" onClick={() => setOffOpen(true)}>Add offline</button>
        <button class="btn btn-primary" onClick={() => setMsOpen(true)}><UserPlus size={15} /> Add Microsoft account</button>
      </PageHeader>

      <div class="grid grid-cols-[1fr_320px] gap-4">
        <div class="flex flex-col gap-2">
          <For each={app.accounts}>
            {(a) => (
              <div
                class="glass glass-hover p-4 flex items-center gap-4 cursor-pointer"
                classList={{ "!border-[var(--accent)]": a.id === app.activeAccountId }}
                onClick={() => { setApp("activeAccountId", a.id); toast(`Switched to ${a.username}`, "ok"); }}
              >
                <div class="w-11 h-11 rounded-xl flex items-center justify-center font-bold text-white" style={{ background: `linear-gradient(135deg, hsl(${a.skinSeed} 70% 55%), hsl(${a.skinSeed + 50} 70% 45%))` }}>{a.username[0]}</div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="font-semibold">{a.username}</span>
                    <Show when={a.kind === "offline"}><span class="chip">Offline</span></Show>
                    <Show when={a.kind === "microsoft"}><span class="chip">Microsoft</span></Show>
                    <Show when={a.status === "refresh"}><span class="chip chip-warn">Token refreshes on launch</span></Show>
                  </div>
                  <div class="text-faint text-[11.5px] mono mt-0.5">{a.uuid}</div>
                </div>
                <Show when={a.id === app.activeAccountId} fallback={<span class="text-faint text-[12px]">Click to switch</span>}>
                  <span class="chip chip-ok"><Check size={11} /> Active</span>
                </Show>
                <button class="btn btn-ghost btn-icon h-8 text-faint hover:!text-[var(--bad)]" disabled={app.accounts.length < 2} onClick={(e) => {
                  e.stopPropagation();
                  setApp("accounts", (l) => l.filter((x) => x.id !== a.id));
                  if (app.activeAccountId === a.id) setApp("activeAccountId", app.accounts[0].id);
                  toast(`Removed ${a.username}`, "info");
                }}>
                  <Trash2 size={15} />
                </button>
              </div>
            )}
          </For>
        </div>

        <div class="glass p-5 flex flex-col items-center">
          <div class="font-semibold self-start">Skin & cape</div>
          <div class="text-faint text-[12px] self-start">{acc().username}</div>
          <SkinViewer seed={acc().skinSeed} cape={!!acc().activeCape} width={220} height={270} />
          <Show when={acc().kind === "microsoft"} fallback={<p class="text-faint text-[12px] text-center">Offline profiles use a local skin only.</p>}>
            <div class="flex gap-2 w-full">
              <button class="btn flex-1" onClick={() => toast("Skin uploaded via Mojang API", "ok")}><Upload size={14} /> Upload skin</button>
              <button class="btn flex-1" onClick={() => toast("Classic/Slim model toggled", "info")}><Shirt size={14} /> Slim / Classic</button>
            </div>
            <div class="w-full mt-4">
              <div class="text-[12px] font-semibold text-muted mb-2">Capes</div>
              <div class="flex flex-wrap gap-1.5">
                <button class="chip cursor-pointer" classList={{ "chip-accent": !acc().activeCape }} onClick={() => setApp("accounts", (x) => x.id === acc().id, "activeCape", null)}>None</button>
                <For each={acc().capes}>
                  {(c) => <button class="chip cursor-pointer" classList={{ "chip-accent": acc().activeCape === c }} onClick={() => setApp("accounts", (x) => x.id === acc().id, "activeCape", c)}>{c}</button>}
                </For>
                <Show when={acc().capes.length === 0}><span class="text-faint text-[12px]">No capes on this account</span></Show>
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
