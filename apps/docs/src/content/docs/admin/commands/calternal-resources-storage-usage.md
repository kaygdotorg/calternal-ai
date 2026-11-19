---
title: "calternal resources storage-usage"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `resources storage-usage SPACE --appdata DIR --userdata-ssd DIR --userdata-hdd DIR` | Read committed current body bytes for one space. Recoverable Trash remains charged until purge. | Space ID, resource owner, text bytes, binary bytes. | `space_id`, `owner`, `text_bytes`, `binary_bytes` |

## JSON data shape

```json
{
  "space_id": "string",
  "owner": "any",
  "text_bytes": "integer",
  "binary_bytes": "integer"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal resources storage-usage --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal resources storage-usage --json
```
