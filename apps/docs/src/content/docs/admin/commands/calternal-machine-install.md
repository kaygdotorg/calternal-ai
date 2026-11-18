---
title: "calternal machine install"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machine install` | Print a single-use personal enrollment script. Supply a person credential. | Shell script. Keep it private and run it on the machine before the code expires. | `script`, `expires_ms` |

## JSON data shape

```json
{
  "required": [
    "script",
    "expires_ms"
  ],
  "optional": []
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal machine install --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal machine install --json
```
