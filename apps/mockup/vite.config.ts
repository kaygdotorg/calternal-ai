import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { join } from "node:path";

// The mockup is a plain static site (served by serve.mjs, or any static host). It reuses packages/ui by path (its tokens, styles and components) and nothing else of the workspace.
export default defineConfig({
  root: __dirname,
  base: "./",
  plugins: [svelte()],
  resolve: { dedupe: ["svelte"], conditions: ["browser"], alias: [
    { find: "@calternal/ui/styles", replacement: join(__dirname, "../../packages/ui/src/styles/index.css") },
    { find: "@calternal/ui", replacement: join(__dirname, "../../packages/ui/src/index.ts") },
  ] },
  server: { fs: { allow: [join(__dirname, "../..")] }, host: "0.0.0.0", port: 4313 },
  build: { target: "es2022", outDir: join(__dirname, "dist"), emptyOutDir: true },
});
