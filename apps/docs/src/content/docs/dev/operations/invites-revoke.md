---
title: "invites.revoke"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Revoke an unused space invitation.

Capability: members.invite.

## Declaration

```json
{
  "risk": "irreversible",
  "required_capability": "members.invite",
  "target": "space",
  "id": "invites.revoke",
  "description": "Revoke an unused space invitation.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Identify an invitation within its owning space.",
  "properties": {
    "invite_id": {
      "description": "Invitation record UUID.",
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "invite_id",
    "space_id"
  ],
  "title": "InviteInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "A revoked invitation's public identity.",
  "properties": {
    "invite_id": {
      "description": "Invitation record UUID.",
      "type": "string"
    }
  },
  "required": [
    "invite_id"
  ],
  "title": "InviteRevoked",
  "type": "object"
}
```
