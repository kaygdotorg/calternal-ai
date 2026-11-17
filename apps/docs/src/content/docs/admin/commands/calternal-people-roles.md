---
title: "calternal people roles"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-roles-width-120.txt, crates/cli/tests/golden/people-roles.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people roles` | Show role definitions for a space. | Role labels and capability tables. | `roles` |

## JSON data shape

```json
{
  "roles": [
    {
      "name": "string",
      "capabilities": [
        "string"
      ]
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
role:  editor
CAPABILITY
content.write
plugins.use
space.read
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "roles": [
      {
        "capabilities": [
          "content.write",
          "plugins.use",
          "space.read"
        ],
        "name": "editor"
      }
    ]
  },
  "ok": true,
  "request_id": "people-example"
}
```
