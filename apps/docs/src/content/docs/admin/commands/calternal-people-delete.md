---
title: "calternal people delete"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people delete [--account ACCOUNT_UUID]` | Permanently erase your account or delete another as a server administrator. | Account UUID and deletion status. | `account_id`, `deleted` |

## JSON data shape

```json
{
  "account_id": "string",
  "deleted": "bool"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal people delete --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal people delete --json
```
