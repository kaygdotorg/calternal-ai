import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// A single-page app: every path (docs/addresses.md) is served index.html. Fonts are subset files copied by scripts/fonts.mjs.
// `/api` goes to the real server (`calternal serve --development`, default 127.0.0.1:3210). The server checks Host and Origin against its own address, so the proxy
// presents the server's own: changeOrigin sets Host, and the origin header is rewritten to the target. Use with `?server=1&token=<dev token>`.
const target = process.env.CAL_SERVER ?? "http://127.0.0.1:3210";
const api = { "/api": { target, changeOrigin: true, headers: { origin: target } } };

export default defineConfig({
  // CAL_BASE mounts the app under a path (the preview host serves it at /ai/).
  base: process.env.CAL_BASE ?? "/",
  plugins: [svelte()],
  server: { port: 5174, strictPort: true, proxy: api },
  preview: { port: 4174, strictPort: true, proxy: api },
  // CAL_OUT builds somewhere other than dist, so a build for the preview host (base /ai/) never replaces the one the e2e scripts serve (base /).
  build: { target: "es2022", outDir: process.env.CAL_OUT ?? "dist" },
  resolve: { conditions: ["browser"] },
  test: { environment: "jsdom", include: ["src/**/*.test.ts"], setupFiles: ["./src/test-setup.ts"], css: false },
});
