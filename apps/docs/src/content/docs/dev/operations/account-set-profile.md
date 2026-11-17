---
title: "account.set_profile"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Change your own display name.

Capability: account.profile.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "account.profile",
  "target": "account",
  "id": "account.set_profile",
  "description": "Change your own display name.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "The current account's new display name.",
  "properties": {
    "name": {
      "description": "Name shown to other people.",
      "type": "string"
    }
  },
  "required": [
    "name"
  ],
  "title": "ProfileInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "The updated profile's public display fields.",
  "properties": {
    "account_id": {
      "description": "The unchanged account UUID.",
      "type": "string"
    },
    "name": {
      "description": "The normalized name.",
      "type": "string"
    }
  },
  "required": [
    "account_id",
    "name"
  ],
  "title": "ProfileResult",
  "type": "object"
}
```
