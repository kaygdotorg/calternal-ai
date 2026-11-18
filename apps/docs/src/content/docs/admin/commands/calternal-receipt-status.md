---
title: "calternal receipt-status"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `receipt-status SPACE_UUID --appdata DIR [--installation-id UUID]` | Inspect pruning through the running server's private admin socket as its Unix user. The ID defaults to `CALTERNAL_INSTALLATION_ID`. | Reports the oldest pinned receipt past the configured retention, or that the outbox or journal is unreadable and the age is unknown. | `space_id`, `issue` (null or an object with `reason` and, for `expired_pin`, `age_ms`) |

## JSON data shape

```json
{
  "space_id": "string",
  "issue": "any"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal receipt-status --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal receipt-status --json
```
