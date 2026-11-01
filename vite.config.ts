import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";
import { viteSingleFile } from "vite-plugin-singlefile";

// `vite build --mode artifact` produces one self-contained HTML file for the
// shareable UI preview. Normal builds feed Tauri (fixed dev port, relative paths).
export default defineConfig(({ mode }) => {
  const artifact = mode === "artifact";
  return {
    plugins: [solid(), tailwindcss(), ...(artifact ? [viteSingleFile()] : [])],
    base: "./",
    clearScreen: false,
    server: { port: 1420, strictPort: true },
    define: { "import.meta.env.VITE_ARTIFACT": JSON.stringify(artifact) },
    build: {
      target: "es2022",
      sourcemap: false,
      chunkSizeWarningLimit: 700,
      outDir: artifact ? "dist-artifact" : "dist",
      ...(artifact ? { assetsInlineLimit: 1e8 } : {}),
    },
  };
});
