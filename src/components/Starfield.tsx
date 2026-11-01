import { createEffect, onCleanup, onMount } from "solid-js";
import { app, settings } from "../lib/store";

interface Star { x: number; y: number; r: number; a: number; phase: number; speed: number; tinted: boolean }
interface Meteor { x: number; y: number; vx: number; vy: number; len: number; life: number; maxLife: number; width: number }

/**
 * Night-sky background: twinkling stars plus occasional shooting stars, on a
 * single 2D canvas. The loop stops completely when the window is hidden, the
 * game is running, animation is turned off, or the OS asks for reduced motion.
 * In those cases it paints one still frame, so idle CPU/GPU cost is zero.
 */
export default function Starfield() {
  let canvas!: HTMLCanvasElement;

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)");
    let w = 0, h = 0, stars: Star[] = [], meteors: Meteor[] = [];
    let raf = 0, last = 0, untilNext = 1200;

    const rand = (a: number, b: number) => a + Math.random() * (b - a);

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = window.innerWidth;
      h = window.innerHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      stars = Array.from({ length: Math.round((w * h) / 2400) }, () => {
        const big = Math.random() < 0.07;
        return { x: rand(0, w), y: rand(0, h), r: big ? rand(1.1, 1.8) : rand(0.35, 1), a: rand(0.35, 1), phase: rand(0, Math.PI * 2), speed: rand(0.5, 1.8), tinted: Math.random() < 0.3 };
      });
      draw(performance.now(), 0);
    };

    const spawn = () => {
      const angle = rand(0.38, 0.62); // ~22–35° below horizontal, travelling down-left
      const speed = rand(0.7, 1.25); // px per ms
      const maxLife = rand(700, 1200);
      meteors.push({ x: rand(w * 0.25, w * 1.1), y: rand(-h * 0.1, h * 0.4), vx: -Math.cos(angle) * speed, vy: Math.sin(angle) * speed, len: rand(140, 280), life: maxLife, maxLife, width: rand(1.2, 2.2) });
    };

    function draw(now: number, dt: number) {
      const light = settings.theme === "light";
      const hue = settings.accentHue;
      ctx.clearRect(0, 0, w, h);

      for (const s of stars) {
        const tw = dt === 0 ? 0.85 : 0.6 + 0.4 * Math.sin((now / 1000) * s.speed + s.phase);
        const alpha = s.a * tw * (light ? 0.55 : 1);
        ctx.fillStyle = light
          ? `oklch(0.5 0.16 ${hue} / ${alpha})`
          : s.tinted ? `oklch(0.86 0.09 ${hue} / ${alpha})` : `rgb(255 255 255 / ${alpha})`;
        ctx.beginPath();
        ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
        ctx.fill();
      }

      for (const m of meteors) {
        m.x += m.vx * dt;
        m.y += m.vy * dt;
        m.life -= dt;
        // Fade in fast, fade out slow.
        const t = m.life / m.maxLife;
        const fade = Math.min(1, (1 - t) * 6) * Math.min(1, t * 2.2);
        const speed = Math.hypot(m.vx, m.vy);
        const tx = m.x - (m.vx / speed) * m.len;
        const ty = m.y - (m.vy / speed) * m.len;
        const g = ctx.createLinearGradient(m.x, m.y, tx, ty);
        g.addColorStop(0, light ? `oklch(0.45 0.2 ${hue} / ${fade})` : `rgb(255 255 255 / ${fade})`);
        g.addColorStop(0.25, `oklch(${light ? 0.55 : 0.8} 0.17 ${hue} / ${fade * 0.6})`);
        g.addColorStop(1, `oklch(0.7 0.2 ${hue + 60} / 0)`);
        ctx.strokeStyle = g;
        ctx.lineWidth = m.width;
        ctx.lineCap = "round";
        ctx.beginPath();
        ctx.moveTo(m.x, m.y);
        ctx.lineTo(tx, ty);
        ctx.stroke();
        // Glowing head
        const head = ctx.createRadialGradient(m.x, m.y, 0, m.x, m.y, 7);
        head.addColorStop(0, light ? `oklch(0.5 0.2 ${hue} / ${fade * 0.8})` : `rgb(255 255 255 / ${fade * 0.9})`);
        head.addColorStop(1, `oklch(0.8 0.15 ${hue} / 0)`);
        ctx.fillStyle = head;
        ctx.beginPath();
        ctx.arc(m.x, m.y, 7, 0, Math.PI * 2);
        ctx.fill();
      }
      meteors = meteors.filter((m) => m.life > 0 && m.x > -m.len && m.y < h + m.len);
    }

    const animating = () => settings.animatedBackground && !reduced.matches && !document.hidden && !Object.values(app.launch).some((l) => l?.phase === "running");

    function frame(now: number) {
      const dt = Math.min(50, now - last);
      last = now;
      untilNext -= dt;
      if (untilNext <= 0) {
        spawn();
        if (Math.random() < 0.18) setTimeout(spawn, rand(120, 380)); // occasional pair
        untilNext = rand(1800, 5200);
      }
      draw(now, dt);
      raf = animating() ? requestAnimationFrame(frame) : 0;
      if (!raf) {
        meteors = [];
        draw(now, 0);
      }
    }

    const sync = () => {
      if (animating() && !raf) {
        last = performance.now();
        raf = requestAnimationFrame(frame);
      } else if (!animating()) {
        draw(performance.now(), 0);
      }
    };

    resize();
    window.addEventListener("resize", resize);
    document.addEventListener("visibilitychange", sync);
    reduced.addEventListener("change", sync);
    // Re-evaluate when the toggle, game state, or colors change.
    createEffect(() => {
      settings.animatedBackground;
      settings.theme;
      settings.accentHue;
      Object.values(app.launch).map((l) => l?.phase);
      sync();
    });

    onCleanup(() => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      document.removeEventListener("visibilitychange", sync);
      reduced.removeEventListener("change", sync);
    });
  });

  return (
    <div class="sky" aria-hidden="true">
      <canvas ref={canvas} />
    </div>
  );
}
