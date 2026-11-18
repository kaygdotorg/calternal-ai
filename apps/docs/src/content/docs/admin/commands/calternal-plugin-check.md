---
title: "calternal plugin check"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Command output shape registered by the CLI tests. See CLI conventions for global options.

## JSON data shape

```json
{
  "id": "string",
  "version": "string",
  "host_calls": [
    "string"
  ],
  "operations": [
    {
      "id": "string",
      "risk": "string",
      "capability": "string"
    }
  ]
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal plugin check --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal plugin check --json
```
