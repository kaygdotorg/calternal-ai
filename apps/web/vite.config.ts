import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * The default wallpaper on first paint: when the build has the default photograph's tiny pre-blurred renditions (scripts/wallpapers.mjs), index.html paints them behind the app before
 * any script runs, for a visit with no stored wallpaper choice (or the default chosen). The real wallpaper, blurred and veiled by script, takes over when it is ready.
 */
const cheapWallpaper = () => ({
  name: "cheap-wallpaper",
  transformIndexHtml(html: string) {
    const id = JSON.parse(readFileSync(join(__dirname, "../../packages/ui/tokens/tokens.json"), "utf8")).appearance["default-wallpaper"] as string;
    const dir = join(__dirname, "public/wallpapers");
    if (!existsSync(join(dir, `${id}-blur.jpg`)) || !existsSync(join(dir, `${id}-blur-light.jpg`))) return html;
    const base = process.env.CAL_BASE ?? "/";
    const dark = `${base}wallpapers/${id}-blur.jpg`, light = `${base}wallpapers/${id}-blur-light.jpg`;
    // No inline script (the server's CSP allows none): the two addresses ride on the <html> element and public/boot.js, which runs first, reads them.
    const head = `<style>:root{--wall-cheap:url(${light})}:root[data-scheme="dark"]{--wall-cheap:url(${dark})}html[data-cheap]{--wall-fallback:var(--wall-cheap);background:var(--wall-cheap) center/cover no-repeat fixed}</style>`;
    return html.replace("<html ", `<html data-wall-light="${light}" data-wall-dark="${dark}" `).replace("</head>", `${head}\n  </head>`);
  },
});

// A single-page app: every path (docs/addresses.md) is served index.html. Fonts are subset files copied by scripts/fonts.mjs.
// `/api` goes to the real server (`calternal serve --development`, default 127.0.0.1:3210). The server checks Host and Origin against its own address, so the proxy
// presents the server's own: changeOrigin sets Host, and the origin header is rewritten to the target. Use with `?server=1&token=<dev token>`.
const target = process.env.CAL_SERVER ?? "http://127.0.0.1:3210";
const api = { "/api": { target, changeOrigin: true, headers: { origin: target } } };

export default defineConfig({
  // CAL_BASE mounts the app under a path (the preview host serves it at /ai/).
  base: process.env.CAL_BASE ?? "/",
  plugins: [svelte(), cheapWallpaper()],
  server: { port: 5174, strictPort: true, proxy: api },
  preview: { port: 4174, strictPort: true, proxy: api },
  // CAL_OUT builds somewhere other than dist, so a build for the preview host (base /ai/) never replaces the one the e2e scripts serve (base /).
  build: { target: "es2022", outDir: process.env.CAL_OUT ?? "dist" },
  resolve: { conditions: ["browser"] },
  test: { environment: "jsdom", include: ["src/**/*.test.ts"], setupFiles: ["./src/test-setup.ts"], css: false },
});
