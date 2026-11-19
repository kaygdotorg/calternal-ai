---
title: "calternal plugin-digest"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/server/tests/golden/cli/plugin-digest.txt, crates/server/tests/golden/cli/plugin-digest.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `plugin-digest [--plugin-root DIR]` | Hash each trusted worker bundle and say whether `trusted-plugins.json` pins it. | Table `PLUGIN WORKER SHA-256 STATE` (`pinned`, `NOT PINNED`, `unreadable: …`), or a friendly empty result. | `workers[]`: `id`, `worker`, `sha256` (or null), `pinned`, `state` |

## JSON data shape

```json
{
  "workers": [
    {
      "id": "string",
      "worker": "string",
      "sha256": "string?",
      "pinned": "bool",
      "state": "string"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
exit: 0
--- stdout ---
PLUGIN                           WORKER                                          SHA-256  STATE
example-worker                   example-worker/dist/worker.js                   -        unreadable: worker: No such file or directory (os error 2)
example-worker-with-a-long-name  example-worker-with-a-long-name/dist/worker.js  -        unreadable: worker: No such file or directory (os error 2)
--- stderr ---
```

## JSON example

Recorded test fixture:

```json
exit: 0
--- stdout ---
{
  "data": {
    "workers": [
      {
        "id": "example-worker",
        "pinned": false,
        "sha256": null,
        "state": "unreadable: worker: No such file or directory (os error 2)",
        "worker": "example-worker/dist/worker.js"
      },
      {
        "id": "example-worker-with-a-long-name",
        "pinned": false,
        "sha256": null,
        "state": "unreadable: worker: No such file or directory (os error 2)",
        "worker": "example-worker-with-a-long-name/dist/worker.js"
      }
    ]
  },
  "ok": true
}
--- stderr ---
```
