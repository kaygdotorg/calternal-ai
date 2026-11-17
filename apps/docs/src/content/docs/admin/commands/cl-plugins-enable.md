---
title: "cl plugins enable"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/plugins-enable.txt, crates/cli/tests/golden/plugins-enable.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `plugins enable PLUGIN` / `plugins disable PLUGIN` | Turn a plugin on or off in a space. | `id`, `name`, `version`, `enabled`, `generation`. | `plugin` |

## JSON data shape

```json
{
  "plugin": {
    "manifest": "any",
    "enabled": "bool",
    "generation": "integer"
  }
}
```

## Human example

Recorded test fixture:

```text
id:          word-count
name:        Word count
version:     0.1.0
enabled:     yes
generation:  3
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "plugin": {
      "enabled": true,
      "generation": 3,
      "manifest": {
        "api_version": 1,
        "capabilities": [],
        "dependencies": [],
        "description": "d",
        "id": "word-count",
        "name": "Word count",
        "operations": [],
        "version": "0.1.0",
        "views": []
      }
    }
  },
  "ok": true,
  "request_id": "req-1"
}
```
