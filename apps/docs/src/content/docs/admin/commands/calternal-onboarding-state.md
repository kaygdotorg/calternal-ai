---
title: "calternal onboarding state"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `onboarding state` | Read whether your account has completed onboarding. Supply a person credential. | Labelled onboarding status and the next step. | `finished` |

## JSON data shape

```json
{
  "finished": "bool"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal onboarding state --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal onboarding state --json
```
