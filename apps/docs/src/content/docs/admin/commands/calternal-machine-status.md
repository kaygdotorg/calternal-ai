---
title: "calternal machine status"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/machine/tests/golden/machine-status.txt, crates/machine/tests/golden/machine-status.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machine status [--dir DIR]` | Show this machine's identity and server. | Same block as `enroll`. Not enrolled: `not_found`. | as `enroll` |

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
credentials:  $HOME/.local/share/calternal/machine/credentials
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "credentials_file": "$HOME/.local/share/calternal/machine/credentials",
    "machine_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "owner": "pool",
    "server": "https://cal.example.org"
  },
  "ok": true
}
```
