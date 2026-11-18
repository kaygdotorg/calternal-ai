# ui-tree bench (throwaway, not shipped)

Measures what a plugin-sent UI tree costs to draw: a worker sends patches, a small recursive renderer maps them onto the real `packages/ui` components (`VirtualList`, `Row`, `Panel`/`Glass`, `Button`).
Written for docs/ui-host-catalogue.md. It is a measurement, not a renderer: nothing here is imported by the app and the main build never sees it.

    cd apps/web && npx vite build --config bench/ui-tree/vite.config.ts && node bench/ui-tree/run.mjs

`run.mjs` serves `bench/ui-tree/dist`, drives Chromium (Nix) with CPU throttling 1x and 4x, and prints frame-time and latency tables.
