---
title: "cl invoke"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/invoke.txt, crates/cli/tests/golden/invoke.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `invoke OPERATION --input JSON` | Call any operation by id. | The operation's data as indented JSON (the same text as `data` in `--json`). | the operation's own result (`any`) |

## JSON data shape

```json
"any"
```

## Human example

Recorded test fixture:

```text
{
  "anything": [
    1,
    2
  ]
}
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "anything": [
      1,
      2
    ]
  },
  "ok": true,
  "request_id": "req-1"
}
```
