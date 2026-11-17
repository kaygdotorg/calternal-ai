---
title: "secrets.revoke"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Destroy a personal secret or request; space management requires secrets.manage

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "irreversible",
  "id": "secrets.revoke",
  "description": "Destroy a personal secret or request; space management requires secrets.manage",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Destroy a secret (`name`) or withdraw a pending request (`request_id`); exactly one.",
  "properties": {
    "name": {
      "description": "Secret to destroy, with every version.",
      "type": "string"
    },
    "request_id": {
      "description": "Pending request to withdraw.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning space UUID; omit for a personal secret.",
      "type": "string"
    }
  },
  "title": "SecretsRevokeInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Empty success for `secrets.revoke`.",
  "title": "SecretsRevokeResult",
  "type": "object"
}
```
