---
title: "calternal machine run"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machine run [--dir DIR] [--backend podman\|process] [--image REF] [--podman PATH] [--max-sandboxes N]` | Connect to the server and run assigned threads until stopped (SIGINT/SIGTERM). Logs to stderr. | On clean stop: `Machine stopped.` Revoked: error `revoked`, exit 3. | `stopped` |

## JSON data shape

```json
{
  "stopped": "bool"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal machine run --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal machine run --json
```
