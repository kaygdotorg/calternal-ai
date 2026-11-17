---
title: "calternal machines pool-grant"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/server/tests/golden/cli/machines-pool-grant.txt, crates/server/tests/golden/cli/machines-pool-grant.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machines pool-grant ACCOUNT [--remove] --data-dir DIR` | Allow (or stop allowing) an account to use pool machines. | One sentence. | `account`, `pool_access` |

## JSON data shape

```json
{
  "account": "string",
  "pool_access": "bool"
}
```

## Human example

Recorded test fixture:

```text
exit: 0
--- stdout ---
Account cccccccc-cccc-4ccc-8ccc-cccccccccccc may now use pool machines.
--- stderr ---
```

## JSON example

Recorded test fixture:

```json
exit: 0
--- stdout ---
{
  "data": {
    "account": "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    "pool_access": true
  },
  "ok": true
}
--- stderr ---
```
