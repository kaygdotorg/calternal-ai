---
title: "cl plugins preview"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/plugins-preview.txt, crates/cli/tests/golden/plugins-preview.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `plugins preview PLUGIN` | Show the plugin's GenUI view without changing anything. | One sentence, then the view as indented JSON. | `view`, `preview` |

## JSON data shape

```json
{
  "view": "any",
  "preview": "bool"
}
```

## Human example

Recorded test fixture:

```text
Preview of the plugin's GenUI view (nothing was changed):
{
  "genui_version": 2,
  "source": "Text(\"hello\")"
}
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "preview": true,
    "view": {
      "genui_version": 2,
      "source": "Text(\"hello\")"
    }
  },
  "ok": true,
  "request_id": "req-1"
}
```
