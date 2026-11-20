# Drawing spike (throwaway, #107)

Tests "plugins never draw; they declare" from the primitives-only host design: the authoring section's Reading List and Chat
declarations (`decl/`, serialised to `data/declarations.json`) and viewer-filled fixtures (`data/fixtures.json`) drawn by
generic renderers only, on the web (`web/`, Svelte with `packages/ui`) and on iPhone (`apple/`, SwiftUI). Findings:
[docs/reviews/drawing-spike-2026-11-21.md](../../docs/reviews/drawing-spike-2026-11-21.md). Not product code; nothing merges.

- Data: `cd web && npm run data` (Node 23.6+; regenerates both JSON files).
- Web, on a dev VM: `cd web && npm install && npm run build && npx vite preview --port 4391`; captures: `node shots.mjs`.
  The address picks the state: `?app=calternal-chat&item=c1&scheme=dark`, `?app=reading-list&open=All&view=board`.
- iPhone, on the Mac Studio: `cd apple && ./run.sh build && ./run.sh run -app calternal-chat -item c1`; captures:
  `./shots.sh`. Unsigned Simulator build in its own device (`drawing-spike`); the bundle identifier is a local placeholder.
- Evidence images go on #107, never into git.
