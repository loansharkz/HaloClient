import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed dev port and relative asset paths in production.
export default defineConfig({
  plugins: [solid(), tailwindcss()],
  base: "./",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", sourcemap: false, chunkSizeWarningLimit: 700 },
});
