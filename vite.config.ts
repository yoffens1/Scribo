import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: {
      "@database": path.resolve(__dirname, "./frontend/core/database"),
      "@automation": path.resolve(__dirname, "./frontend/core/automation"),
      "@logging": path.resolve(__dirname, "./frontend/core/logging"),
      "@refinery": path.resolve(__dirname, "./frontend/core/refinery"),
      "@llm": path.resolve(__dirname, "./frontend/core/llm"),
      "@retrieval": path.resolve(__dirname, "./frontend/core/retrieval"),
      "@test": path.resolve(__dirname, "./frontend/test"),
      "@utils": path.resolve(__dirname, "./frontend/utils"),
      "@settings": path.resolve(__dirname, "./frontend/settings.ts"),
      "@ai": path.resolve(__dirname, "./frontend/core/ai"),
      "@translation": path.resolve(__dirname, "./frontend/core/translation"),
      "@filesearch": path.resolve(__dirname, "./frontend/core/filesearch")
    }
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
