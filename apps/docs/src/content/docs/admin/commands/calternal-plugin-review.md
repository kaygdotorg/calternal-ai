---
title: "calternal plugin review"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Command output shape registered by the CLI tests. See CLI conventions for global options.

## JSON data shape

```json
{
  "id": "string",
  "version": "string",
  "checksum": "string",
  "operations": [
    {
      "id": "string",
      "risk": "string",
      "capability": "string"
    }
  ],
  "host_calls": [
    "string"
  ],
  "secrets": [
    "string"
  ],
  "complete_manifest": "any",
  "changes": "any"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal plugin review --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal plugin review --json
```
