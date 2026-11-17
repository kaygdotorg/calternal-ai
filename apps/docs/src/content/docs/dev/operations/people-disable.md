---
title: "people.disable"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Disable an account and revoke its sessions.

Capability: people.manage.

## Declaration

```json
{
  "risk": "irreversible",
  "required_capability": "people.manage",
  "target": "server",
  "id": "people.disable",
  "description": "Disable an account and revoke its sessions.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Target of a server people operation.",
  "properties": {
    "account_id": {
      "description": "Account UUID.",
      "type": "string"
    }
  },
  "required": [
    "account_id"
  ],
  "title": "PersonInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "A completed account disable.",
  "properties": {
    "account_id": {
      "description": "Disabled account UUID.",
      "type": "string"
    },
    "disabled": {
      "description": "Always true on a successful disable.",
      "type": "boolean"
    }
  },
  "required": [
    "account_id",
    "disabled"
  ],
  "title": "DisabledPerson",
  "type": "object"
}
```
