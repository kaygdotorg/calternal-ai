---
title: "cl notes get"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/notes-get.txt, crates/cli/tests/golden/notes-get.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `notes get ID` | Show one note. | `id`, `space`, `title`, `revision`, `updated`, blank line, the Markdown. | `note` |

## JSON data shape

```json
{
  "note": {
    "id": "string",
    "space_id": "string",
    "title": "string",
    "markdown": "string",
    "revision": "string",
    "updated_at": "string"
  }
}
```

## Human example

Recorded test fixture:

```text
id:        11111111-1111-4111-8111-111111111111
space:     22222222-2222-4222-8222-222222222222
title:     Groceries
revision:  r-7
updated:   2026-11-17 07:12:03 +01:00

- milk
- eggs
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "note": {
      "id": "11111111-1111-4111-8111-111111111111",
      "markdown": "- milk\n- eggs\n",
      "revision": "r-7",
      "space_id": "22222222-2222-4222-8222-222222222222",
      "title": "Groceries",
      "updated_at": "2026-11-17T06:12:03Z"
    }
  },
  "ok": true,
  "request_id": "req-1"
}
```
