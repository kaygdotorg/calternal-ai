---
title: "calternal resources set-group-storage"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `resources set-group-storage GROUP [--text-bytes BYTES] [--binary-bytes BYTES] [--total-bytes BYTES]` | Replace one allocation shared by all spaces assigned to the group. Existing contents remain readable when above a ceiling. Omit a ceiling to clear it. | One sentence. | `group_id`, `limits` |

## JSON data shape

```json
{
  "group_id": "string",
  "limits": "any"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal resources set-group-storage --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal resources set-group-storage --json
```
