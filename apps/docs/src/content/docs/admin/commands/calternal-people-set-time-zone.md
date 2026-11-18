---
title: "calternal people set-time-zone"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-set-time-zone-width-120.txt, crates/cli/tests/golden/people-set-time-zone.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people set-time-zone [ZONE]` | Save your IANA time zone or follow this device. | Account ID and time-zone preference. | `account_id`, `time_zone` |

## JSON data shape

```json
{
  "account_id": "string",
  "time_zone": "string?"
}
```

## Human example

Recorded test fixture:

```text
Account:    01912345-6789-7abc-8123-000000000001
Time zone:  Asia/Kolkata
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "account_id": "01912345-6789-7abc-8123-000000000001",
    "time_zone": "Asia/Kolkata"
  },
  "ok": true,
  "request_id": "people-example"
}
```
