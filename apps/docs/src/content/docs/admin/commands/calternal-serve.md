---
title: "calternal serve"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/server/tests/golden/cli/serve.txt, crates/server/tests/golden/cli/serve.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `serve --data-dir DIR [--port N] [--web-dir DIR] [--development] [--origin URL] [--trusted-proxies CIDRS] [--plugin-root DIR] [--node PATH] [--machine-lease-ttl-ms N] [--machine-poll-hold-ms N]` | Run the server on 127.0.0.1 until stopped. Prints one startup block, then logs to stderr. | Title line, then `url`, one `<name> token file` per development token (paths only, never tokens), `startup info`. | `profile`, `url`, `token_files` (name → path), `startup_file` |

## JSON data shape

```json
{
  "profile": "string",
  "url": "string",
  "token_files": "any",
  "startup_file": "string"
}
```

## Human example

Recorded test fixture:

```text
exit: 0
--- stdout ---
Calternal-AI development preview (disposable data; not production)
url:               http://127.0.0.1:<PORT>
owner token file:  <DATA>/dev-tokens/owner.token
guest token file:  <DATA>/dev-tokens/guest.token
startup info:      <DATA>/startup.json
--- stderr ---
```

## JSON example

Recorded test fixture:

```json
exit: 0
--- stdout ---
{
  "data": {
    "profile": "development-preview",
    "startup_file": "<DATA>/startup.json",
    "token_files": {
      "guest": "<DATA>/dev-tokens/guest.token",
      "owner": "<DATA>/dev-tokens/owner.token"
    },
    "url": "http://127.0.0.1:<PORT>"
  },
  "ok": true
}
--- stderr ---
```
