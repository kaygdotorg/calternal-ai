---
title: "calternal requests approve"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/requests-approve.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `requests approve ID --lifetime once\|this-thread` | Approve using a fresh passkey session. | Request ID and status. | `id`, `status` |

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
calternal requests approve --help
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "id": "019a8c00-0000-7000-8000-000000000001",
    "status": "approved"
  },
  "ok": true,
  "request_id": "fixture-request"
}
```
