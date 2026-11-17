---
title: "cl requests list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/requests-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `requests list` | List private questions, approvals and secret prompts. | Adaptive table with kind, request ID, space and detail. | `requests[]` |

## JSON data shape

```json
{
  "requests": [
    {
      "id": "string",
      "space_id": "string?",
      "thread_id": "string?",
      "detail": "any"
    }
  ]
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
cl requests list --help
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "requests": [
      {
        "detail": {
          "action": {
            "operation": "notes.create"
          },
          "app": "notes",
          "kind": "approval",
          "write": true
        },
        "id": "019a8c00-0000-7000-8000-000000000001",
        "space_id": "019a8c00-0000-7000-8000-000000000002",
        "thread_id": null
      }
    ]
  },
  "ok": true,
  "request_id": "fixture-request"
}
```
