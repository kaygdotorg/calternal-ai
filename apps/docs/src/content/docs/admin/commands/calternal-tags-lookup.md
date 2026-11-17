---
title: "calternal tags lookup"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/tags-lookup.txt, crates/cli/tests/golden/tags-lookup.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `tags lookup --space UUID PREFIX` | Find tags by name prefix. | Tag and count table | `tags[]` |

## JSON data shape

```json
{
  "tags": [
    {
      "name": "string",
      "count": "integer"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
TAG   NOTES
work  3
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "tags": [
      {
        "count": 3,
        "name": "work"
      }
    ]
  },
  "ok": true,
  "request_id": "req-notes"
}
```
