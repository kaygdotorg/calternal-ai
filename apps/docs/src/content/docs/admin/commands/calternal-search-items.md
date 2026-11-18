---
title: "calternal search items"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/search-items.txt, crates/cli/tests/golden/search-items.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `search items PREFIX [--space UUID] [--limit N] [--cursor TOKEN]` | Look up items by title prefix. | Item table and optional continuation. | `hits`, `partial`, optional `next_cursor` |

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
TITLE         KIND   ADDRESS                                      SPACE
Review notes  notes  /notes/019aabb0-1100-7111-8111-111111111111  019aabb0-2200-7222-8222-222222222222

next cursor:  p-next
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "hits": [
      {
        "address": "/notes/019aabb0-1100-7111-8111-111111111111",
        "excerpt": [],
        "kind": "notes",
        "space_id": "019aabb0-2200-7222-8222-222222222222",
        "title": "Review notes"
      }
    ],
    "next_cursor": "p-next",
    "partial": false
  },
  "ok": true,
  "request_id": "req-search"
}
```
