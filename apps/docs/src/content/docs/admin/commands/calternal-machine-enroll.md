---
title: "calternal machine enroll"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/machine/tests/golden/machine-enroll.txt, crates/machine/tests/golden/machine-enroll.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machine enroll --server URL --code CODE [--dir DIR] [--name LABEL]` | Exchange a one-time code for a machine identity; saves credentials (0600). `--code` also reads `CALTERNAL_ENROLL_CODE`. | `machine`, `owner`, `server`, `credentials` (path). | `machine_id`, `owner`, `server`, `credentials_file` |

## JSON data shape

```json
{
  "machine_id": "string",
  "owner": "string",
  "server": "string",
  "credentials_file": "string"
}
```

## Human example

Recorded test fixture:

```text
machine:      aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
owner:        pool
server:       https://cal.example.org
credentials:  /home/u/.local/share/calternal/machine/credentials
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "credentials_file": "/home/u/.local/share/calternal/machine/credentials",
    "machine_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "owner": "pool",
    "server": "https://cal.example.org"
  },
  "ok": true
}
```
