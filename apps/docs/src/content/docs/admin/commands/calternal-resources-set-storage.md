---
title: "calternal resources set-storage"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `resources set-storage SPACE [--text-bytes BYTES] [--binary-bytes BYTES] [--total-bytes BYTES]` | Replace per-kind and combined logical ceilings for a space. Omit a ceiling to clear it. Uses the installation arguments below. | One sentence. | `space_id`, `limits` |

## JSON data shape

```json
{
  "space_id": "string",
  "limits": "any"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal resources set-storage --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal resources set-storage --json
```
