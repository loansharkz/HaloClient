import { For } from "solid-js";

interface Span { text: string; color?: string; bold?: boolean; italic?: boolean; underline?: boolean; strike?: boolean; obf?: boolean }

/** Parses Minecraft "§" formatting codes into styled spans. Never renders HTML from the server. */
export function parseMotd(line: string): Span[] {
  const out: Span[] = [];
  let cur: Span = { text: "" };
  const push = () => { if (cur.text) out.push(cur); };
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === "§" && i + 1 < line.length) {
      const code = line[++i].toLowerCase();
      push();
      const base: Span = { text: "" };
      if (/[0-9a-f]/.test(code)) cur = { ...base, color: code };
      else if (code === "r") cur = base;
      else cur = { ...cur, text: "", bold: cur.bold || code === "l", italic: cur.italic || code === "o", underline: cur.underline || code === "n", strike: cur.strike || code === "m", obf: cur.obf || code === "k" };
      continue;
    }
    cur.text += ch;
  }
  push();
  return out;
}

export default function Motd(props: { lines: string[]; class?: string }) {
  return (
    <div class={props.class}>
      <For each={props.lines}>
        {(line) => (
          <div class="truncate">
            <For each={parseMotd(line)}>
              {(s) => (
                <span
                  class={s.color ? `mc-${s.color}` : undefined}
                  classList={{ "mc-obf": !!s.obf }}
                  style={{
                    "font-weight": s.bold ? 700 : undefined,
                    "font-style": s.italic ? "italic" : undefined,
                    "text-decoration": [s.underline && "underline", s.strike && "line-through"].filter(Boolean).join(" ") || undefined,
                  }}
                >
                  {s.text}
                </span>
              )}
            </For>
          </div>
        )}
      </For>
    </div>
  );
}
