import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { join } from "node:path";

// Reuses packages/ui by path (tokens, styles, components), as apps/mockup does; `@calternal/ui` resolves to src/lib/ui.ts,
// which re-exports only the surfaces the spike uses.
export default defineConfig({
  root: __dirname,
  base: "./",
  plugins: [svelte()],
  resolve: { dedupe: ["svelte"], conditions: ["browser"], alias: [
    { find: "@calternal/ui/styles", replacement: join(__dirname, "../../../packages/ui/src/styles/index.css") },
    { find: /^@calternal\/ui$/, replacement: join(__dirname, "src/lib/ui.ts") },
  ] },
  server: { fs: { allow: [join(__dirname, "../../..")] }, host: "0.0.0.0", port: 4391 },
  build: { target: "es2022", outDir: join(__dirname, "dist"), emptyOutDir: true },
});
