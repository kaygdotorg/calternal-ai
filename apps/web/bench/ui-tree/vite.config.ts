import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const here = dirname(fileURLToPath(import.meta.url));
export default defineConfig({ root: here, base: "./", plugins: [svelte()], build: { outDir: join(here, "dist"), emptyOutDir: true, target: "es2022" }, worker: { format: "es" } });
