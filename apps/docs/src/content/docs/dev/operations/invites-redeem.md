---
title: "invites.redeem"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Redeem a space invitation as the signed-in account.

Capability: invites.redeem.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "invites.redeem",
  "target": "invitation",
  "id": "invites.redeem",
  "description": "Redeem a space invitation as the signed-in account.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Redeem a link as the signed-in account.",
  "properties": {
    "secret": {
      "description": "The secret from the invite URL, passed in the HTTP body.",
      "type": "string"
    }
  },
  "required": [
    "secret"
  ],
  "title": "InviteRedeemInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for members, leave and directory-record deletion.",
  "properties": {
    "space_id": {
      "description": "Space UUID; names are never accepted in its place.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceInput",
  "type": "object"
}
```
