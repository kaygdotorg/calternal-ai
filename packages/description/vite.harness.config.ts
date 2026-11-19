import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { join } from "node:path";

// The harness page (harness/): a static build served by `vite preview`, driven by the browser suites and the performance measures.
export default defineConfig({
  root: join(__dirname, "harness"),
  plugins: [svelte()],
  resolve: { conditions: ["browser"] },
  server: { fs: { allow: [join(__dirname, "../..")] } },
  build: { target: "es2022", outDir: join(__dirname, "dist-harness"), emptyOutDir: true },
  preview: { port: 4381, strictPort: true, host: "127.0.0.1" },
});
