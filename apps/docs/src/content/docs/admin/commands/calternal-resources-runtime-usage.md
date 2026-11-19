---
title: "calternal resources runtime-usage"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `resources runtime-usage --account UUID` | Read confirmed hosted time and unresolved guest ends for an account or `--group UUID`. Uses the installation arguments below. | Confirmed seconds, unresolved count, start of retention. | `owner`, `usage` |

## JSON data shape

```json
{
  "owner": {
    "kind": "string",
    "id": "string"
  },
  "usage": {
    "running_seconds": "integer",
    "unresolved": "integer",
    "since_ms": "integer"
  }
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal resources runtime-usage --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal resources runtime-usage --json
```
