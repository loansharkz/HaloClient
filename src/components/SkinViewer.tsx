import { createEffect, onCleanup, onMount } from "solid-js";
import { makeCape, makeSkin } from "../lib/skin";

/**
 * 3D skin render. skinview3d (and three.js) are dynamically imported so they
 * never block the launcher's first paint. Uses the account's real skin when
 * available; otherwise a procedural placeholder (no Mojang assets bundled).
 */
export default function SkinViewer(props: {
  seed: number;
  skin?: string | null;
  cape?: string | null;
  showCape?: boolean;
  slim?: boolean;
  width?: number;
  height?: number;
  walk?: boolean;
}) {
  let canvas!: HTMLCanvasElement;
  let viewer: any;
  let observer: IntersectionObserver | undefined;

  const skinSource = () => props.skin ?? makeSkin(props.seed);
  const capeSource = () => props.cape ?? (props.showCape ? makeCape(props.seed) : null);

  onMount(async () => {
    const sv = await import("skinview3d");
    viewer = new sv.SkinViewer({
      canvas,
      width: props.width ?? 220,
      height: props.height ?? 300,
      zoom: 0.82,
      fov: 45,
    });
    viewer.renderer.setClearColor(0x000000, 0);
    viewer.autoRotate = true;
    viewer.autoRotateSpeed = 0.6;
    viewer.animation = props.walk === false ? new sv.IdleAnimation() : new sv.WalkingAnimation();
    if (viewer.animation) viewer.animation.speed = 0.55;
    viewer.globalLight.intensity = 3;
    viewer.cameraLight.intensity = 0.6;
    apply();
    // Pause rendering when offscreen so idle GPU use is ~0.
    observer = new IntersectionObserver(([e]) => (viewer.renderPaused = !e.isIntersecting));
    observer.observe(canvas);
  });

  function apply() {
    if (!viewer) return;
    viewer.loadSkin(skinSource(), { model: props.slim ? "slim" : "default" });
    const cape = capeSource();
    if (cape) viewer.loadCape(cape);
    else viewer.resetCape();
  }

  createEffect(() => {
    props.seed;
    props.skin;
    props.cape;
    props.showCape;
    props.slim;
    apply();
  });

  onCleanup(() => {
    observer?.disconnect();
    viewer?.dispose();
  });

  return <canvas ref={canvas} class="cursor-grab active:cursor-grabbing" aria-label="3D skin preview" />;
}
