import { Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import {
  Apple, Atom, Book, Box, Coins, EyeOff, FlaskConical, Hammer, List, Map, Package, Rocket, Sparkles, Swords, Zap,
} from "lucide-solid";
import { projectMedia, serverMedia } from "../lib/media";
import type { Instance } from "../lib/mock";

const ICONS = {
  swords: Swords, coins: Coins, package: Package, box: Box, flask: FlaskConical, hammer: Hammer,
  zap: Zap, sparkles: Sparkles, rocket: Rocket, atom: Atom, map: Map, list: List, "eye-off": EyeOff, apple: Apple, book: Book,
} as const;

/** Gradient tile with an icon: used for instance + project icons (no Mojang assets). */
export function IconTile(props: { icon: string; hue: number; size?: number; radius?: number; src?: string; pixel?: boolean }) {
  const size = () => props.size ?? 44;
  return (
    <Show when={props.src} fallback={<GradientTile {...props} />}>
      <img
        src={props.src}
        alt=""
        draggable={false}
        class="shrink-0 object-cover"
        style={{
          width: `${size()}px`,
          height: `${size()}px`,
          "border-radius": `${props.radius ?? Math.round(size() * 0.28)}px`,
          "image-rendering": props.pixel ? "pixelated" : "auto",
          background: "var(--glass-strong)",
          "box-shadow": "0 4px 14px rgb(0 0 0 / .25)",
        }}
      />
    </Show>
  );
}

function GradientTile(props: { icon: string; hue: number; size?: number; radius?: number }) {
  const size = () => props.size ?? 44;
  return (
    <div
      class="flex items-center justify-center shrink-0 text-white"
      style={{
        width: `${size()}px`,
        height: `${size()}px`,
        "border-radius": `${props.radius ?? Math.round(size() * 0.28)}px`,
        background: `linear-gradient(135deg, oklch(0.62 0.19 ${props.hue}), oklch(0.45 0.2 ${props.hue + 40}))`,
        "box-shadow": `inset 0 1px 0 rgb(255 255 255 / .25), 0 4px 14px oklch(0.5 0.2 ${props.hue} / .3)`,
      }}
    >
      <Dynamic component={ICONS[props.icon as keyof typeof ICONS] ?? Box} size={Math.round(size() * 0.48)} stroke-width={2} />
    </div>
  );
}

export function HaloLogo(props: { size?: number }) {
  const s = () => props.size ?? 28;
  return (
    <svg width={s()} height={s()} viewBox="0 0 64 64" class="halo-glow">
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

/** Instance icon: the server's real favicon when the instance is tied to one, else a gradient tile. */
export function InstanceIcon(props: { inst: Instance; size?: number }) {
  return <IconTile icon={props.inst.icon} hue={props.inst.hue} size={props.size} src={serverMedia(props.inst.iconServer)?.icon} pixel />;
}

/** Mod/project icon from Modrinth, else a tile tinted from the name. */
export function ModIcon(props: { slug?: string; name: string; size?: number; icon?: string; hue?: number }) {
  const hue = () => props.hue ?? [...props.name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);
  return <IconTile icon={props.icon ?? "package"} hue={hue()} size={props.size} src={projectMedia(props.slug)?.icon} />;
}
