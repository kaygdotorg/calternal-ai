---
title: "calternal machine verify"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machine verify MACHINE_UUID --space-id SPACE_UUID [--provider ID] [--token-file FILE]` | Run one provider diagnostic on your machine or a pool machine you administer. | Sandbox status, provider installation and sign-in, and the provider's sign-in command. | `sandbox_started`, `provider` |

## JSON data shape

```json
{
  "sandbox_started": "bool",
  "provider": "any"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal machine verify --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal machine verify --json
```
