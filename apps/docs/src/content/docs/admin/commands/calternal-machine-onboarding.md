---
title: "calternal machine onboarding"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machine onboarding` | Check the first owner's instance machine. Supply a person credential with server settings authority. | Attachment, placement availability and machine ID if present. | `attached`, `ready`, `machine_id` |

## JSON data shape

```json
{
  "required": [
    "attached",
    "ready",
    "machine_id"
  ],
  "optional": []
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal machine onboarding --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal machine onboarding --json
```
