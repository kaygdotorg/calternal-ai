---
title: "account.set_time_zone"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Choose your time zone or follow the device.

Capability: account.profile.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "account.profile",
  "target": "account",
  "id": "account.set_time_zone",
  "description": "Choose your time zone or follow the device.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "The caller's IANA time zone preference. Null follows the device.",
  "properties": {
    "time_zone": {
      "description": "IANA time zone name, or null to clear the stored preference.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "TimeZoneInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "The caller's saved time-zone preference.",
  "properties": {
    "account_id": {
      "description": "The unchanged account UUID.",
      "type": "string"
    },
    "time_zone": {
      "description": "Canonically cased IANA name, or null when following the device.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "account_id"
  ],
  "title": "TimeZoneResult",
  "type": "object"
}
```
