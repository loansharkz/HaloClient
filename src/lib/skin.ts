// Procedurally paints an original 64x64 skin texture so the mockup's 3D viewer
// works without bundling any Mojang skin. Real accounts load their skin URL
// from the Minecraft profile API instead.

type Rect = [x: number, y: number, w: number, h: number];

export function makeSkin(hue: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d")!;
  const fill = (color: string, ...rects: Rect[]) => {
    g.fillStyle = color;
    for (const [x, y, w, h] of rects) g.fillRect(x, y, w, h);
  };

  const skin = "#e9b998";
  const skinShade = "#d29f7e";
  const hair = `hsl(${hue + 200} 30% 16%)`;
  const hoodie = `hsl(${hue} 70% 52%)`;
  const hoodieDark = `hsl(${hue} 60% 38%)`;
  const pants = "#232033";
  const shoes = "#f2f2f7";

  // Head: all faces skin, then hair on top + upper band.
  fill(skin, [0, 0, 32, 16]);
  fill(hair, [8, 0, 8, 8], [0, 8, 32, 2], [24, 8, 8, 8], [0, 10, 2, 3], [22, 10, 2, 3], [16, 10, 2, 3], [14, 10, 2, 3]);
  // Face (front = 8,8 8x8)
  fill("#ffffff", [9, 12, 2, 1], [13, 12, 2, 1]);
  fill(`hsl(${hue} 70% 45%)`, [10, 12, 1, 1], [13, 12, 1, 1]);
  fill(skinShade, [11, 13, 2, 1]);
  fill("#9c5b4d", [11, 14, 2, 1]);

  // Body (16..40, 16..32): hoodie.
  fill(hoodie, [16, 16, 24, 16]);
  fill(hoodieDark, [20, 26, 8, 1], [23, 20, 1, 6]);
  fill("#ffffff", [22, 21, 1, 3], [25, 21, 1, 3]); // hoodie strings

  // Right arm (40..56, 16..32) + left arm (32..48, 48..64): sleeves + hands.
  fill(hoodie, [40, 16, 16, 16], [32, 48, 16, 16]);
  fill(hoodieDark, [40, 28, 16, 1], [32, 60, 16, 1]);
  fill(skin, [40, 29, 16, 3], [32, 61, 16, 3], [48, 16, 4, 4], [36, 48, 4, 4]);

  // Right leg (0..16, 16..32) + left leg (16..32, 48..64).
  fill(pants, [0, 16, 16, 16], [16, 48, 16, 16]);
  fill(shoes, [0, 29, 16, 3], [16, 61, 16, 3], [4, 16, 4, 4], [20, 48, 4, 4]);

  return c;
}

export function makeCape(hue: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = 64;
  c.height = 32;
  const g = c.getContext("2d")!;
  const grad = g.createLinearGradient(0, 0, 0, 17);
  grad.addColorStop(0, `hsl(${hue} 75% 55%)`);
  grad.addColorStop(1, `hsl(${hue + 50} 75% 45%)`);
  g.fillStyle = grad;
  g.fillRect(0, 0, 22, 17);
  g.fillStyle = "rgba(255,255,255,.85)";
  g.fillRect(4, 6, 4, 1);
  g.fillRect(3, 7, 1, 2);
  g.fillRect(8, 7, 1, 2);
  g.fillRect(4, 9, 4, 1);
  return c;
}
