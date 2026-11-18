---
title: "calternal account-recover"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `account-recover ACCOUNT_UUID --reason REASON --identity-verified --appdata DIR [--installation-id UUID]` | Issue an enrollment-only recovery grant through the local admin socket after verifying identity outside Calternal. Run as the server's Unix user in a private terminal. | Single-use grant, valid for 15 minutes, and the address to open. Keep the output private. | `enrollment_grant`, `url`, `valid_for_seconds` |

## JSON data shape

```json
{
  "enrollment_grant": "string",
  "url": "string",
  "valid_for_seconds": "integer"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal account-recover --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal account-recover --json
```
