---
title: "calternal resources set-group"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `resources set-compute --account UUID --hosted BOOL` | Set hosted access for an account or `--group UUID` and optional `--memory-mib`, `--active-threads`, `--queue-bound`, `--idle-seconds`, and paired `--runtime-seconds` and `--reset-seconds`. Pass `--clear` to restore default admission. Uses the same installation arguments as `resources set-group`. | One sentence. | `owner`, `limits` |

## JSON data shape

```json
{
  "space_id": "string",
  "group_id": "string?"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal resources set-group --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal resources set-group --json
```
