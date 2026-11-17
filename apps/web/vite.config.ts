import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// A single-page app: every path (docs/addresses.md) is served index.html. Fonts are subset files copied by scripts/fonts.mjs.
export default defineConfig({
  plugins: [svelte({ hot: !process.env.VITEST })],
  server: { port: 5174, strictPort: true },
  preview: { port: 4174, strictPort: true },
  build: { target: "es2022" },
  resolve: { conditions: ["browser"] },
  test: { environment: "jsdom", include: ["src/**/*.test.ts"], setupFiles: ["./src/test-setup.ts"], css: false },
});
