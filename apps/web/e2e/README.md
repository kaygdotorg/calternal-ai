# Browser suites

`gate.mjs` (`npm run e2e:gate -w apps/web`) is the merge gate; `gate-server.mjs` (`npm run e2e:server`) runs the server-mode suites. Both are described in the repository README ("UI merge gate").

## Real-server suites

Build the web app and server from the same checkout on a development VM.
Run `npm ci` first so a previous checkout's dependencies cannot leak into the
build. In `nix develop`, run `npm run build:plugins`,
`npm run build -w apps/web` and `cargo build --locked -p calternal-server -j3`.
From `apps/web`, set `CAL_BIN` to the absolute path of the matching binary
and `CAL_DIST` to the web build directory before running
`node e2e/machines-server.mjs` or `node e2e/skills-server.mjs`.
The suites use fresh disposable roots and a virtual passkey.

## In the gate

first-visit, shell-audit, flat-sidebars, resize, sidebar-scroll, a11y (four parts: device and scheme), text-parity, palette, palette-focus (12 opens), interact, phone-nav, settings-pages, focus-ring, swipe-rows, morph, states, new-thread, keyboard-threads, swipe, sidebar-pull, switch, safe-areas, seam, section-spacing, floating-material, onboarding, app-switcher, composer. Each exits non-zero on a failed check. When a decided behaviour changes on purpose, change the suite in the same merge.

The gate sets `CAL_GATE=1`: a suite then reports a timing measurement as `info` instead of failing on it (`palette`: keystroke cost at 6x CPU slowdown, which failed on dev-01 and passed on dev-02 with the same code; `morph`: the interrupted-open check needs at least 12 sampled frames and says so when a busy machine drew fewer). Measure on the perf VMs.

## Left out, and why

- **Performance and profiling scripts** (`perf*`, `shell-perf`, `paint-*`, `profile-*`, `trace-*`, `editor-perf`, `ab-*`, `bench-page`, `first-paint`, `first-load`): they measure; timings on a shared dev VM are not a pass or fail. Measure on the perf VMs.
- **Screenshot scripts** (`shots`, `gpu-*`, `*-shots`, `review-shots`, `docs-*`, `font-*`, `veil-pixels`, `wallpapers`): they produce pictures for a person to look at.
- **Server-mode** (`sign-in`, `a11y-server`, `people-server`, `machines-server`, `startup-server`): in `e2e:server`. `notes-server`, `notes-remote` (needs the scale seed), `long-server`, `scale-*` and `preview-*` stay manual (dev-host numbers, a seeded data set, or the live preview).
- **Threads suites not yet in the gate**: `a11y-threads` (fails the mention-picker contrast check on dev-01, also on a clean origin/dev: axe reads the tooltip while it fades in; Threads lane to make it wait), `live-threads`, `live-flaky`: not checked for determinism yet.

## Flood mode (fixtures build only)

Query flags that make the fixtures a heavy, moving data set, to try lists and panes at a size nobody has yet (owner, 2026-11-19). Code: `src/lib/flood.ts`, `src/apps/threads/fixtures.ts`, `src/apps/notes/client.ts`.

| Flag | Meaning |
|---|---|
| `?flood` or `?flood=N` | N threads (default 5,000; at most 20,000) and N notes: long and short titles, the four states (idle, running, waiting, failed, done), Claude and Codex marks, four workspaces, times across months, five named sections over the first 400 threads, 1 in 40 waiting threads with a question card. Every thread has its own seed, so each one has different heavy content (long transcripts with tool steps, code blocks, tables, pictures, run output); every note differs in length, headings, tables, code and tags |
| `&churn` or `&churn=N` | N changes a second (default 4, at most 50) from one seeded stream: a thread gets a message (time, state and count change, it moves to the top), a new thread arrives at the top, one is renamed, one is removed; notes are created, renamed or trashed the same way |
| `?notes=N` | only generated notes, no flood threads |

Example: `/threads?flood&churn` and `/notes?flood&churn=8`. `e2e/list-keys.mjs` holds Down for five seconds through the flood with and without churn (see `docs/native-feel.md`, section 20). The threads model has no unread count or detail line yet, so the flood varies state, provider, title, time and position instead.
