---
title: "calternal window-status"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `window-status SPACE_UUID --appdata DIR [--installation-id UUID]` | Inspect one space through the running server's private admin socket as its Unix user. The ID defaults to `CALTERNAL_INSTALLATION_ID`. | Shows the recorded over-limit item count, the failure category, and whether automatic retries stopped. | `space_id`, `skipped_items` (integer or null), `failure` (string or null), `retries_exhausted` |

## JSON data shape

```json
{
  "space_id": "string",
  "skipped_items": "any",
  "failure": "string?",
  "retries_exhausted": "bool"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal window-status --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal window-status --json
```
