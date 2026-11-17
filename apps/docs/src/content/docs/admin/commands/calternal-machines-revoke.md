---
title: "calternal machines revoke"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/server/tests/golden/cli/machines-revoke.txt, crates/server/tests/golden/cli/machines-revoke.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machines revoke MACHINE --data-dir DIR` | Revoke a machine; a running server applies it within a second. | One sentence. Unknown machine: `not_found`. | `machine`, `revoked` |

## JSON data shape

```json
{
  "machine": "string",
  "revoked": "bool"
}
```

## Human example

Recorded test fixture:

```text
exit: 0
--- stdout ---
Revoked machine aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa. A running server applies it within a second.
--- stderr ---
```

## JSON example

Recorded test fixture:

```json
exit: 0
--- stdout ---
{
  "data": {
    "machine": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "revoked": true
  },
  "ok": true
}
--- stderr ---
```
