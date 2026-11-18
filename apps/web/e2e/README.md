# Browser suites

`gate.mjs` (`npm run e2e:gate -w apps/web`) is the merge gate; `gate-server.mjs` (`npm run e2e:server`) runs the server-mode suites. Both are described in the repository README ("UI merge gate").

## In the gate

first-visit, shell-audit, flat-sidebars, resize, sidebar-scroll, a11y, text-parity, palette, palette-focus (12 opens), interact, phone-nav, settings-pages, focus-ring, swipe-rows, morph, states, new-thread. Each exits non-zero on a failed check. When a decided behaviour changes on purpose, change the suite in the same merge.

## Left out, and why

- **Performance and profiling scripts** (`perf*`, `shell-perf`, `paint-*`, `profile-*`, `trace-*`, `editor-perf`, `ab-*`, `bench-page`, `first-paint`, `first-load`): they measure; timings on a shared dev VM are not a pass or fail. Measure on the perf VMs.
- **Screenshot scripts** (`shots`, `gpu-*`, `*-shots`, `review-shots`, `docs-*`, `font-*`, `veil-pixels`, `wallpapers`): they produce pictures for a person to look at.
- **Server-mode** (`sign-in`, `a11y-server`, `people-server`, `machines-server`, `startup-server`): in `e2e:server`. `notes-server`, `long-server`, `scale-*` and `preview-*` stay manual (dev-host numbers, a seeded data set, or the live preview).
- **Threads suites not yet in the gate**: `a11y-threads`, `keyboard-threads`, `live-threads`, `live-flaky`, `swipe`, `composer`: not checked for determinism yet.
