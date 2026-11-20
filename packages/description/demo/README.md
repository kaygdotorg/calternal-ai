# Overnight demo

A static server plus a proxy for the step 0 host, so a browser needs no tunnel and never sees a bearer.

- URL: `http://10.69.69.222:4312/` (dev-02 on the private network). `/app/?host=/api&viewName=reading|notes|threads[&synthetic=1]` draws a view.
- `demo/server.mjs` serves `dist-harness/` at `/app/` and `demo/index.html` at `/`, and forwards only the sealed routes under `/api/` to `STEP0_BASE` (default `http://127.0.0.1:43125/step0/v2`), adding the owner bearer from `BEARER_FILE` (default `~/Developer/calternal-ai-lanes/.owner.token`) and dropping any `Authorization` a client sends.
- Restart on dev-02 (`ubuntu@calternal-dev-02-std`): `cd ~/Developer/calternal-ai-lanes/v2-renderer-web && bash packages/description/demo/run.sh` (kills and restarts the server, log in `~/Developer/calternal-ai-lanes/demo.log`). After a code change rebuild first: `flock -s /tmp/calternal-dev02.lock npm run harness:build -w @calternal/description`.
- The host lives on dev-01 and is reachable from dev-02 only through hd01's SSH forward (`dev-02:43125` to `hd01:43125` to dev-01) because dev-02 may not log in to dev-01. `~/.local/share/calternal-demo/link.sh` on hd01 keeps that forward alive and refreshes the owner token on dev-02 every 30 s (the launcher gives every run a new token file). If hd01 sleeps or the host restarts on another port, the demo shows the "host not reachable" answer until it is back.
- Stand-ins (`&synthetic=1`): transcript, composer options and Send, request card (`testing/synthetic-source.ts`). Everything else is the step 0 host's own answers, including its refusals.

## Threads fixture (no host needed)

`/app/?fixture=threads[&scenario=empty|error|stale|loading|unavailable]` draws the Threads plugin's own description (`plugins/calternal-threads/src/description.ts`) over `testing/threads-host.ts`: invented threads and people, a committed transcript as snapshot nodes, `item_view.follow`, the harness and placement choices and the protected confirmation. Narrow the window to 720 px or less for the phone layout. The browser suite is `e2e/threads.mjs` (`node e2e/threads.mjs <shots-directory>` also saves captures).
