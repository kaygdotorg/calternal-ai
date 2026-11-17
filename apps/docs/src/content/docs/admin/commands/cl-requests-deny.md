---
title: "cl requests deny"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/requests-deny.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `requests deny ID` | Decline a pending approval. | Request ID and status. | `id`, `status` |

## JSON data shape

```json
{
  "id": "string",
  "status": "string"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
cl requests deny --help
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "id": "019a8c00-0000-7000-8000-000000000001",
    "status": "denied"
  },
  "ok": true,
  "request_id": "fixture-request"
}
```
