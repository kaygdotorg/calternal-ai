---
title: "calternal trash purge"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/trash-purge.txt, crates/cli/tests/golden/trash-purge.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `trash purge ID` | Permanently delete a note in Trash. | ID and deletion state | `id`, `state` |

## JSON data shape

```json
{
  "id": "string",
  "state": "string"
}
```

## Human example

Recorded test fixture:

```text
ID:     019aabb0-1100-7111-8111-111111111111
State:  Permanently deleted
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "id": "019aabb0-1100-7111-8111-111111111111",
    "state": "deleted"
  },
  "ok": true,
  "request_id": "req-notes"
}
```
