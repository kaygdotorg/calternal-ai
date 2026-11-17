---
title: "calternal machines code"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `machines code (--pool \| --account UUID) --appdata DIR --userdata-ssd DIR --userdata-hdd DIR [--minutes N]` | Issue a one-time machine enrollment code (default 60 minutes). | `code`, `owner`, `expires` (local), then two sentences on using it. | `code`, `owner` (`pool` or `account:<uuid>`), `expires_at` |

## JSON data shape

```json
{
  "code": "string",
  "owner": "string",
  "expires_at": "string"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal machines code --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal machines code --json
```
