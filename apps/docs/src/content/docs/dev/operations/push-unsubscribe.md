---
title: "push.unsubscribe"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Remove one of your push subscriptions

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "push.unsubscribe",
  "description": "Remove one of your push subscriptions",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Remove an installation belonging to the signed-in account.",
  "properties": {
    "device_id": {
      "description": "Installation UUID returned by push.devices.",
      "type": "string"
    }
  },
  "required": [
    "device_id"
  ],
  "title": "PushUnsubscribeInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Outcome of an idempotent subscription removal.",
  "properties": {
    "removed": {
      "description": "True when a stored subscription was removed.",
      "type": "boolean"
    }
  },
  "required": [
    "removed"
  ],
  "title": "PushRemoved",
  "type": "object"
}
```
