---
title: "cl plugins list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/plugins-list.txt, crates/cli/tests/golden/plugins-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `plugins list` | Installed plugins for a space. | Table `ID NAME VERSION ENABLED GENERATION`. | `plugins[]` |

## JSON data shape

```json
{
  "plugins": [
    {
      "manifest": "any",
      "enabled": "bool",
      "generation": "integer"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
ID          NAME        VERSION  ENABLED  GENERATION
word-count  Word count  0.1.0    yes      3
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "plugins": [
      {
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
    ]
  },
  "ok": true,
  "request_id": "req-1"
}
```
