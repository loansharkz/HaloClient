import { Show, createEffect, createResource } from "solid-js";
import { Dynamic } from "solid-js/web";
import {
  Apple, Atom, Book, Box, Coins, EyeOff, FlaskConical, Gamepad2, Hammer, List, Map, Package, Pickaxe, Rocket, Sparkles, Swords, Zap,
} from "lucide-solid";
import { fileSrc } from "../lib/api";
import { app, pingServer } from "../lib/store";
import type { Instance } from "../lib/types";

export const ICONS = {
  swords: Swords, coins: Coins, package: Package, box: Box, flask: FlaskConical, hammer: Hammer, pickaxe: Pickaxe, gamepad: Gamepad2,
  zap: Zap, sparkles: Sparkles, rocket: Rocket, atom: Atom, map: Map, list: List, "eye-off": EyeOff, apple: Apple, book: Book,
} as const;
export const ICON_CHOICES = ["box", "swords", "coins", "package", "pickaxe", "hammer", "flask", "gamepad", "rocket", "sparkles", "map", "book"] as const;

/** Square icon: an image when `src` is set, otherwise a tinted gradient tile with a glyph. */
export function IconTile(props: { icon?: string; hue: number; size?: number; radius?: number; src?: string | null; pixel?: boolean }) {
  const size = () => props.size ?? 44;
  const radius = () => `${props.radius ?? Math.round(size() * 0.24)}px`;
  return (
    <Show
      when={props.src}
      fallback={
        <div
          class="flex items-center justify-center shrink-0 text-white"
          style={{
            width: `${size()}px`,
            height: `${size()}px`,
            "border-radius": radius(),
            background: `linear-gradient(135deg, oklch(0.62 0.19 ${props.hue}), oklch(0.45 0.2 ${props.hue + 40}))`,
            "box-shadow": `inset 0 1px 0 rgb(255 255 255 / .25)`,
          }}
        >
          <Dynamic component={ICONS[(props.icon ?? "box") as keyof typeof ICONS] ?? Box} size={Math.round(size() * 0.48)} stroke-width={2} />
        </div>
      }
    >
      <img
        src={props.src!}
        alt=""
        draggable={false}
        loading="lazy"
        class="shrink-0 object-cover"
        style={{
          width: `${size()}px`,
          height: `${size()}px`,
          "border-radius": radius(),
          "image-rendering": props.pixel ? "pixelated" : "auto",
          background: "var(--glass-strong)",
        }}
      />
    </Show>
  );
}

/** Instance icon: custom image, else the linked server's favicon, else a gradient tile. */
export function InstanceIcon(props: { inst: Instance; size?: number }) {
  const [custom] = createResource(() => props.inst.iconPath, (p) => fileSrc(p));
  createEffect(() => props.inst.iconServer && pingServer(props.inst.iconServer));
  const favicon = () => (props.inst.iconServer ? app.serverStatus[props.inst.iconServer]?.favicon : null);
  return <IconTile icon={props.inst.icon} hue={props.inst.hue} size={props.size} src={custom() ?? favicon()} pixel={!custom()} />;
}

/** Stable hue derived from a name. */
export const hueOf = (name: string) => [...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);

export function HaloLogo(props: { size?: number }) {
  const s = () => props.size ?? 28;
  return (
    <svg width={s()} height={s()} viewBox="0 0 64 64" class="halo-glow" aria-hidden="true">
      <defs>
        <linearGradient id="hl" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="oklch(0.85 0.1 var(--accent-h))" />
          <stop offset=".5" stop-color="var(--accent)" />
          <stop offset="1" stop-color="var(--accent-2)" />
        </linearGradient>
      </defs>
      <g transform="rotate(-14 32 32)">
        <ellipse cx="32" cy="32" rx="24" ry="10" fill="none" stroke="url(#hl)" stroke-width="5.5" />
      </g>
      <circle cx="32" cy="32" r="3" fill="var(--text)" />
    </svg>
  );
}
