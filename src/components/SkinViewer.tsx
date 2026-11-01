import { createEffect, onCleanup, onMount } from "solid-js";
import { makeCape, makeSkin } from "../lib/skin";

/**
 * 3D skin render. skinview3d (and three.js) are dynamically imported so they
 * never block the launcher's first paint, which matters for the <1s open goal.
 */
export default function SkinViewer(props: { seed: number; cape?: boolean; width?: number; height?: number; walk?: boolean }) {
  let canvas!: HTMLCanvasElement;
  let viewer: any;
  let observer: IntersectionObserver | undefined;

  onMount(async () => {
    const sv = await import("skinview3d");
    viewer = new sv.SkinViewer({
      canvas,
      width: props.width ?? 220,
      height: props.height ?? 300,
      skin: makeSkin(props.seed),
      cape: props.cape ? makeCape(props.seed) : undefined,
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
    // Pause rendering when offscreen to keep idle GPU at ~0.
    observer = new IntersectionObserver(([e]) => (viewer.renderPaused = !e.isIntersecting));
    observer.observe(canvas);
  });

  createEffect(() => {
    const seed = props.seed;
    const cape = props.cape;
    if (!viewer) return;
    viewer.loadSkin(makeSkin(seed));
    if (cape) viewer.loadCape(makeCape(seed));
    else viewer.resetCape();
  });

  onCleanup(() => {
    observer?.disconnect();
    viewer?.dispose();
  });

  return <canvas ref={canvas} class="cursor-grab active:cursor-grabbing" />;
}
