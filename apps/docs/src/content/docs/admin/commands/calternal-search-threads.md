---
title: "calternal search threads"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/search-threads.txt, crates/cli/tests/golden/search-threads.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `search threads PREFIX [--space UUID] [--limit N] [--cursor TOKEN]` | Look up threads by title prefix. | Thread table and optional continuation. | `hits`, `partial`, optional `next_cursor` |

## JSON data shape

```json
{
  "hits": [
    {
      "address": "string",
      "kind": "string",
      "space_id": "string",
      "title": "string",
      "excerpt": [
        {
          "text": "string",
          "matched": "bool"
        }
      ]
    }
  ],
  "partial": "bool",
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
TITLE        KIND     ADDRESS                                        SPACE
Review plan  threads  /threads/019aabb0-1100-7111-8111-111111111111  019aabb0-2200-7222-8222-222222222222
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "hits": [
      {
        "address": "/threads/019aabb0-1100-7111-8111-111111111111",
        "excerpt": [],
        "kind": "threads",
        "space_id": "019aabb0-2200-7222-8222-222222222222",
        "title": "Review plan"
      }
    ],
    "partial": false
  },
  "ok": true,
  "request_id": "req-search"
}
```
