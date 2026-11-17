---
title: "cl pending list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Command output shape registered by the CLI tests. See CLI conventions for global options.

## JSON data shape

```json
{
  "records": [
    {
      "id": "string",
      "space_id": "string",
      "item_id": "string",
      "revision": "string",
      "created_at_ms": "integer",
      "text": "string?",
      "request": "any",
      "state": "string"
    }
  ]
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
cl pending list --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
cl pending list --json
```
