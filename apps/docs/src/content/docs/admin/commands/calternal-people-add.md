---
title: "calternal people add"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-add-width-120.txt, crates/cli/tests/golden/people-add.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people add --space SPACE (--account ID or --group ID) --role ROLE` | Grant a direct role to a person or group. | Grant confirmation. | empty object |

## JSON data shape

```json
{}
```

## Human example

Recorded test fixture:

```text
Access granted.
```

## JSON example

Recorded test fixture:

```json
{
  "data": {},
  "ok": true,
  "request_id": "people-example"
}
```
