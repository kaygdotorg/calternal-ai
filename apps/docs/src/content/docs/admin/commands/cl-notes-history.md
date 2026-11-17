---
title: "cl notes history"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/notes-history.txt, crates/cli/tests/golden/notes-history.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `notes history ID [--limit N] [--cursor C]` | Earlier versions of a note. | Table `REVISION TITLE UPDATED`, then "More versions: repeat with --cursor C" when there are more; an empty result suggests opening the current version. | `versions[]`, `next_cursor`? |

## JSON data shape

```json
{
  "versions": [
    {
      "title": "string",
      "markdown": "string",
      "revision": "string",
      "updated_at": "string"
    }
  ],
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
REVISION  TITLE      UPDATED
r-6       Groceries  2026-11-16 21:00:00 +01:00

More versions: repeat with --cursor c-2
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "next_cursor": "c-2",
    "versions": [
      {
        "markdown": "- milk\n",
        "revision": "r-6",
        "title": "Groceries",
        "updated_at": "2026-11-16T20:00:00Z"
      }
    ]
  },
  "ok": true,
  "request_id": "req-1"
}
```
