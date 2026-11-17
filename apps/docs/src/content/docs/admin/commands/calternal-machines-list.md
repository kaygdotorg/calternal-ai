---
title: "calternal machines list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/server/tests/golden/cli/machines-list.txt, crates/server/tests/golden/cli/machines-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machines list --data-dir DIR` | List enrolled machines, revoked ones included. | Table `ID OWNER NAME PLATFORM STATE` (state `enrolled`, `draining` or `revoked`), or a friendly empty result with the command to add one. | `machines[]`: `id`, `owner`, `name`, `os`, `arch`, `state` |

## JSON data shape

```json
{
  "machines": [
    {
      "id": "string",
      "owner": "string",
      "name": "string",
      "os": "string",
      "arch": "string",
      "state": "string"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
exit: 0
--- stdout ---
ID                                    OWNER                                         NAME       PLATFORM       STATE
aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa  pool                                          build-box  linux/x86_64   enrolled
bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb  account:cccccccc-cccc-4ccc-8ccc-cccccccccccc  laptop     macos/aarch64  enrolled
--- stderr ---
```

## JSON example

Recorded test fixture:

```json
exit: 0
--- stdout ---
{
  "data": {
    "machines": [
      {
        "arch": "x86_64",
        "id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "name": "build-box",
        "os": "linux",
        "owner": "pool",
        "state": "enrolled"
      },
      {
        "arch": "aarch64",
        "id": "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        "name": "laptop",
        "os": "macos",
        "owner": "account:cccccccc-cccc-4ccc-8ccc-cccccccccccc",
        "state": "enrolled"
      }
    ]
  },
  "ok": true
}
--- stderr ---
```
