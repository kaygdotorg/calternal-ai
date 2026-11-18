---
title: "calternal mail-test"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `mail-test RECIPIENT --appdata DIR [--installation-id UUID]` | Send one test message through the running server's private admin socket as its Unix user. The ID defaults to `CALTERNAL_INSTALLATION_ID`. | Reports when the SMTP server accepts the test message or when acceptance could not be confirmed. | `accepted` |

## JSON data shape

```json
{
  "accepted": "bool"
}
```

## Human example

No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):

```sh
calternal mail-test --help
```

## JSON example

No recorded JSON fixture is present. Invocation pattern (supply required arguments):

```sh
calternal mail-test --json
```
