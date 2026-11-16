import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  build: { target: 'es2022', sourcemap: false, outDir: 'dist', assetsInlineLimit: 0 },
  server: {
    port: 5173,
    proxy: { '/api': process.env['API_TARGET'] ?? 'http://127.0.0.1:3210' }
  },
  resolve: process.env['VITEST'] ? { conditions: ['browser'] } : undefined,
  test: { environment: 'jsdom', include: ['tests/**/*.test.ts'] }
});
