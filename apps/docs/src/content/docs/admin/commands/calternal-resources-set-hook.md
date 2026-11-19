---
title: "calternal resources set-hook"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `resources set-hook SPACE [PACKAGE --checksum SHA256]` | Select an exact reviewed package as this space's hosted admission hook. An optional `--state-space SPACE` points to an operator-controlled state space. Omit PACKAGE to clear. Uses the installation arguments below. | One sentence. | `space_id`, `binding` |

## JSON data shape

```json
{
  "space_id": "string",
  "binding": "any"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal resources set-hook --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal resources set-hook --json
```
